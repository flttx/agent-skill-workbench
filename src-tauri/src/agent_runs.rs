use chrono::{DateTime, Duration, Utc};
use ignore::gitignore::GitignoreBuilder;
use ignore::Match;
use ignore::WalkBuilder;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
};
use tauri::State;
use walkdir::WalkDir;

const MAX_RESULT_CHARS: usize = 12_000;
const MAX_HISTORY_FILE_BYTES: u64 = 12_000_000;
const HISTORY_HEAD_BYTES: usize = 256_000;

#[derive(Serialize, Clone)]
pub struct AgentRun {
    pub id: String,
    pub task_id: Option<i64>,
    pub agent: String,
    pub workspace_path: String,
    pub window_mode: String,
    pub transport: String,
    pub window_handle: Option<i64>,
    pub prompt_snapshot: String,
    pub status: String,
    pub error_message: Option<String>,
    pub session_id: Option<String>,
    pub match_state: String,
    pub result_state: String,
    pub result_summary: String,
    pub changed_files: String,
    pub verification: String,
    pub unresolved_issues: String,
    pub raw_excerpt: String,
    pub result_source_path: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub change_source: String,
    pub baseline_at: Option<String>,
    pub intermediate_files: String,
    pub change_error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunStartRequest {
    pub task_id: Option<i64>,
    pub agent: String,
    pub workspace_path: Option<String>,
    pub prompt: String,
}

#[derive(Serialize)]
pub struct AgentRunStartResult {
    pub run_id: String,
    pub status: String,
    pub transport: String,
    pub error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunResultInput {
    pub run_id: String,
    pub result_summary: String,
    pub changed_files: String,
    pub verification: String,
    pub unresolved_issues: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveAgentRunInput {
    pub run_id: String,
    pub task_id: Option<i64>,
    pub action: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

impl ChangeKind {
    fn marker(self) -> char {
        match self {
            Self::Added => '+',
            Self::Modified => '~',
            Self::Deleted => '-',
        }
    }
}

#[derive(Clone)]
struct ChangedFile {
    path: String,
    kind: ChangeKind,
}

#[derive(Default, Clone)]
struct HistorySession {
    agent: String,
    session_id: String,
    workspace_path: Option<String>,
    source_path: String,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    prompt: String,
    assistant: String,
    changed_files: Vec<ChangedFile>,
    raw_excerpt: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct FileFingerprint {
    #[serde(default)]
    path: String,
    hash: String,
    size: u64,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct BaselineManifest {
    files: BTreeMap<String, FileFingerprint>,
    scan_errors: Vec<String>,
}

struct ScanResult {
    manifest: BaselineManifest,
    source: String,
}

struct FinalChangeResult {
    source: String,
    files: Vec<ChangedFile>,
    intermediate_files: String,
    error: Option<String>,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn snapshot_key(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn snapshot_relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    (!relative.is_empty()).then_some(relative)
}

fn excluded_snapshot_component(component: &str) -> bool {
    matches!(
        component.to_ascii_lowercase().as_str(),
        ".git" | ".hg" | ".svn" | "node_modules" | "dist" | "target" | ".cache" | ".vite"
    )
}

fn excluded_snapshot_path(path: &Path) -> bool {
    path.components()
        .any(|component| excluded_snapshot_component(&component.as_os_str().to_string_lossy()))
}

fn hash_file(path: &Path) -> Result<FileFingerprint, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        size += read as u64;
        for byte in &buffer[..read] {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(FileFingerprint {
        path: String::new(),
        hash: format!("{hash:016x}"),
        size,
    })
}

fn is_git_workspace(workspace: &Path) -> bool {
    Command::new("git")
        .args([
            "-C",
            &workspace.to_string_lossy(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn scan_workspace(workspace: &Path) -> ScanResult {
    let source = if is_git_workspace(workspace) {
        "git_baseline"
    } else {
        "snapshot"
    }
    .to_string();
    let mut manifest = BaselineManifest::default();
    let local_gitignore = if is_git_workspace(workspace) {
        None
    } else {
        let path = workspace.join(".gitignore");
        if path.is_file() {
            let mut builder = GitignoreBuilder::new(workspace);
            if let Some(error) = builder.add(path) {
                manifest.scan_errors.push(error.to_string());
            }
            builder.build().ok()
        } else {
            None
        }
    };
    let mut walker = WalkBuilder::new(workspace);
    walker
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .filter_entry(move |entry| {
            if excluded_snapshot_path(entry.path()) {
                return false;
            }
            local_gitignore.as_ref().is_none_or(|gitignore| {
                !matches!(
                    gitignore.matched_path_or_any_parents(
                        entry.path(),
                        entry.file_type().is_some_and(|kind| kind.is_dir())
                    ),
                    Match::Ignore(_)
                )
            })
        });
    for entry in walker.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                manifest.scan_errors.push(error.to_string());
                continue;
            }
        };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Some(relative) = snapshot_relative_path(workspace, entry.path()) else {
            continue;
        };
        match hash_file(entry.path()) {
            Ok(fingerprint) => {
                let mut fingerprint = fingerprint;
                fingerprint.path = relative;
                manifest
                    .files
                    .insert(snapshot_key(&fingerprint.path), fingerprint);
            }
            Err(error) => manifest.scan_errors.push(format!("{relative}: {error}")),
        }
    }
    ScanResult { manifest, source }
}

fn capture_run_baseline(db: &Connection, run_id: &str, workspace: &Path) -> Result<(), String> {
    let already_captured: Option<String> = db
        .query_row(
            "SELECT baseline_at FROM agent_runs WHERE id=?1",
            [run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if already_captured.is_some() {
        return Ok(());
    }
    let scan = scan_workspace(workspace);
    let manifest = serde_json::to_string(&scan.manifest).map_err(|error| error.to_string())?;
    let error = (!scan.manifest.scan_errors.is_empty()).then(|| {
        clip(&format!(
            "工作区基线扫描不完整：{}",
            scan.manifest.scan_errors.join("; ")
        ))
    });
    db.execute(
        "UPDATE agent_runs SET change_source=?1,baseline_manifest=?2,baseline_at=?3,change_error=?4,updated_at=?3 WHERE id=?5",
        params![scan.source, manifest, now(), error, run_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn prepare_run_baseline(db: &Connection, run_id: &str, workspace: &Path) -> Result<(), String> {
    capture_run_baseline(db, run_id, workspace)
}

fn clip(value: &str) -> String {
    value
        .chars()
        .take(MAX_RESULT_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn normalize_path(value: &str) -> Option<String> {
    let path = PathBuf::from(value.trim());
    if !path.is_dir() {
        return None;
    }
    Some(
        path.canonicalize()
            .unwrap_or(path)
            .to_string_lossy()
            .to_ascii_lowercase(),
    )
}

fn normalized_path_text(value: &str) -> String {
    value.trim().replace('/', "\\").to_ascii_lowercase()
}

fn workspace_slug(value: &str) -> String {
    let normalized = normalized_path_text(value);
    normalized
        .strip_prefix(r"\\?\")
        .unwrap_or(&normalized)
        .replace(':', "")
        .replace('\\', "-")
        .trim_matches('-')
        .to_string()
}

fn cursor_project_slug(source_path: &str) -> Option<String> {
    let normalized = normalized_path_text(source_path);
    let marker = r"\.cursor\projects\";
    let project_root = normalized.find(marker)? + marker.len();
    normalized[project_root..]
        .split('\\')
        .next()
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
}

fn value_text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_string))
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| value_text(payload, keys))
        })
}

fn timestamp_value(value: &Value) -> Option<DateTime<Utc>> {
    for key in [
        "timestamp",
        "created_at",
        "createdAt",
        "time",
        "at",
        "updated_at",
        "updatedAt",
    ] {
        let Some(candidate) = value.get(key) else {
            continue;
        };
        if let Some(text) = candidate.as_str() {
            if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
                return Some(parsed.with_timezone(&Utc));
            }
        }
        if let Some(number) = candidate.as_i64() {
            let millis = if number.abs() < 10_000_000_000 {
                number * 1000
            } else {
                number
            };
            if let Some(parsed) = DateTime::<Utc>::from_timestamp_millis(millis) {
                return Some(parsed);
            }
        }
    }
    value.get("payload").and_then(timestamp_value)
}

fn session_id(value: &Value, path: &Path) -> String {
    for key in [
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
        "trajectory_id",
        "trajectoryId",
    ] {
        if let Some(id) = value
            .get(key)
            .and_then(Value::as_str)
            .filter(|item| !item.trim().is_empty())
        {
            return id.to_string();
        }
    }
    value
        .get("payload")
        .and_then(|payload| {
            let id = session_id(payload, path);
            (id != "unknown-session").then_some(id)
        })
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|item| item.to_str())
                .unwrap_or("unknown-session")
                .to_string()
        })
}

fn role(value: &Value) -> Option<String> {
    value_text(value, &["role"]).or_else(|| value.get("payload").and_then(role))
}

fn collect_strings(value: &Value, keys: &[&str], output: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if keys.iter().any(|wanted| key.eq_ignore_ascii_case(wanted)) {
                    if let Some(text) = child.as_str().filter(|text| !text.trim().is_empty()) {
                        output.push(text.to_string());
                    }
                }
                collect_strings(child, keys, output);
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_strings(item, keys, output)),
        _ => {}
    }
}

fn line_text(value: &Value, raw: &str) -> String {
    let mut texts = Vec::new();
    collect_strings(
        value,
        &[
            "content", "text", "delta", "message", "summary", "command", "output", "detail",
        ],
        &mut texts,
    );
    if texts.is_empty() {
        if raw.trim_start().starts_with('{') {
            String::new()
        } else {
            raw.to_string()
        }
    } else {
        texts.join(" ")
    }
}

fn workspace_from_value(value: &Value) -> Option<String> {
    let mut candidates = Vec::new();
    collect_strings(
        value,
        &[
            "cwd",
            "working_dir",
            "workingDirectory",
            "workspace",
            "workspace_path",
            "workspacePath",
            "project_path",
            "projectPath",
        ],
        &mut candidates,
    );
    candidates
        .into_iter()
        .find_map(|candidate| normalize_path(&candidate))
}

fn path_candidates(text: &str) -> Vec<String> {
    let text = text
        .replace("\\\\", "\\")
        .replace("\\n", "\n")
        .replace("\\r", "");
    text.split_whitespace()
        .filter_map(|part| {
            let candidate =
                part.trim_matches(|character: char| "\"'`.,;:()[]{}<>".contains(character));
            let looks_like_path = candidate.contains(":\\")
                || candidate.starts_with("\\\\")
                || candidate.starts_with("/");
            (looks_like_path && candidate.len() > 3).then(|| candidate.to_string())
        })
        .collect()
}

fn mutating_tool(name: &str) -> bool {
    let name = name.to_ascii_lowercase().replace(['_', '-'], "");
    [
        "write", "edit", "patch", "delete", "rename", "move", "copy", "replace",
    ]
    .iter()
    .any(|tool| name.contains(tool))
}

fn change_kind_for_tool(name: &str) -> ChangeKind {
    let name = name.to_ascii_lowercase().replace(['_', '-'], "");
    if name.contains("delete") || name.contains("remove") {
        ChangeKind::Deleted
    } else if name.contains("create") || name.contains("new") || name.contains("add") {
        ChangeKind::Added
    } else {
        ChangeKind::Modified
    }
}

fn collect_mutating_tool_paths(value: &Value, output: &mut Vec<ChangedFile>) {
    match value {
        Value::Object(map) => {
            let is_tool = map.get("type").and_then(Value::as_str).is_some_and(|kind| {
                let kind = kind.to_ascii_lowercase();
                kind == "tool_use"
                    || kind == "tool-use"
                    || kind == "tool_call"
                    || kind == "tool-call"
            });
            let tool_name = map.get("name").and_then(Value::as_str);
            if is_tool && tool_name.is_some_and(mutating_tool) {
                let kind = change_kind_for_tool(tool_name.unwrap_or_default());
                if let Some(input) = map.get("input") {
                    if let Some(text) = input.as_str() {
                        push_changed_unique(output, extract_file_candidates(text), kind);
                    }
                    let mut paths = Vec::new();
                    collect_strings(
                        input,
                        &["path", "file_path", "filePath", "filename", "target"],
                        &mut paths,
                    );
                    for path in paths {
                        push_changed_unique(output, extract_file_candidates(&path), kind);
                    }
                }
            }
            map.values()
                .for_each(|child| collect_mutating_tool_paths(child, output));
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_mutating_tool_paths(item, output)),
        _ => {}
    }
}

fn merge_change_kind(current: ChangeKind, incoming: ChangeKind) -> ChangeKind {
    match (current, incoming) {
        (_, ChangeKind::Deleted) | (ChangeKind::Deleted, _) => ChangeKind::Deleted,
        (ChangeKind::Added, _) | (_, ChangeKind::Added) => ChangeKind::Added,
        _ => ChangeKind::Modified,
    }
}

fn push_changed_unique(items: &mut Vec<ChangedFile>, values: Vec<String>, kind: ChangeKind) {
    for value in values {
        let key = normalize_text(&value);
        if key.is_empty() {
            continue;
        }
        if let Some(existing) = items
            .iter_mut()
            .find(|item| normalize_text(&item.path) == key)
        {
            existing.kind = merge_change_kind(existing.kind, kind);
        } else {
            items.push(ChangedFile { path: value, kind });
        }
    }
}

fn is_internal_heading(line: &str) -> bool {
    let heading = line
        .trim()
        .trim_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
        .to_ascii_lowercase();
    [
        "planning",
        "inspecting",
        "researching",
        "investigating",
        "exploring",
        "considering",
        "evaluating",
        "analyzing",
        "clarifying",
        "creating",
        "determining",
        "addressing",
        "checking",
        "preparing",
        "reviewing",
    ]
    .iter()
    .any(|prefix| heading.starts_with(prefix))
}

fn clean_assistant_text(text: &str) -> Option<String> {
    let mut kept = Vec::new();
    for line in text.lines() {
        if is_internal_heading(line) {
            if kept.is_empty() {
                return None;
            }
            break;
        }
        let trimmed = line.trim().to_ascii_lowercase();
        if kept.is_empty()
            && [
                "i need to ",
                "i'm thinking",
                "i’ll ",
                "i'll ",
                "i should ",
                "let me ",
                "first, i'll ",
            ]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
        {
            return None;
        }
        kept.push(line);
    }
    let cleaned = kept.join("\n").trim().to_string();
    (!cleaned.is_empty()).then(|| clip(&cleaned))
}

fn heading_text(line: &str) -> String {
    line.trim()
        .trim_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
        .to_ascii_lowercase()
}

fn extract_section(text: &str, names: &[&str]) -> Option<String> {
    let mut collecting = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let heading = heading_text(line);
        if !collecting
            && names
                .iter()
                .any(|name| heading == *name || heading.starts_with(&format!("{name}:")))
        {
            collecting = true;
            continue;
        }
        if collecting && line.trim_start().starts_with('#') {
            break;
        }
        if collecting {
            lines.push(line);
        }
    }
    let value = lines.join("\n").trim().to_string();
    (!value.is_empty()).then(|| clip(&value))
}

fn extract_file_candidates(text: &str) -> Vec<String> {
    let text = text
        .replace("\\\\", "\\")
        .replace("\\n", "\n")
        .replace("\\r", "");
    let mut files = path_candidates(&text);
    for token in text.split_whitespace() {
        let candidate =
            token.trim_matches(|character: char| "-`\"'.,;:()[]{}<>".contains(character));
        if [
            ".css", ".less", ".scss", ".ts", ".tsx", ".js", ".jsx", ".json", ".md", ".woff",
            ".woff2", ".ttf",
        ]
        .iter()
        .any(|extension| candidate.to_ascii_lowercase().ends_with(extension))
        {
            files.push(candidate.to_string());
        }
    }
    files
}

fn change_kind_for_text(text: &str) -> ChangeKind {
    let text = text.to_ascii_lowercase();
    if ["deleted", "removed", "delete", "remove", "删除", "移除"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        ChangeKind::Deleted
    } else if ["added", "created", "new", "新增", "创建", "添加"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        ChangeKind::Added
    } else {
        ChangeKind::Modified
    }
}

fn extract_changed_file_candidates(text: &str) -> Vec<ChangedFile> {
    let mut files = Vec::new();
    for line in text.lines() {
        push_changed_unique(
            &mut files,
            extract_file_candidates(line),
            change_kind_for_text(line),
        );
    }
    files
}

fn display_path(value: &str) -> String {
    value
        .trim()
        .replace("\\\\", "\\")
        .replace("\\n", "")
        .replace("\\r", "")
        .trim_matches(|character: char| "-`\"'.,;:()[]{}<>".contains(character))
        .replace('\\', "/")
}

fn display_workspace_path(value: &str) -> String {
    let path = display_path(value);
    let path = path
        .strip_prefix("//?/")
        .or_else(|| path.strip_prefix("/?/"))
        .unwrap_or(&path);
    path.trim_end_matches('/').to_string()
}

fn relative_to_workspace(path: &str, workspace: &str) -> String {
    let path = display_path(path);
    let workspace = display_workspace_path(workspace);
    if path.is_empty() || workspace.is_empty() {
        return path;
    }

    let path_lower = path.to_ascii_lowercase();
    let workspace_lower = workspace.to_ascii_lowercase();
    if path_lower == workspace_lower {
        return ".".into();
    }
    let prefix = format!("{workspace_lower}/");
    if path_lower.starts_with(&prefix) {
        path[workspace.len() + 1..].to_string()
    } else {
        path
    }
}

fn relative_file_list(files: Vec<ChangedFile>, workspace: &str) -> String {
    let mut relative = Vec::new();
    for file in files {
        let path = relative_to_workspace(&file.path, workspace);
        if path.is_empty() {
            continue;
        }
        if let Some(existing) = relative
            .iter_mut()
            .find(|item: &&mut ChangedFile| normalize_text(&item.path) == normalize_text(&path))
        {
            existing.kind = merge_change_kind(existing.kind, file.kind);
        } else {
            relative.push(ChangedFile {
                path,
                kind: file.kind,
            });
        }
    }
    clip(
        &relative
            .iter()
            .map(|file| format!("{} {}", file.kind.marker(), file.path))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn format_intermediate_files(
    touched: &[ChangedFile],
    final_files: &[ChangedFile],
    workspace: &str,
) -> String {
    let final_paths: HashSet<String> = final_files
        .iter()
        .map(|file| snapshot_key(&relative_to_workspace(&file.path, workspace)))
        .collect();
    let mut paths = Vec::new();
    for file in touched {
        let path = relative_to_workspace(&file.path, workspace);
        let key = snapshot_key(&path);
        if path.is_empty()
            || final_paths.contains(&key)
            || paths.iter().any(|item: &String| snapshot_key(item) == key)
        {
            continue;
        }
        paths.push(path);
    }
    clip(&paths.join("\n"))
}

fn diff_manifests(baseline: &BaselineManifest, current: &BaselineManifest) -> Vec<ChangedFile> {
    let incomplete = !baseline.scan_errors.is_empty() || !current.scan_errors.is_empty();
    let mut files = Vec::new();
    for (key, baseline_file) in &baseline.files {
        match current.files.get(key) {
            Some(current_file)
                if current_file.hash != baseline_file.hash
                    || current_file.size != baseline_file.size =>
            {
                files.push(ChangedFile {
                    path: current_file.path.clone(),
                    kind: ChangeKind::Modified,
                });
            }
            None if !incomplete => files.push(ChangedFile {
                path: if baseline_file.path.is_empty() {
                    key.clone()
                } else {
                    baseline_file.path.clone()
                },
                kind: ChangeKind::Deleted,
            }),
            _ => {}
        }
    }
    for (key, current_file) in &current.files {
        if !baseline.files.contains_key(key) && !incomplete {
            files.push(ChangedFile {
                path: current_file.path.clone(),
                kind: ChangeKind::Added,
            });
        }
    }
    files.sort_by_key(|file| snapshot_key(&file.path));
    files
}

fn final_change_for_run(
    db: &Connection,
    run_id: &str,
    session: &HistorySession,
    workspace: &str,
) -> Result<FinalChangeResult, String> {
    let (source, baseline_json, baseline_error): (String, String, Option<String>) = db
        .query_row(
            "SELECT change_source,baseline_manifest,change_error FROM agent_runs WHERE id=?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| error.to_string())?;
    if source == "legacy_history" || baseline_json.trim().is_empty() || workspace.trim().is_empty()
    {
        return Ok(FinalChangeResult {
            source,
            files: Vec::new(),
            intermediate_files: String::new(),
            error: Some("未记录启动时工作区基线，当前结果仅来自 Agent 历史".into()),
        });
    }
    let baseline: BaselineManifest = match serde_json::from_str(&baseline_json) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Ok(FinalChangeResult {
                source: "legacy_history".into(),
                files: Vec::new(),
                intermediate_files: String::new(),
                error: Some(format!("工作区基线格式无效：{error}")),
            });
        }
    };
    let current = scan_workspace(Path::new(workspace));
    let files = diff_manifests(&baseline, &current.manifest);
    let mut errors = Vec::new();
    if let Some(error) = baseline_error.filter(|error| !error.trim().is_empty()) {
        errors.push(error);
    }
    errors.extend(baseline.scan_errors);
    errors.extend(current.manifest.scan_errors);
    let error = (!errors.is_empty()).then(|| clip(&errors.join("; ")));
    Ok(FinalChangeResult {
        source,
        intermediate_files: format_intermediate_files(&session.changed_files, &files, workspace),
        files,
        error,
    })
}

fn result_summary(session: &HistorySession) -> String {
    if let Some(summary) = extract_section(&session.assistant, &["summary", "摘要", "result"]) {
        return summary;
    }
    let leading = session
        .assistant
        .lines()
        .take_while(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if leading.is_empty() {
        session.assistant.clone()
    } else {
        clip(&leading)
    }
}

fn result_changed_files(session: &HistorySession, workspace: &str) -> String {
    let section = extract_section(
        &session.assistant,
        &["changed files", "改动文件", "修改文件", "产物"],
    );
    let mut files = section
        .as_deref()
        .map(extract_changed_file_candidates)
        .unwrap_or_default();
    for file in &session.changed_files {
        push_changed_unique(&mut files, vec![file.path.clone()], file.kind);
    }
    relative_file_list(files, workspace)
}

fn result_verification(session: &HistorySession) -> String {
    if let Some(section) = extract_section(
        &session.assistant,
        &["verification", "验证结果", "验证通过", "tests"],
    ) {
        return section;
    }
    let lines: Vec<String> = session
        .assistant
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            let command = [
                "pnpm ", "npm ", "cargo ", "yarn ", "pytest", "vitest", "jest",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
            let result = [
                "pass", "passed", "success", "通过", "fail", "failed", "失败",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
            command && result
        })
        .map(str::to_string)
        .collect();
    clip(&lines.join("\n"))
}

fn result_unresolved(session: &HistorySession) -> String {
    extract_section(
        &session.assistant,
        &["unresolved issues", "未解决问题", "阻塞问题", "blockers"],
    )
    .unwrap_or_default()
}

fn result_signal_count(session: &HistorySession) -> usize {
    [
        extract_section(&session.assistant, &["summary", "摘要", "result"]),
        extract_section(
            &session.assistant,
            &["changed files", "改动文件", "修改文件", "产物", "artifacts"],
        ),
        extract_section(
            &session.assistant,
            &["verification", "验证结果", "验证通过", "tests"],
        ),
        extract_section(
            &session.assistant,
            &["unresolved issues", "未解决问题", "阻塞问题", "blockers"],
        ),
    ]
    .into_iter()
    .flatten()
    .count()
}

fn merge_history_session(session: &HistorySession, all: &[HistorySession]) -> HistorySession {
    let mut merged = session.clone();
    for continuation in all.iter().filter(|item| {
        item.source_path == session.source_path
            && item.session_id == session.session_id
            && item.prompt != session.prompt
    }) {
        for file in &continuation.changed_files {
            push_changed_unique(
                &mut merged.changed_files,
                vec![file.path.clone()],
                file.kind,
            );
        }
        if result_signal_count(continuation) > result_signal_count(&merged)
            || (result_signal_count(continuation) == result_signal_count(&merged)
                && !continuation.assistant.is_empty())
        {
            merged.assistant = continuation.assistant.clone();
        }
        merged.raw_excerpt = clip(&format!(
            "{}\n{}",
            merged.raw_excerpt, continuation.raw_excerpt
        ));
        if merged.started_at.is_none()
            || continuation
                .started_at
                .is_some_and(|at| Some(at) < merged.started_at)
        {
            merged.started_at = continuation.started_at;
        }
        if continuation.ended_at > merged.ended_at {
            merged.ended_at = continuation.ended_at;
        }
    }
    merged
}

fn new_history_session(agent: &str, path: &Path, modified: DateTime<Utc>) -> HistorySession {
    HistorySession {
        agent: agent.to_string(),
        session_id: path
            .file_stem()
            .and_then(|item| item.to_str())
            .unwrap_or("unknown-session")
            .to_string(),
        source_path: path.to_string_lossy().to_string(),
        started_at: None,
        ended_at: Some(modified),
        ..Default::default()
    }
}

fn push_history_session(sessions: &mut Vec<HistorySession>, session: Option<HistorySession>) {
    if let Some(mut session) = session {
        session.raw_excerpt = clip(&session.raw_excerpt);
        if !session.prompt.is_empty()
            || !session.assistant.is_empty()
            || !session.changed_files.is_empty()
        {
            sessions.push(session);
        }
    }
}

fn parse_history_file(agent: &str, path: &Path) -> Option<Vec<HistorySession>> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now);
    let content = if metadata.len() <= MAX_HISTORY_FILE_BYTES {
        fs::read_to_string(path).ok()?
    } else {
        let mut file = fs::File::open(path).ok()?;
        let mut head = vec![0; HISTORY_HEAD_BYTES.min(metadata.len() as usize)];
        file.read_exact(&mut head).ok()?;
        let tail_size = MAX_HISTORY_FILE_BYTES as usize - head.len();
        file.seek(SeekFrom::End(-(tail_size as i64))).ok()?;
        let mut tail = vec![0; tail_size];
        file.read_exact(&mut tail).ok()?;
        String::from_utf8_lossy(&head).into_owned() + "\n" + &String::from_utf8_lossy(&tail)
    };
    let mut sessions = Vec::new();
    let mut session = None;
    for raw in content.lines() {
        let value = serde_json::from_str::<Value>(raw).ok();
        let Some(value) = value.as_ref() else {
            continue;
        };
        let current_role = role(value).map(|item| item.to_ascii_lowercase());
        if current_role.as_deref() == Some("user") {
            push_history_session(&mut sessions, session.take());
            let mut next = new_history_session(agent, path, modified);
            next.session_id = session_id(value, path);
            next.workspace_path = workspace_from_value(value);
            next.prompt = clip(&line_text(value, raw));
            next.raw_excerpt = next.prompt.clone();
            if let Some(at) = timestamp_value(value) {
                next.started_at = Some(at);
                next.ended_at = Some(at);
            }
            session = Some(next);
            continue;
        }
        let Some(session) = session.as_mut() else {
            continue;
        };
        session.session_id = session_id(value, path);
        if session.workspace_path.is_none() {
            session.workspace_path = workspace_from_value(value);
        }
        if let Some(at) = timestamp_value(value) {
            if session.started_at.is_none_or(|current| at < current) {
                session.started_at = Some(at);
            }
            if session.ended_at.is_none_or(|current| at > current) {
                session.ended_at = Some(at);
            }
        }
        let text = clip(&line_text(value, raw));
        if current_role.as_deref() == Some("assistant") {
            if let Some(cleaned) = clean_assistant_text(&text) {
                session.assistant = cleaned;
            }
        }
        let mut tool_files = Vec::new();
        collect_mutating_tool_paths(value, &mut tool_files);
        for file in tool_files {
            push_changed_unique(&mut session.changed_files, vec![file.path], file.kind);
        }
        if !text.is_empty() {
            session.raw_excerpt = clip(&format!("{}\n{}", session.raw_excerpt, text));
        }
    }
    push_history_session(&mut sessions, session.take());
    (!sessions.is_empty()).then_some(sessions)
}

fn history_roots() -> [(&'static str, PathBuf); 3] {
    let home = super::home();
    [
        ("Codex", home.join(".codex\\sessions")),
        ("Claude", home.join(".claude\\projects")),
        ("Cursor", home.join(".cursor\\projects")),
    ]
}

fn read_history(runs: &[AgentRun]) -> Vec<HistorySession> {
    let eligible: Vec<&AgentRun> = runs
        .iter()
        .filter(|run| {
            run.task_id.is_some()
                && !run.prompt_snapshot.trim().is_empty()
                && run.result_state != "saved"
        })
        .collect();
    let Some(cutoff) = eligible
        .iter()
        .filter_map(|run| parse_at(&run.created_at))
        .min()
        .map(|at| at - Duration::minutes(5))
    else {
        return Vec::new();
    };
    let agents: HashSet<String> = eligible
        .iter()
        .map(|run| run.agent.to_ascii_lowercase())
        .collect();
    let mut sessions = Vec::new();
    for (agent, root) in history_roots() {
        if !agents.contains(&agent.to_ascii_lowercase()) {
            continue;
        }
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(root)
            .max_depth(10)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let extension = entry
                .path()
                .extension()
                .and_then(|item| item.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(extension.as_str(), "jsonl" | "json") {
                continue;
            }
            if entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .map(DateTime::<Utc>::from)
                .is_some_and(|at| at < cutoff)
            {
                continue;
            }
            if let Some(file_sessions) = parse_history_file(agent, entry.path()) {
                sessions.extend(
                    file_sessions
                        .into_iter()
                        .filter(|session| session.ended_at.is_some_and(|at| at >= cutoff)),
                );
            }
        }
    }
    sessions
}

fn similarity(left: &str, right: &str) -> f32 {
    let left = normalize_text(left);
    let right = normalize_text(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left.contains(&right) || right.contains(&left) {
        return 1.0;
    }
    let left_tokens: HashSet<&str> = left
        .split_whitespace()
        .filter(|token| token.len() > 2)
        .collect();
    let right_tokens: HashSet<&str> = right
        .split_whitespace()
        .filter(|token| token.len() > 2)
        .collect();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    left_tokens.intersection(&right_tokens).count() as f32
        / left_tokens.len().max(right_tokens.len()) as f32
}

fn parse_at(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRun> {
    Ok(AgentRun {
        id: row.get(0)?,
        task_id: row.get(1)?,
        agent: row.get(2)?,
        workspace_path: row.get(3)?,
        window_mode: row.get(4)?,
        transport: row.get(5)?,
        window_handle: row.get(6)?,
        prompt_snapshot: row.get(7)?,
        status: row.get(8)?,
        error_message: row.get(9)?,
        session_id: row.get(10)?,
        match_state: row.get(11)?,
        result_state: row.get(12)?,
        result_summary: row.get(13)?,
        changed_files: row.get(14)?,
        verification: row.get(15)?,
        unresolved_issues: row.get(16)?,
        raw_excerpt: row.get(17)?,
        result_source_path: row.get(18)?,
        completed_at: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        change_source: row.get(22)?,
        baseline_at: row.get(23)?,
        intermediate_files: row.get(24)?,
        change_error: row.get(25)?,
    })
}

const RUN_SELECT: &str = "SELECT id,task_id,agent,workspace_path,window_mode,transport,window_handle,prompt_snapshot,status,error_message,session_id,match_state,result_state,result_summary,changed_files,verification,unresolved_issues,raw_excerpt,result_source_path,completed_at,created_at,updated_at,change_source,baseline_at,intermediate_files,change_error FROM agent_runs";

fn insert_run(
    db: &Connection,
    id: &str,
    task_id: Option<i64>,
    agent: &str,
    workspace: &str,
    transport: &str,
    prompt: &str,
    status: &str,
) -> Result<(), String> {
    let timestamp = now();
    db.execute("INSERT INTO agent_runs(id,task_id,agent,workspace_path,window_mode,transport,prompt_snapshot,status,match_state,result_state,created_at,updated_at) VALUES(?1,?2,?3,?4,'new',?5,?6,?7,'matched','none',?8,?8)", params![id, task_id, agent, workspace, transport, prompt, status, timestamp]).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn create_cursor_run(
    db: &Connection,
    id: &str,
    task_id: Option<i64>,
    workspace: &str,
    mode: &str,
    transport: &str,
    handle: Option<i64>,
    prompt: &str,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    let timestamp = now();
    db.execute("INSERT INTO agent_runs(id,task_id,agent,workspace_path,window_mode,transport,window_handle,prompt_snapshot,status,error_message,match_state,result_state,created_at,updated_at) VALUES(?1,?2,'Cursor',?3,?4,?5,?6,?7,?8,?9,'matched','none',?10,?10) ON CONFLICT(id) DO UPDATE SET window_mode=excluded.window_mode,transport=excluded.transport,window_handle=excluded.window_handle,status=excluded.status,error_message=excluded.error_message,updated_at=excluded.updated_at", params![id, task_id, workspace, mode, transport, handle, prompt, status, error, timestamp]).map_err(|error| error.to_string())?;
    Ok(())
}

fn spawn_agent(executable: &Path, workspace: Option<&Path>, agent: &str) -> Result<(), String> {
    let cwd_arg = workspace
        .map(|path| format!(" -WorkingDirectory {}", super::powershell_literal(path)))
        .unwrap_or_default();
    let extension = executable
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let script = if matches!(extension.as_str(), "cmd" | "bat") {
        format!(
            "Start-Process -FilePath $env:ComSpec{} -ArgumentList @('/D','/C',{})",
            cwd_arg,
            super::powershell_argument_literal(executable)
        )
    } else if extension == "ps1" {
        format!("Start-Process -FilePath 'powershell.exe'{} -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',{})", cwd_arg, super::powershell_argument_literal(executable))
    } else {
        format!(
            "Start-Process -FilePath {}{}",
            super::powershell_literal(executable),
            cwd_arg
        )
    };
    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .spawn()
        .map_err(|error| format!("启动 {agent} 失败：{error}"))?;
    Ok(())
}

#[tauri::command]
pub fn start_agent_run(
    request: AgentRunStartRequest,
    db: State<super::Db>,
) -> Result<AgentRunStartResult, String> {
    let agent = request.agent.trim();
    let command = match agent.to_ascii_lowercase().as_str() {
        "codex" => "codex",
        "claude" => "claude",
        _ => return Err("通用启动接口仅支持 Codex 或 Claude".into()),
    };
    if request.prompt.trim().is_empty() {
        return Err("Prompt 不能为空".into());
    }
    let workspace = request
        .workspace_path
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            let path = PathBuf::from(value)
                .canonicalize()
                .map_err(|_| "工作目录不存在或无权访问".to_string())?;
            if !path.is_dir() {
                return Err("工作目录必须是文件夹".into());
            }
            Ok::<PathBuf, String>(path)
        })
        .transpose()?;
    let executable = PathBuf::from(
        super::first_command_path(command)
            .ok_or_else(|| format!("未找到 {agent} 命令，请先检查 PATH 或安装配置"))?,
    );
    let run_id = format!(
        "{}-{}",
        command,
        super::fnv_hash(&format!(
            "{}:{}",
            request.prompt,
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    );
    {
        let db = super::lock(&db)?;
        let workspace_text = workspace
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        insert_run(
            &db,
            &run_id,
            request.task_id,
            agent,
            &workspace_text,
            "agent_process",
            &request.prompt,
            "prepared",
        )?;
        if let Some(path) = workspace.as_deref() {
            prepare_run_baseline(&db, &run_id, path)?;
        }
        db.execute(
            "UPDATE agent_runs SET status='launching',updated_at=?1 WHERE id=?2",
            params![now(), run_id],
        )
        .map_err(|error| error.to_string())?;
    }
    if let Err(error) = spawn_agent(&executable, workspace.as_deref(), agent) {
        let db = super::lock(&db)?;
        db.execute(
            "UPDATE agent_runs SET status='failed',error_message=?1,updated_at=?2 WHERE id=?3",
            params![error, now(), run_id],
        )
        .map_err(|item| item.to_string())?;
        return Err(error);
    }
    Ok(AgentRunStartResult {
        run_id,
        status: "launching".into(),
        transport: "agent_process".into(),
        error: None,
    })
}

fn session_result(
    session: &HistorySession,
    workspace: &str,
) -> (String, String, String, String, String) {
    let summary = if session.assistant.trim().is_empty() {
        String::new()
    } else {
        result_summary(session)
    };
    (
        clip(&summary),
        result_changed_files(session, workspace),
        result_verification(session),
        result_unresolved(session),
        clip(&session.raw_excerpt),
    )
}

fn update_with_session(
    db: &Connection,
    run_id: &str,
    session: &HistorySession,
    match_state: &str,
) -> Result<(), String> {
    let workspace: String = db
        .query_row(
            "SELECT workspace_path FROM agent_runs WHERE id=?1",
            [run_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let (summary, history_changed_files, verification, unresolved, raw_excerpt) =
        session_result(session, &workspace);
    let final_change = final_change_for_run(db, run_id, session, &workspace)?;
    let changed_files = if final_change.source == "legacy_history" {
        history_changed_files
    } else {
        relative_file_list(final_change.files.clone(), &workspace)
    };
    db.execute("UPDATE agent_runs SET session_id=?1,match_state=?2,result_state=CASE WHEN result_state='saved' THEN result_state ELSE 'draft' END,result_summary=CASE WHEN result_state='saved' THEN result_summary ELSE ?3 END,changed_files=CASE WHEN result_state='saved' THEN changed_files ELSE ?4 END,verification=CASE WHEN result_state='saved' THEN verification ELSE ?5 END,unresolved_issues=CASE WHEN result_state='saved' THEN unresolved_issues ELSE ?6 END,raw_excerpt=?7,result_source_path=?8,status=CASE WHEN status IN ('failed','fallback') THEN status ELSE 'result_ready' END,completed_at=?9,updated_at=?10,change_source=?11,intermediate_files=?12,change_error=?13 WHERE id=?14", params![session.session_id, match_state, summary, changed_files, verification, unresolved, raw_excerpt, session.source_path, session.ended_at.map(|at| at.to_rfc3339()), now(), final_change.source, final_change.intermediate_files, final_change.error]).map_err(|error| error.to_string())?;
    Ok(())
}

fn update_baseline_only(db: &Connection, run: &AgentRun) -> Result<(), String> {
    let session = HistorySession {
        agent: run.agent.clone(),
        session_id: run.session_id.clone().unwrap_or_default(),
        ..Default::default()
    };
    let final_change = final_change_for_run(db, &run.id, &session, &run.workspace_path)?;
    if final_change.source == "legacy_history" {
        return Ok(());
    }
    let changed_files = relative_file_list(final_change.files, &run.workspace_path);
    db.execute(
        "UPDATE agent_runs SET changed_files=CASE WHEN result_state='saved' THEN changed_files ELSE ?1 END,change_source=?2,intermediate_files=?3,change_error=?4,updated_at=?5 WHERE id=?6",
        params![
            changed_files,
            final_change.source,
            final_change.intermediate_files,
            final_change.error,
            now(),
            run.id
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn has_active_workspace_overlap(run: &AgentRun, runs: &[AgentRun]) -> bool {
    if run.workspace_path.trim().is_empty()
        || run.completed_at.is_some()
        || matches!(run.status.as_str(), "failed" | "fallback" | "result_ready")
    {
        return false;
    }
    let workspace = normalized_path_text(&run.workspace_path);
    runs.iter().any(|other| {
        other.id != run.id
            && other.completed_at.is_none()
            && !matches!(
                other.status.as_str(),
                "failed" | "fallback" | "result_ready"
            )
            && normalized_path_text(&other.workspace_path) == workspace
    })
}

fn record_workspace_overlap_warning(db: &Connection, run_id: &str) -> Result<(), String> {
    let warning = "同一工作区存在并行运行记录，最终差异可能无法唯一归属";
    db.execute(
        "UPDATE agent_runs SET change_error=CASE WHEN change_error IS NULL OR change_error='' THEN ?1 WHEN instr(change_error,?1)>0 THEN change_error ELSE change_error||'; '||?1 END,updated_at=?2 WHERE id=?3",
        params![warning, now(), run_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn candidate_score(run: &AgentRun, session: &HistorySession) -> Option<i32> {
    if run.task_id.is_none()
        || run.prompt_snapshot.trim().is_empty()
        || !run.agent.eq_ignore_ascii_case(&session.agent)
    {
        return None;
    }
    let created = parse_at(&run.created_at)?;
    let ended = session.ended_at.unwrap_or_else(Utc::now);
    if ended < created - Duration::minutes(5) {
        return None;
    }
    let mut score = 2;
    let run_workspace = normalize_path(&run.workspace_path);
    match (
        run_workspace,
        session.workspace_path.as_deref().and_then(normalize_path),
    ) {
        (Some(left), Some(right)) if left == right => score += 5,
        (Some(_), Some(_)) => return None,
        (None, Some(_)) => score += 1,
        (_, None) => score += 1,
    }
    if run.agent.eq_ignore_ascii_case("Cursor")
        && workspace_slug(&run.workspace_path)
            == cursor_project_slug(&session.source_path).unwrap_or_default()
    {
        score += 5;
    }
    let prompt_score = similarity(&run.prompt_snapshot, &session.prompt);
    if prompt_score >= 0.85 {
        score += 4;
    } else if prompt_score >= 0.15 {
        score += 2;
    } else if !session.prompt.is_empty() {
        return None;
    }
    if session
        .started_at
        .is_some_and(|started| started <= created + Duration::minutes(5))
    {
        score += 2;
    } else if session.started_at.is_none() && session.ended_at.is_some_and(|ended| ended >= created)
    {
        score += 1;
    }
    Some(score)
}

fn refresh_locked(db: &Connection) -> Result<(), String> {
    db.execute("DELETE FROM agent_runs WHERE transport='history'", [])
        .map_err(|error| error.to_string())?;
    let existing = list_locked(db, None)?;
    let sessions = read_history(&existing);
    let mut by_source = HashMap::new();
    let mut claimed_runs = HashSet::new();
    for run in &existing {
        if run.task_id.is_none()
            || run.prompt_snapshot.trim().is_empty()
            || run.result_state == "saved"
        {
            continue;
        }
        if let Some(source) = &run.result_source_path {
            by_source.insert(source.clone(), run.id.clone());
        }
        if let Some(session) = &run.session_id {
            by_source.insert(
                format!("{}:{}", run.agent.to_ascii_lowercase(), session),
                run.id.clone(),
            );
        }
    }
    for index in 0..sessions.len() {
        let session = &sessions[index];
        let direct_id = by_source.get(&session.source_path).cloned().or_else(|| {
            by_source
                .get(&format!(
                    "{}:{}",
                    session.agent.to_ascii_lowercase(),
                    session.session_id
                ))
                .cloned()
        });
        if let Some(run_id) = direct_id {
            let direct_confidence = existing
                .iter()
                .find(|run| run.id == run_id)
                .and_then(|run| candidate_score(run, &session))
                .unwrap_or_default();
            if !claimed_runs.contains(&run_id) && direct_confidence >= 7 {
                let merged = merge_history_session(session, &sessions);
                update_with_session(db, &run_id, &merged, "matched")?;
                claimed_runs.insert(run_id);
                continue;
            }
        }
        let mut candidates: Vec<(i32, &AgentRun)> = existing
            .iter()
            .filter(|run| !claimed_runs.contains(&run.id))
            .filter_map(|run| candidate_score(run, &session).map(|score| (score, run)))
            .collect();
        candidates.sort_by(|left, right| right.0.cmp(&left.0));
        if !candidates.first().is_some_and(|best| {
            best.0 >= 9 && candidates.get(1).is_none_or(|next| best.0 - next.0 >= 2)
        }) {
            continue;
        }
        let match_id = {
            let merged = merge_history_session(session, &sessions);
            update_with_session(db, &candidates[0].1.id, &merged, "matched")?;
            claimed_runs.insert(candidates[0].1.id.clone());
            candidates[0].1.id.clone()
        };
        by_source.insert(session.source_path.clone(), match_id);
    }
    for run in &existing {
        if run.task_id.is_some()
            && run.baseline_at.is_some()
            && run.result_state != "saved"
            && !claimed_runs.contains(&run.id)
        {
            update_baseline_only(db, run)?;
        }
    }
    for run in &existing {
        if has_active_workspace_overlap(run, &existing) {
            record_workspace_overlap_warning(db, &run.id)?;
        }
    }
    Ok(())
}

fn list_locked(db: &Connection, task_id: Option<i64>) -> Result<Vec<AgentRun>, String> {
    let query = match task_id {
        Some(_) => format!("{} WHERE task_id=?1 ORDER BY created_at DESC", RUN_SELECT),
        None => format!("{} ORDER BY created_at DESC", RUN_SELECT),
    };
    let mut statement = db.prepare(&query).map_err(|error| error.to_string())?;
    let rows = match task_id {
        Some(id) => statement
            .query_map([id], run_row)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
        None => statement
            .query_map([], run_row)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
    };
    rows.map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_agent_runs(
    task_id: Option<i64>,
    db: State<super::Db>,
) -> Result<Vec<AgentRun>, String> {
    let db = super::lock(&db)?;
    list_locked(&db, task_id)
}

#[tauri::command]
pub fn refresh_agent_runs(db: State<super::Db>) -> Result<(), String> {
    let db = super::lock(&db)?;
    refresh_locked(&db)
}

#[tauri::command]
pub fn save_agent_run_result(
    input: AgentRunResultInput,
    db: State<super::Db>,
) -> Result<AgentRun, String> {
    let db = super::lock(&db)?;
    if db
        .query_row(
            "SELECT id FROM agent_runs WHERE id=?1",
            [&input.run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Err("执行记录不存在".into());
    }
    db.execute("UPDATE agent_runs SET result_state='saved',result_summary=?1,changed_files=?2,verification=?3,unresolved_issues=?4,updated_at=?5 WHERE id=?6", params![clip(&input.result_summary), clip(&input.changed_files), clip(&input.verification), clip(&input.unresolved_issues), now(), input.run_id]).map_err(|error| error.to_string())?;
    db.query_row(
        &format!("{} WHERE id=?1", RUN_SELECT),
        [&input.run_id],
        run_row,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn resolve_agent_run(
    input: ResolveAgentRunInput,
    db: State<super::Db>,
) -> Result<AgentRun, String> {
    let db = super::lock(&db)?;
    match input.action.as_str() {
        "link" => {
            let task_id = input
                .task_id
                .ok_or_else(|| "关联任务不能为空".to_string())?;
            let exists: Option<i64> = db
                .query_row("SELECT id FROM tasks WHERE id=?1", [task_id], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|error| error.to_string())?;
            if exists.is_none() {
                return Err("任务不存在".into());
            }
            db.execute(
                "UPDATE agent_runs SET task_id=?1,match_state='matched',updated_at=?2 WHERE id=?3",
                params![task_id, now(), input.run_id],
            )
            .map_err(|error| error.to_string())?;
        }
        "ignore" => {
            db.execute(
                "UPDATE agent_runs SET match_state='ignored',updated_at=?1 WHERE id=?2",
                params![now(), input.run_id],
            )
            .map_err(|error| error.to_string())?;
        }
        _ => return Err("不支持的执行记录操作".into()),
    }
    db.query_row(
        &format!("{} WHERE id=?1", RUN_SELECT),
        [&input.run_id],
        run_row,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_long_results() {
        assert_eq!(
            clip(&"x".repeat(MAX_RESULT_CHARS + 20)).chars().count(),
            MAX_RESULT_CHARS
        );
    }

    #[test]
    fn prompt_similarity_handles_markdown_context() {
        assert!(
            similarity(
                "Fix the parser and run cargo test",
                "Fix parser run cargo test"
            ) > 0.15
        );
        assert_eq!(similarity("alpha", "beta"), 0.0);
    }

    #[test]
    fn result_draft_ignores_internal_planning_and_requires_explicit_sections() {
        assert_eq!(
            clean_assistant_text("**Planning file inspection**\n\nI need to inspect files"),
            None
        );
        let session = HistorySession { assistant: "已完成字体子集化。\n\n## 改动文件\n- `src/styles/global.css`\n\n## 验证结果\n- `pnpm build` passed\n\n## 未解决问题\n- 无".into(), ..Default::default() };
        assert_eq!(result_summary(&session), "已完成字体子集化。");
        assert_eq!(
            result_changed_files(&session, ""),
            "~ src/styles/global.css"
        );
        assert_eq!(result_verification(&session), "- `pnpm build` passed");
        assert_eq!(result_unresolved(&session), "- 无");
    }

    #[test]
    fn history_parser_splits_multiple_user_turns() {
        let path =
            std::env::temp_dir().join(format!("agent-run-history-{}.jsonl", std::process::id()));
        let content = concat!(
            r#"{"role":"user","message":{"content":[{"type":"text","text":"task one"}]}}"#,
            "\n",
            r#"{"role":"assistant","message":{"content":[{"type":"text","text":"**Planning inspection**\\n\\nI need to inspect files"}]}}"#,
            "\n",
            r#"{"role":"assistant","message":{"content":[{"type":"text","text":"Task one is complete."}]}}"#,
            "\n",
            r#"{"role":"user","message":{"content":[{"type":"text","text":"task two"}]}}"#,
            "\n",
            r#"{"role":"assistant","message":{"content":[{"type":"text","text":"Task two is complete."}]}}"#,
        );
        std::fs::write(&path, content).unwrap();
        let sessions = parse_history_file("Cursor", &path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].prompt, "task one");
        assert_eq!(sessions[0].assistant, "Task one is complete.");
        assert_eq!(sessions[1].prompt, "task two");
        assert_eq!(sessions[1].assistant, "Task two is complete.");
    }

    #[test]
    fn changed_files_only_include_explicit_mutating_tool_paths() {
        let write_file = serde_json::json!({"type": "tool_use", "name": "WriteFile", "input": "{'path': 'e:\\\\Projects\\\\jupiter\\\\src\\\\global.css'}"});
        let read_file = serde_json::json!({"type": "tool_use", "name": "ReadFile", "input": "{'path': 'e:\\\\Projects\\\\jupiter\\\\src\\\\readme.md'}"});
        let delete_file = serde_json::json!({"type": "tool_use", "name": "Delete", "input": "{'path': 'e:\\\\Projects\\\\jupiter\\\\src\\\\old.css'}"});
        let mut paths = Vec::new();
        collect_mutating_tool_paths(&write_file, &mut paths);
        collect_mutating_tool_paths(&read_file, &mut paths);
        collect_mutating_tool_paths(&delete_file, &mut paths);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].path, r"e:\Projects\jupiter\src\global.css");
        assert_eq!(paths[0].kind, ChangeKind::Modified);
        assert_eq!(paths[1].path, r"e:\Projects\jupiter\src\old.css");
        assert_eq!(paths[1].kind, ChangeKind::Deleted);
    }

    #[test]
    fn changed_files_are_relative_to_run_workspace() {
        let session = HistorySession {
            changed_files: vec![
                ChangedFile {
                    path: r"e:\Projects\jupiter\awa-community-web\package.json".into(),
                    kind: ChangeKind::Added,
                },
                ChangedFile {
                    path: r"e:\Projects\jupiter\awa-community-web\scripts\subset-noto-sans-sc.mjs"
                        .into(),
                    kind: ChangeKind::Modified,
                },
                ChangedFile {
                    path: "NotoSansSC-VariableFont_wght.woff2".into(),
                    kind: ChangeKind::Added,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            result_changed_files(&session, r"\\?\E:\Projects\jupiter\awa-community-web"),
            "+ package.json\n~ scripts/subset-noto-sans-sc.mjs\n+ NotoSansSC-VariableFont_wght.woff2"
        );
    }

    #[test]
    fn changed_file_markers_merge_and_parse_explicit_statuses() {
        let session = HistorySession {
            assistant: "## Changed Files\n- Added: `src/new.ts`\n- Modified: `src/existing.ts`\n- Deleted: `src/old.ts`".into(),
            ..Default::default()
        };
        assert_eq!(
            result_changed_files(&session, ""),
            "+ src/new.ts\n~ src/existing.ts\n- src/old.ts"
        );
    }

    #[test]
    fn baseline_diff_contains_only_final_changes() {
        let baseline = BaselineManifest {
            files: BTreeMap::from([
                (
                    snapshot_key("src/keep.ts"),
                    FileFingerprint {
                        path: "src/keep.ts".into(),
                        hash: "same".into(),
                        size: 4,
                    },
                ),
                (
                    snapshot_key("src/modified.ts"),
                    FileFingerprint {
                        path: "src/modified.ts".into(),
                        hash: "old".into(),
                        size: 3,
                    },
                ),
                (
                    snapshot_key("src/deleted.ts"),
                    FileFingerprint {
                        path: "src/deleted.ts".into(),
                        hash: "gone".into(),
                        size: 4,
                    },
                ),
            ]),
            scan_errors: Vec::new(),
        };
        let current = BaselineManifest {
            files: BTreeMap::from([
                (
                    snapshot_key("src/keep.ts"),
                    FileFingerprint {
                        path: "src/keep.ts".into(),
                        hash: "same".into(),
                        size: 4,
                    },
                ),
                (
                    snapshot_key("src/modified.ts"),
                    FileFingerprint {
                        path: "src/modified.ts".into(),
                        hash: "new".into(),
                        size: 3,
                    },
                ),
                (
                    snapshot_key("src/new.ts"),
                    FileFingerprint {
                        path: "src/new.ts".into(),
                        hash: "new-file".into(),
                        size: 8,
                    },
                ),
            ]),
            scan_errors: Vec::new(),
        };
        let changes = diff_manifests(&baseline, &current);
        assert_eq!(
            changes
                .iter()
                .map(|file| format!("{} {}", file.kind.marker(), file.path))
                .collect::<Vec<_>>(),
            vec!["- src/deleted.ts", "~ src/modified.ts", "+ src/new.ts"]
        );
    }

    #[test]
    fn baseline_diff_omits_reverted_and_create_delete_files() {
        let baseline = BaselineManifest {
            files: BTreeMap::from([(
                snapshot_key("src/stable.ts"),
                FileFingerprint {
                    path: "src/stable.ts".into(),
                    hash: "same".into(),
                    size: 4,
                },
            )]),
            scan_errors: Vec::new(),
        };
        let current = baseline.clone();
        assert!(diff_manifests(&baseline, &current).is_empty());
    }

    #[test]
    fn incomplete_snapshot_never_fabricates_additions_or_deletions() {
        let baseline = BaselineManifest {
            files: BTreeMap::from([(
                snapshot_key("src/maybe-deleted.ts"),
                FileFingerprint {
                    path: "src/maybe-deleted.ts".into(),
                    hash: "old".into(),
                    size: 3,
                },
            )]),
            scan_errors: vec!["permission denied".into()],
        };
        let current = BaselineManifest::default();
        assert!(diff_manifests(&baseline, &current).is_empty());
    }

    #[test]
    fn workspace_scan_respects_ignore_and_generated_directory_boundaries() {
        let root = std::env::temp_dir().join(format!("agent-run-snapshot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::create_dir_all(root.join("ignored")).unwrap();
        fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        fs::write(root.join("src/main.ts"), "const main = true;").unwrap();
        fs::write(root.join("node_modules/package.js"), "generated").unwrap();
        fs::write(root.join("ignored/cache.json"), "ignored").unwrap();
        let scan = scan_workspace(&root);
        let _ = fs::remove_dir_all(&root);
        assert!(scan
            .manifest
            .files
            .contains_key(&snapshot_key("src/main.ts")));
        assert!(!scan
            .manifest
            .files
            .contains_key(&snapshot_key("node_modules/package.js")));
        assert!(!scan
            .manifest
            .files
            .contains_key(&snapshot_key("ignored/cache.json")));
    }

    #[test]
    fn paths_are_only_normalized_for_existing_directories() {
        assert!(normalize_path("C:\\definitely-not-a-real-workspace").is_none());
    }

    #[test]
    fn cursor_workspace_slug_matches_transcript_folder() {
        assert_eq!(
            workspace_slug(r"\\?\E:\Projects\jupiter\awa-community-web"),
            "e-projects-jupiter-awa-community-web"
        );
        assert_eq!(
            cursor_project_slug(
                r"C:\Users\Administrator\.cursor\projects\e-Projects-jupiter-awa-community-web\agent-transcripts\session.jsonl"
            ),
            Some("e-projects-jupiter-awa-community-web".into())
        );
    }

    #[test]
    fn cursor_history_without_event_timestamps_still_matches_prompt_and_workspace() {
        let run = AgentRun {
            id: "run".into(),
            task_id: Some(1),
            agent: "Cursor".into(),
            workspace_path: r"\\?\E:\Projects\jupiter\awa-community-web".into(),
            window_mode: "reuse".into(),
            transport: "cursor_ide".into(),
            window_handle: None,
            prompt_snapshot: "Fix the font subset task".into(),
            status: "prompt_filled".into(),
            error_message: None,
            session_id: None,
            match_state: "matched".into(),
            result_state: "none".into(),
            result_summary: String::new(),
            changed_files: String::new(),
            verification: String::new(),
            unresolved_issues: String::new(),
            raw_excerpt: String::new(),
            result_source_path: None,
            completed_at: None,
            created_at: "2026-08-06T07:40:38Z".into(),
            updated_at: "2026-08-06T07:40:38Z".into(),
            change_source: "legacy_history".into(),
            baseline_at: None,
            intermediate_files: String::new(),
            change_error: None,
        };
        let session = HistorySession {
      agent: "Cursor".into(), session_id: "session".into(), source_path: r"C:\Users\Administrator\.cursor\projects\e-Projects-jupiter-awa-community-web\agent-transcripts\session.jsonl".into(),
      prompt: "Fix the font subset task".into(), ended_at: Some(DateTime::parse_from_rfc3339("2026-08-06T08:13:18Z").unwrap().with_timezone(&Utc)), ..Default::default()
    };
        assert!(candidate_score(&run, &session).is_some_and(|score| score >= 9));
    }
}
