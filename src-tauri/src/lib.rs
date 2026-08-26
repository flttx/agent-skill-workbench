use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    thread,
};
use tauri::{AppHandle, Emitter, State};
use walkdir::{DirEntry, WalkDir};
mod agent_runs;
mod cursor;

const MAX_INDEXED_BYTES: u64 = 1_000_000;
const MAX_EXCERPT_CHARS: usize = 900;
const DB_SCHEMA_VERSION: i64 = 4;
struct Db(Mutex<Connection>);

#[derive(Serialize, Clone)]
struct Skill {
    id: i64,
    name: String,
    description: String,
    body: String,
    agent: String,
    path: String,
    custom: bool,
    available: bool,
    has_card: bool,
}
#[derive(Serialize, Clone)]
struct SkillVariant {
    id: i64,
    name: String,
    description: String,
    body: String,
    content: String,
    agent: String,
    path: String,
    custom: bool,
    available: bool,
    has_card: bool,
    parent_key: Option<String>,
    children_keys: Vec<String>,
    github_url: Option<String>,
}
#[derive(Serialize, Clone)]
struct SkillRelation {
    source: String,
    target: String,
    relation: String,
    unresolved: bool,
}
#[derive(Serialize, Clone)]
struct SkillFunctionGroup {
    key: String,
    name: String,
    description: String,
    skill_keys: Vec<String>,
    github_references: Vec<String>,
}
#[derive(Serialize, Clone)]
struct SkillFunctionRelation {
    source: String,
    target: String,
    relation: String,
}
#[derive(Serialize, Clone)]
struct SkillGroup {
    key: String,
    name: String,
    description: String,
    variants: Vec<SkillVariant>,
    parents: Vec<String>,
    children: Vec<String>,
    unresolved_relations: Vec<String>,
    function_keys: Vec<String>,
    cycle: bool,
    root: bool,
}
#[derive(Serialize, Clone)]
struct SkillLibrary {
    groups: Vec<SkillGroup>,
    relations: Vec<SkillRelation>,
    function_groups: Vec<SkillFunctionGroup>,
    function_relations: Vec<SkillFunctionRelation>,
}
#[derive(Serialize, Clone)]
struct SkillGroupDetail {
    group: SkillGroup,
    related_groups: Vec<SkillGroup>,
    relations: Vec<SkillRelation>,
}
#[derive(Serialize, Deserialize)]
struct Card {
    skill_id: i64,
    scenarios: String,
    triggers: String,
    steps: String,
    notes: String,
    pitfalls: String,
    links: String,
    tags: String,
}
#[derive(Serialize, Clone)]
struct SkillUpdateCommand {
    agent: String,
    command: String,
    enabled: bool,
}
#[derive(Deserialize)]
struct SkillUpdateCommandInput {
    agent: String,
    command: String,
    enabled: bool,
}
#[derive(Serialize, Clone)]
struct SkillUpdateRun {
    run_id: String,
    skill_id: i64,
    command: String,
    cwd: String,
    status: String,
}
#[derive(Serialize, Clone)]
struct SkillUpdateEvent {
    run_id: String,
    status: String,
    stream: String,
    line: String,
    exit_code: Option<i32>,
}
#[derive(Serialize, Clone)]
struct SkillCopyTarget {
    agent: String,
    path: String,
    available: bool,
}
#[derive(Serialize, Clone)]
struct CopySkillResult {
    source_agent: String,
    source_path: String,
    target_agent: String,
    target_path: String,
    backup_path: Option<String>,
    copied_files: i64,
    rescanned: bool,
}
#[derive(Serialize, Default)]
struct Event {
    id: i64,
    at: String,
    agent: String,
    skill: String,
    session_id: String,
    occurrences: i64,
    project_path: Option<String>,
    summary: String,
    parse_status: String,
    timestamp_quality: String,
}
#[derive(Serialize)]
struct AdapterStatus {
    agent: String,
    state: String,
    detail: String,
    last_sync: Option<String>,
}
#[derive(Serialize)]
struct SyncStatus {
    stage: String,
    state: String,
    detail: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}
#[derive(Serialize, Clone)]
struct AgentProbe {
    agent: String,
    state: String,
    command: String,
    executable: Option<String>,
    launch_mode: String,
    detail: String,
}
#[derive(Serialize)]
struct ScanRoot {
    id: i64,
    agent: String,
    path: String,
    enabled: bool,
    custom: bool,
}
#[derive(Serialize)]
struct Project {
    id: i64,
    title: String,
    path: Option<String>,
    updated_at: String,
}
#[derive(Deserialize)]
struct ProjectInput {
    id: Option<i64>,
    title: String,
    path: Option<String>,
}
#[derive(Serialize)]
struct Workspace {
    id: i64,
    title: String,
    description: String,
    path: Option<String>,
    color: String,
    updated_at: String,
    last_opened_at: Option<String>,
    inbox_count: i64,
    knowledge_count: i64,
    source_count: i64,
}
#[derive(Deserialize)]
struct WorkspaceInput {
    id: Option<i64>,
    title: String,
    description: String,
    path: Option<String>,
    color: String,
}
#[derive(Serialize)]
struct WorkspaceDetail {
    workspace: Workspace,
    items: Vec<KnowledgeItem>,
    roots: Vec<KnowledgeRoot>,
    events: Vec<Event>,
    tasks: Vec<WorkspaceTaskSummary>,
}
#[derive(Serialize, Clone)]
struct WorkspaceTaskSummary {
    id: i64,
    title: String,
    status: String,
    updated_at: String,
    source: Option<TaskSource>,
}
#[derive(Serialize)]
struct KnowledgeRoot {
    id: i64,
    name: String,
    kind: String,
    path: String,
    project_id: Option<i64>,
    enabled: bool,
    last_scan: Option<String>,
    detail: String,
}
#[derive(Serialize)]
struct KnowledgeItem {
    id: i64,
    title: String,
    kind: String,
    source_path: Option<String>,
    capture_kind: String,
    source_uri: Option<String>,
    excerpt: String,
    body: String,
    status: String,
    project_id: Option<i64>,
    project_title: Option<String>,
    available: bool,
    updated_at: String,
    tags: Vec<String>,
    skill_ids: Vec<i64>,
}
#[derive(Serialize)]
struct Dashboard {
    inbox_count: i64,
    project_count: i64,
    knowledge_count: i64,
    recent_items: Vec<KnowledgeItem>,
}
#[derive(Serialize, Clone)]
struct TaskProjectRef {
    id: i64,
    title: String,
}
#[derive(Serialize, Clone)]
struct TaskSource {
    id: i64,
    kind: String,
    title: String,
    uri: String,
    content: String,
    knowledge_item_id: Option<i64>,
    knowledge_item_status: Option<String>,
}
#[derive(Serialize, Clone)]
struct Task {
    id: i64,
    title: String,
    objective: String,
    steps: String,
    status: String,
    priority: i64,
    recommended_agent: Option<String>,
    recommended_skill: Option<String>,
    projects: Vec<TaskProjectRef>,
    source: Option<TaskSource>,
    updated_at: String,
    created_at: String,
}
#[derive(Deserialize)]
struct TaskInput {
    title: String,
    objective: String,
    steps: String,
    status: String,
    priority: i64,
    project_ids: Vec<i64>,
    source_kind: Option<String>,
    source_title: Option<String>,
    source_uri: Option<String>,
    source_content: Option<String>,
    source_knowledge_item_id: Option<i64>,
    recommended_agent: Option<String>,
    recommended_skill: Option<String>,
}

fn home() -> PathBuf {
    PathBuf::from(env::var("USERPROFILE").unwrap_or_default())
}
fn built_in_skill_roots() -> Vec<(String, PathBuf)> {
    vec![
        ("Codex".into(), home().join(".codex\\skills")),
        ("Claude".into(), home().join(".claude\\skills")),
        ("Cursor".into(), home().join(".cursor\\skills")),
        (
            "Gemini".into(),
            home().join(".gemini\\antigravity\\builtin\\skills"),
        ),
        ("Agents".into(), home().join(".agents\\skills")),
    ]
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn db_path() -> PathBuf {
    PathBuf::from(env::var("APPDATA").unwrap_or_else(|_| ".".into()))
        .join("AgentSkillWorkbench")
        .join("workbench.sqlite")
}
fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock()
        .map_err(|_| "数据库正在使用，请稍后重试。".to_string())
}
fn backup_database_before_migration(path: &Path, db: &Connection) {
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or_default();
    let has_data = fs::metadata(path)
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false);
    if version >= DB_SCHEMA_VERSION || !has_data {
        return;
    }
    let _ = db.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let backup = path.with_file_name(format!("workbench.sqlite.backup-{stamp}"));
    fs::copy(path, &backup).expect("backup local database before migration");
}
fn migrate_task_sources_schema(db: &Connection) -> Result<(), String> {
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(task_sources)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    if !columns.iter().any(|name| name == "knowledge_item_id") {
        db.execute(
            "ALTER TABLE task_sources ADD COLUMN knowledge_item_id INTEGER",
            [],
        )
        .map_err(|error| error.to_string())?;
    }
    db.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_sources_knowledge_item_id ON task_sources(knowledge_item_id)",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}
fn init_db() -> Connection {
    let path = db_path();
    let _ = fs::create_dir_all(path.parent().unwrap());
    let db = Connection::open(&path).expect("open local database");
    backup_database_before_migration(&path, &db);
    db.execute_batch("PRAGMA journal_mode=WAL;
    CREATE TABLE IF NOT EXISTS scan_roots(id INTEGER PRIMARY KEY, agent TEXT NOT NULL, path TEXT NOT NULL UNIQUE, enabled INTEGER NOT NULL DEFAULT 1, custom INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE IF NOT EXISTS skill_sources(id INTEGER PRIMARY KEY, agent TEXT NOT NULL UNIQUE, last_scan TEXT, detail TEXT);
    CREATE TABLE IF NOT EXISTS skills(id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, body TEXT NOT NULL, agent TEXT NOT NULL, path TEXT NOT NULL UNIQUE, custom INTEGER NOT NULL, available INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS skill_installations(id INTEGER PRIMARY KEY, skill_id INTEGER, source_id INTEGER, path TEXT UNIQUE);
    CREATE TABLE IF NOT EXISTS usage_cards(skill_id INTEGER PRIMARY KEY, scenarios TEXT NOT NULL DEFAULT '', triggers TEXT NOT NULL DEFAULT '', steps TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '', pitfalls TEXT NOT NULL DEFAULT '', links TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS tags(id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);
    CREATE TABLE IF NOT EXISTS timeline_events(id INTEGER PRIMARY KEY, source_key TEXT NOT NULL UNIQUE, at TEXT NOT NULL, agent TEXT NOT NULL, skill TEXT NOT NULL, session_id TEXT NOT NULL DEFAULT '', occurrences INTEGER NOT NULL DEFAULT 1, project_path TEXT, summary TEXT NOT NULL, parse_status TEXT NOT NULL, timestamp_quality TEXT NOT NULL DEFAULT 'exact');
    CREATE TABLE IF NOT EXISTS sync_state(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS sync_cursors(id INTEGER PRIMARY KEY, agent TEXT NOT NULL UNIQUE, cursor TEXT, updated_at TEXT);
    CREATE TABLE IF NOT EXISTS timeline_file_state(agent TEXT NOT NULL, path TEXT NOT NULL, modified_at TEXT NOT NULL, size INTEGER NOT NULL, PRIMARY KEY(agent,path));
    CREATE TABLE IF NOT EXISTS timeline_file_events(agent TEXT NOT NULL, path TEXT NOT NULL, source_key TEXT NOT NULL, at TEXT NOT NULL, skill TEXT NOT NULL, session_id TEXT NOT NULL, occurrences INTEGER NOT NULL DEFAULT 1, summary TEXT NOT NULL, timestamp_quality TEXT NOT NULL DEFAULT 'exact', PRIMARY KEY(agent,path,source_key));
    CREATE TABLE IF NOT EXISTS projects(id INTEGER PRIMARY KEY, title TEXT NOT NULL, path TEXT UNIQUE, updated_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS knowledge_roots(id INTEGER PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('project','agent_artifact')), path TEXT NOT NULL UNIQUE, project_id INTEGER, enabled INTEGER NOT NULL DEFAULT 1, last_scan TEXT, detail TEXT NOT NULL DEFAULT '等待扫描');
    CREATE TABLE IF NOT EXISTS knowledge_items(id INTEGER PRIMARY KEY, title TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('note','file','agent_artifact')), source_root_id INTEGER, source_path TEXT UNIQUE, capture_kind TEXT NOT NULL DEFAULT 'note', source_uri TEXT, content_hash TEXT, excerpt TEXT NOT NULL DEFAULT '', body TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'inbox' CHECK(status IN ('inbox','archived','ignored')), project_id INTEGER, available INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS ignored_sources(source_path TEXT PRIMARY KEY, root_id INTEGER, ignored_at TEXT NOT NULL, reason TEXT NOT NULL DEFAULT 'manual');
    CREATE TABLE IF NOT EXISTS knowledge_item_tags(item_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, PRIMARY KEY(item_id,tag_id));
    CREATE TABLE IF NOT EXISTS knowledge_item_skills(item_id INTEGER NOT NULL, skill_id INTEGER NOT NULL, PRIMARY KEY(item_id,skill_id));
    CREATE TABLE IF NOT EXISTS tasks(id INTEGER PRIMARY KEY, title TEXT NOT NULL, objective TEXT NOT NULL DEFAULT '', steps TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'ready' CHECK(status IN ('draft','ready','in_progress','blocked','done')), priority INTEGER NOT NULL DEFAULT 0, recommended_agent TEXT, recommended_skill TEXT, updated_at TEXT NOT NULL, created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS task_projects(task_id INTEGER NOT NULL, project_id INTEGER NOT NULL, PRIMARY KEY(task_id,project_id));
    CREATE TABLE IF NOT EXISTS task_sources(id INTEGER PRIMARY KEY, task_id INTEGER NOT NULL, kind TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', uri TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS agent_runs(id TEXT PRIMARY KEY, task_id INTEGER, agent TEXT NOT NULL, workspace_path TEXT NOT NULL, window_mode TEXT NOT NULL, transport TEXT NOT NULL, window_handle INTEGER, prompt_snapshot TEXT NOT NULL DEFAULT '', status TEXT NOT NULL, error_message TEXT, session_id TEXT, match_state TEXT NOT NULL DEFAULT 'matched', result_state TEXT NOT NULL DEFAULT 'none', result_summary TEXT NOT NULL DEFAULT '', changed_files TEXT NOT NULL DEFAULT '', verification TEXT NOT NULL DEFAULT '', unresolved_issues TEXT NOT NULL DEFAULT '', raw_excerpt TEXT NOT NULL DEFAULT '', result_source_path TEXT, completed_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, change_source TEXT NOT NULL DEFAULT 'legacy_history', baseline_manifest TEXT NOT NULL DEFAULT '', baseline_at TEXT, intermediate_files TEXT NOT NULL DEFAULT '', change_error TEXT);
  ").expect("create local schema");
    db.execute("CREATE TABLE IF NOT EXISTS skill_update_commands(agent TEXT PRIMARY KEY, command TEXT NOT NULL DEFAULT '', enabled INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL)", []).expect("create skill update command schema");
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(projects)")
        .expect("inspect workspace schema")
        .query_map([], |row| row.get(1))
        .expect("read workspace schema")
        .collect::<Result<_, _>>()
        .expect("collect workspace schema");
    if !columns.iter().any(|name| name == "description") {
        db.execute(
            "ALTER TABLE projects ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add workspace description");
    }
    if !columns.iter().any(|name| name == "color") {
        db.execute(
            "ALTER TABLE projects ADD COLUMN color TEXT NOT NULL DEFAULT 'violet'",
            [],
        )
        .expect("add workspace color");
    }
    if !columns.iter().any(|name| name == "last_opened_at") {
        db.execute("ALTER TABLE projects ADD COLUMN last_opened_at TEXT", [])
            .expect("add workspace last opened");
    }
    let timeline_columns: Vec<String> = db
        .prepare("PRAGMA table_info(timeline_events)")
        .expect("inspect timeline schema")
        .query_map([], |row| row.get(1))
        .expect("read timeline schema")
        .collect::<Result<_, _>>()
        .expect("collect timeline schema");
    if !timeline_columns.iter().any(|name| name == "session_id") {
        db.execute(
            "ALTER TABLE timeline_events ADD COLUMN session_id TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add timeline session");
    }
    if !timeline_columns.iter().any(|name| name == "occurrences") {
        db.execute(
            "ALTER TABLE timeline_events ADD COLUMN occurrences INTEGER NOT NULL DEFAULT 1",
            [],
        )
        .expect("add timeline occurrences");
    }
    if !timeline_columns
        .iter()
        .any(|name| name == "timestamp_quality")
    {
        db.execute(
            "ALTER TABLE timeline_events ADD COLUMN timestamp_quality TEXT NOT NULL DEFAULT 'file'",
            [],
        )
        .expect("add timeline timestamp quality");
    }
    let knowledge_columns: Vec<String> = db
        .prepare("PRAGMA table_info(knowledge_items)")
        .expect("inspect knowledge item schema")
        .query_map([], |row| row.get(1))
        .expect("read knowledge item schema")
        .collect::<Result<_, _>>()
        .expect("collect knowledge item schema");
    if !knowledge_columns.iter().any(|name| name == "capture_kind") {
        db.execute(
            "ALTER TABLE knowledge_items ADD COLUMN capture_kind TEXT NOT NULL DEFAULT 'note'",
            [],
        )
        .expect("add knowledge capture kind");
    }
    if !knowledge_columns.iter().any(|name| name == "source_uri") {
        db.execute("ALTER TABLE knowledge_items ADD COLUMN source_uri TEXT", [])
            .expect("add knowledge source uri");
    }
    migrate_task_sources_schema(&db).expect("migrate task source schema");
    let agent_run_columns: Vec<String> = db
        .prepare("PRAGMA table_info(agent_runs)")
        .expect("inspect agent run schema")
        .query_map([], |row| row.get(1))
        .expect("read agent run schema")
        .collect::<Result<_, _>>()
        .expect("collect agent run schema");
    if !agent_run_columns.iter().any(|name| name == "session_id") {
        db.execute("ALTER TABLE agent_runs ADD COLUMN session_id TEXT", [])
            .expect("add agent run session");
    }
    if !agent_run_columns.iter().any(|name| name == "match_state") {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN match_state TEXT NOT NULL DEFAULT 'matched'",
            [],
        )
        .expect("add agent run match state");
    }
    if !agent_run_columns.iter().any(|name| name == "result_state") {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN result_state TEXT NOT NULL DEFAULT 'none'",
            [],
        )
        .expect("add agent run result state");
    }
    if !agent_run_columns
        .iter()
        .any(|name| name == "result_summary")
    {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN result_summary TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run summary");
    }
    if !agent_run_columns.iter().any(|name| name == "changed_files") {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN changed_files TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run changed files");
    }
    if !agent_run_columns.iter().any(|name| name == "verification") {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN verification TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run verification");
    }
    if !agent_run_columns
        .iter()
        .any(|name| name == "unresolved_issues")
    {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN unresolved_issues TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run unresolved issues");
    }
    if !agent_run_columns.iter().any(|name| name == "raw_excerpt") {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN raw_excerpt TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run raw excerpt");
    }
    if !agent_run_columns
        .iter()
        .any(|name| name == "result_source_path")
    {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN result_source_path TEXT",
            [],
        )
        .expect("add agent run source path");
    }
    if !agent_run_columns.iter().any(|name| name == "completed_at") {
        db.execute("ALTER TABLE agent_runs ADD COLUMN completed_at TEXT", [])
            .expect("add agent run completed at");
    }
    if !agent_run_columns.iter().any(|name| name == "change_source") {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN change_source TEXT NOT NULL DEFAULT 'legacy_history'",
            [],
        )
        .expect("add agent run change source");
    }
    if !agent_run_columns
        .iter()
        .any(|name| name == "baseline_manifest")
    {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN baseline_manifest TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run baseline manifest");
    }
    if !agent_run_columns.iter().any(|name| name == "baseline_at") {
        db.execute("ALTER TABLE agent_runs ADD COLUMN baseline_at TEXT", [])
            .expect("add agent run baseline at");
    }
    if !agent_run_columns
        .iter()
        .any(|name| name == "intermediate_files")
    {
        db.execute(
            "ALTER TABLE agent_runs ADD COLUMN intermediate_files TEXT NOT NULL DEFAULT ''",
            [],
        )
        .expect("add agent run intermediate files");
    }
    if !agent_run_columns.iter().any(|name| name == "change_error") {
        db.execute("ALTER TABLE agent_runs ADD COLUMN change_error TEXT", [])
            .expect("add agent run change error");
    }
    db.execute("DELETE FROM agent_runs WHERE transport='history'", [])
        .expect("remove legacy history runs");
    let parser_version: Option<String> = db
        .query_row(
            "SELECT value FROM sync_state WHERE key='timeline_parser'",
            [],
            |row| row.get(0),
        )
        .ok();
    if parser_version.as_deref() != Some("3") {
        db.execute("DELETE FROM timeline_events", [])
            .expect("rebuild timeline events");
        db.execute("DELETE FROM timeline_file_state", [])
            .expect("reset timeline file state");
        db.execute("DELETE FROM timeline_file_events", [])
            .expect("reset timeline file events");
        db.execute("INSERT INTO sync_state(key,value) VALUES('timeline_parser','3') ON CONFLICT(key) DO UPDATE SET value=excluded.value", []).expect("save timeline parser version");
    }
    let defaults = [
        ("Codex", home().join(".codex\\skills")),
        ("Claude", home().join(".claude\\skills")),
        ("Cursor", home().join(".cursor\\skills")),
        (
            "Gemini",
            home().join(".gemini\\antigravity\\builtin\\skills"),
        ),
        ("Agents", home().join(".agents\\skills")),
    ];
    for (agent, path) in defaults {
        let _ = db.execute(
            "INSERT OR IGNORE INTO scan_roots(agent,path,custom) VALUES(?1,?2,0)",
            params![agent, path.to_string_lossy()],
        );
    }
    for agent in ["Codex", "Claude", "Cursor", "Gemini", "Agents"] {
        let _ = db.execute("INSERT OR IGNORE INTO skill_update_commands(agent,command,enabled,updated_at) VALUES(?1,'',0,?2)", params![agent, now()]);
    }
    db.pragma_update(None, "user_version", DB_SCHEMA_VERSION)
        .expect("save database schema version");
    db
}

struct SkillMetadata {
    name: String,
    description: String,
    parent: Option<String>,
    children: Vec<String>,
    github_url: Option<String>,
}
fn parse_frontmatter_value(header: &str, key: &str) -> Option<String> {
    header
        .lines()
        .find_map(|line| line.trim().strip_prefix(&format!("{key}:")))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
}
fn parse_frontmatter_description(header: &str) -> Option<String> {
    let lines: Vec<&str> = header.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(raw) = line.trim().strip_prefix("description:") else {
            continue;
        };
        let value = raw.trim();
        let block = matches!(value, "|" | "|-" | "|+" | ">" | ">-" | ">+");
        let mut parts = Vec::new();
        if !value.is_empty() && !block {
            parts.push(value.trim_matches('"').to_string());
        }
        let mut continued = false;
        for continuation in lines.iter().skip(index + 1) {
            if continuation.trim().is_empty() {
                if continued {
                    parts.push(String::new());
                }
                continue;
            }
            if !continuation.chars().next().is_some_and(char::is_whitespace) {
                break;
            }
            continued = true;
            parts.push(continuation.trim().trim_matches('"').to_string());
        }
        let description = parts.join("\n").trim().to_string();
        return (!description.is_empty()).then_some(description);
    }
    None
}
fn parse_name_from_file(path: &Path) -> Option<String> {
    let body = fs::read_to_string(path).ok()?;
    Some(parse_skill_metadata(&body, path).name)
}
fn parse_skill_list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}
fn fallback_parent(path: &Path) -> Option<String> {
    let parent_skill = path.parent()?.parent()?.join("SKILL.md");
    if parent_skill.is_file() {
        parse_name_from_file(&parent_skill)
    } else {
        None
    }
}
fn parse_skill_metadata(text: &str, path: &Path) -> SkillMetadata {
    let fallback = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|v| v.to_str())
        .unwrap_or("unnamed")
        .to_string();
    let header = text
        .strip_prefix("---")
        .and_then(|v| v.split("---").next())
        .unwrap_or("");
    let name = parse_frontmatter_value(header, "name").unwrap_or(fallback);
    let description = parse_frontmatter_description(header).unwrap_or_default();
    let parent = parse_frontmatter_value(header, "parent").or_else(|| fallback_parent(path));
    let children = parse_frontmatter_value(header, "children")
        .map(|value| parse_skill_list(&value))
        .unwrap_or_default();
    let github_url = ["source-repo", "repository", "repo", "homepage"]
        .iter()
        .find_map(|key| parse_frontmatter_value(header, key))
        .filter(|value| value.to_ascii_lowercase().contains("github.com"));
    SkillMetadata {
        name,
        description,
        parent,
        children,
        github_url,
    }
}
fn parse_skill(text: &str, path: &Path) -> (String, String) {
    let metadata = parse_skill_metadata(text, path);
    (metadata.name, metadata.description)
}
fn skill_content(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return text.to_string();
    }
    for (index, line) in text.lines().enumerate().skip(1) {
        if line.trim() == "---" {
            return text
                .lines()
                .skip(index + 1)
                .collect::<Vec<_>>()
                .join("\n")
                .trim_start()
                .to_string();
        }
    }
    text.to_string()
}
fn skill_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}
fn normalized(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase()
}
fn known_github_url(name: &str, agent: &str) -> Option<String> {
    if agent == "Codex" && name.eq_ignore_ascii_case("skill-creator") {
        Some(
            "https://github.com/openai/skills/blob/main/skills/.system/skill-creator/SKILL.md"
                .into(),
        )
    } else if agent == "Codex" && name.eq_ignore_ascii_case("skill-installer") {
        Some(
            "https://github.com/openai/skills/blob/main/skills/.system/skill-installer/SKILL.md"
                .into(),
        )
    } else {
        None
    }
}
fn default_skill_root(db: &Connection, agent: &str) -> Result<PathBuf, String> {
    if let Some((_, path)) = built_in_skill_roots()
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(agent))
    {
        return Ok(path);
    }
    db.query_row(
        "SELECT path FROM scan_roots WHERE agent=?1 AND custom=0 AND enabled=1 ORDER BY id LIMIT 1",
        [agent],
        |row| row.get::<_, String>(0),
    )
    .map(PathBuf::from)
    .map_err(|_| format!("{agent} 暂无默认 Skill 目录"))
}
fn copy_directory(source: &Path, target: &Path) -> Result<i64, String> {
    let mut copied = 0;
    for entry in WalkDir::new(source).follow_links(false).into_iter() {
        let entry = entry.map_err(|error| format!("读取 Skill 目录失败：{error}"))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).map_err(|error| format!("创建目录失败：{error}"))?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| format!("创建资源目录失败：{error}"))?;
            }
            fs::copy(entry.path(), &destination)
                .map_err(|error| format!("复制文件 {} 失败：{error}", entry.path().display()))?;
            copied += 1;
        } else {
            return Err(format!(
                "Skill 包含暂不支持复制的文件：{}",
                entry.path().display()
            ));
        }
    }
    Ok(copied)
}
fn remove_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|error| format!("清理临时目录失败：{error}"))
}
fn backup_path(target_root: &Path, folder_name: &str) -> PathBuf {
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let candidate = target_root.join(format!("{folder_name}.backup-{stamp}"));
    if !candidate.exists() {
        return candidate;
    }
    target_root.join(format!("{folder_name}.backup-{stamp}-{}", fnv_hash(&now())))
}
fn emit_skill_update(app: &AppHandle, event: SkillUpdateEvent) {
    let _ = app.emit("skill-update-output", event);
}
fn expand_skill_update_command(template: &str, name: &str, path: &Path, agent: &str) -> String {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    template
        .replace("{name}", name)
        .replace("{path}", &path.to_string_lossy())
        .replace("{dir}", &dir.to_string_lossy())
        .replace("{agent}", agent)
}
#[cfg(test)]
mod skill_detail_tests {
    use super::*;
    #[test]
    fn preserves_skill_content_after_frontmatter() {
        assert_eq!(
            skill_content("---\nname: demo\n---\n# Demo\n\n- item"),
            "# Demo\n\n- item"
        );
    }
    #[test]
    fn expands_update_command_variables() {
        let path = Path::new("C:/skills/demo/SKILL.md");
        assert_eq!(
            expand_skill_update_command("echo {name} {agent} {path} {dir}", "demo", path, "Codex"),
            "echo demo Codex C:/skills/demo/SKILL.md C:/skills/demo"
        );
    }
}
fn scan_skills(db: &Connection) -> Result<(), String> {
    db.execute("UPDATE skills SET available=0", [])
        .map_err(|e| e.to_string())?;
    let mut statement = db
        .prepare("SELECT agent,path,custom FROM scan_roots WHERE enabled=1")
        .map_err(|e| e.to_string())?;
    let roots: Vec<(String, String, bool)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? == 1)))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    for (agent, root, custom) in roots {
        let root = PathBuf::from(root);
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(skip_dir)
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file() && e.file_name() == "SKILL.md")
        {
            let path = entry.path();
            let body = match fs::read_to_string(path) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let (name, description) = parse_skill(&body, path);
            db.execute("INSERT INTO skills(name,description,body,agent,path,custom,available,updated_at) VALUES(?1,?2,?3,?4,?5,?6,1,?7) ON CONFLICT(path) DO UPDATE SET name=excluded.name,description=excluded.description,body=excluded.body,agent=excluded.agent,custom=excluded.custom,available=1,updated_at=excluded.updated_at", params![name,description,body,agent,normalized(path),custom as i64,now()]).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn history_has_generated_directory(path: &Path) -> bool {
    let parts: Vec<String> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(|part| part.to_ascii_lowercase())
        .collect();
    parts.windows(2).any(|window| {
        window[0] == ".system_generated" && matches!(window[1].as_str(), "logs" | "messages")
    })
}
fn history_timestamp(line: &str) -> Option<String> {
    if let Some(start) = line.find("<timestamp>") {
        let value = line[start + "<timestamp>".len()..]
            .split("</timestamp>")
            .next()?
            .trim();
        let value = value.split(", ").skip(1).collect::<Vec<_>>().join(", ");
        let value = value
            .replace(" (UTC+8)", " +08:00")
            .replace(" (UTC-8)", " -08:00");
        if let Ok(parsed) = DateTime::parse_from_str(&value, "%b %-d, %Y, %-I:%M %p %:z") {
            return Some(parsed.to_rfc3339());
        }
    }
    fn find_json_timestamp(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::Object(object) => {
                for key in [
                    "timestamp",
                    "created_at",
                    "createdAt",
                    "event_timestamp",
                    "time",
                ] {
                    if let Some(serde_json::Value::String(timestamp)) = object.get(key) {
                        if DateTime::parse_from_rfc3339(timestamp).is_ok() {
                            return Some(timestamp.clone());
                        }
                    }
                }
                object.values().find_map(find_json_timestamp)
            }
            serde_json::Value::Array(items) => items.iter().find_map(find_json_timestamp),
            _ => None,
        }
    }
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| find_json_timestamp(&value))
}
#[allow(dead_code)]
fn detect_history_files_legacy(
    db: &Connection,
    agent: &str,
    roots: &[PathBuf],
    extensions: &[&str],
    generated_only: bool,
) -> Result<(String, String), String> {
    let skills: Vec<String> = db
        .prepare("SELECT name FROM skills WHERE available=1")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    let mut inserted = 0;
    let mut files = 0;
    let mut found_root = false;
    for root in roots {
        if !root.exists() {
            continue;
        }
        found_root = true;
        for file in WalkDir::new(root)
            .max_depth(8)
            .into_iter()
            .filter_entry(skip_dir)
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let extension = file
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !extensions.iter().any(|value| *value == extension)
                || (generated_only && !history_has_generated_directory(file.path()))
            {
                continue;
            }
            files += 1;
            let modified: DateTime<Utc> = file
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .map(DateTime::<Utc>::from)
                .unwrap_or_else(Utc::now);
            let content = match fs::read_to_string(file.path()) {
                Ok(value) => value,
                Err(_) => continue,
            };
            for (line_no, line) in content.lines().enumerate() {
                let lower = line.to_lowercase();
                if !lower.contains("skill") {
                    continue;
                }
                let event_at = history_timestamp(line).unwrap_or_else(|| modified.to_rfc3339());
                for skill in &skills {
                    if lower.contains(&skill.to_lowercase()) {
                        let key =
                            format!("{}:{}:{}:{}", agent, file.path().display(), line_no, skill);
                        inserted += db.execute("INSERT INTO timeline_events(source_key,at,agent,skill,summary,parse_status) VALUES(?1,?2,?3,?4,?5,'ok') ON CONFLICT(source_key) DO UPDATE SET at=excluded.at,agent=excluded.agent,skill=excluded.skill,summary=excluded.summary,parse_status=excluded.parse_status", params![key,event_at,agent,skill,format!("在 {agent} 历史中检测到对 {skill} 的显式 skill 引用")]).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
    if !found_root {
        return Ok(("unavailable".into(), "未找到本地历史目录".into()));
    }
    Ok((
        "ok".into(),
        format!("已解析历史文件，扫描 {files} 个文件，新增 {inserted} 条可证实事件"),
    ))
}
#[allow(dead_code)]
fn detect_jsonl_legacy(
    db: &Connection,
    agent: &str,
    root: PathBuf,
) -> Result<(String, String), String> {
    detect_history_files_legacy(db, agent, &[root], &["jsonl"], false)
}
#[allow(dead_code)]
fn detect_antigravity_history_legacy(db: &Connection) -> Result<(String, String), String> {
    detect_history_files_legacy(
        db,
        "Antigravity",
        &[
            home().join(".gemini\\antigravity\\brain"),
            home().join(".gemini\\antigravity-ide\\brain"),
        ],
        &["json", "jsonl", ""],
        true,
    )
}
#[cfg(test)]
mod history_time_tests {
    use super::*;
    #[test]
    fn reads_json_timestamp() {
        assert_eq!(
            history_timestamp(r#"{"timestamp":"2026-07-31T04:02:52.861Z","type":"event_msg"}"#)
                .as_deref(),
            Some("2026-07-31T04:02:52.861Z")
        );
    }
    #[test]
    fn reads_cursor_timestamp() {
        assert_eq!(
            history_timestamp("<timestamp>Thursday, Aug 6, 2026, 9:26 AM (UTC+8)</timestamp>")
                .as_deref(),
            Some("2026-08-06T09:26:00+08:00")
        );
    }
}
#[cfg(test)]
mod strict_timeline_tests {
    use super::*;
    #[test]
    fn accepts_explicit_calls_and_ignores_context() {
        let skills = vec![("demo".to_string(), "demo".to_string())];
        assert_eq!(
            strict_skill_hits(r#"{"role":"user","content":"$demo"}"#, &skills),
            vec!["demo"]
        );
        assert!(
            strict_skill_hits(r#"{"role":"developer","content":"use $demo"}"#, &skills).is_empty()
        );
        assert!(strict_skill_hits(
            r#"{"role":"assistant","path":"C:\\skills\\demo\\SKILL.md"}"#,
            &skills
        )
        .contains(&"demo".to_string()));
    }
    #[test]
    fn merges_session_calls() {
        let at = DateTime::parse_from_rfc3339("2026-08-06T01:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut events = HashMap::new();
        let draft = |at: DateTime<Utc>, session: &str| TimelineDraft {
            source_key: format!("codex:{}:demo", session),
            at: at.to_rfc3339(),
            at_order: at,
            agent: "Codex".into(),
            skill: "demo".into(),
            session_id: session.into(),
            occurrences: 1,
            summary: "call".into(),
            timestamp_quality: "exact".into(),
        };
        merge_timeline_draft(&mut events, draft(at, "one"));
        merge_timeline_draft(&mut events, draft(at + chrono::Duration::seconds(2), "one"));
        merge_timeline_draft(&mut events, draft(at, "two"));
        assert_eq!(events.len(), 2);
        assert_eq!(events["codex:one:demo"].occurrences, 2);
        assert_eq!(events["codex:one:demo"].at, "2026-08-06T01:00:02+00:00");
    }
}
#[allow(dead_code)]
fn sync_timeline_legacy(db: &Connection) -> Result<(), String> {
    for (agent, root) in [
        ("Codex", home().join(".codex")),
        ("Claude", home().join(".claude\\projects")),
        ("Cursor", home().join(".cursor\\projects")),
    ] {
        let (state, detail) = detect_jsonl_legacy(db, agent, root)?;
        db.execute("INSERT INTO skill_sources(agent,last_scan,detail) VALUES(?1,?2,?3) ON CONFLICT(agent) DO UPDATE SET last_scan=excluded.last_scan,detail=excluded.detail", params![agent,now(),format!("{state}: {detail}")]).map_err(|e| e.to_string())?;
    }
    let (state, detail) = detect_antigravity_history_legacy(db)?;
    db.execute("INSERT INTO skill_sources(agent,last_scan,detail) VALUES(?1,?2,?3) ON CONFLICT(agent) DO UPDATE SET last_scan=excluded.last_scan,detail=excluded.detail", params!["Antigravity",now(),format!("{state}: {detail}")]).map_err(|e| e.to_string())?;
    for (agent, detail) in [
        (
            "Gemini",
            "Skill 来源仍使用 Gemini 内置目录；会话历史统一按 Antigravity 目录解析",
        ),
        ("Agents", "未发现可解析的会话历史"),
    ] {
        db.execute("INSERT INTO skill_sources(agent,last_scan,detail) VALUES(?1,?2,?3) ON CONFLICT(agent) DO UPDATE SET last_scan=excluded.last_scan,detail=excluded.detail", params![agent,now(),format!("unsupported: {detail}")]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Clone)]
struct TimelineDraft {
    source_key: String,
    at: String,
    at_order: DateTime<Utc>,
    agent: String,
    skill: String,
    session_id: String,
    occurrences: i64,
    summary: String,
    timestamp_quality: String,
}

fn strict_history_role(value: &serde_json::Value) -> Option<&str> {
    value
        .get("role")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/payload/role")
                .and_then(serde_json::Value::as_str)
        })
}

fn strict_history_type(value: &serde_json::Value) -> Option<&str> {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .pointer("/payload/type")
                .and_then(serde_json::Value::as_str)
        })
}

fn strict_skill_hits(line: &str, skills: &[(String, String)]) -> Vec<String> {
    let value = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    if matches!(
        strict_history_role(&value)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("system" | "developer" | "tool")
    ) {
        return Vec::new();
    }
    if matches!(
        strict_history_type(&value)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "world_state"
                | "compacted"
                | "function_call_output"
                | "custom_tool_call_output"
                | "tool_result"
        )
    ) {
        return Vec::new();
    }
    let role = strict_history_role(&value).map(str::to_ascii_lowercase);
    let event_type = strict_history_type(&value).map(str::to_ascii_lowercase);
    let structured_call = event_type
        .as_deref()
        .is_some_and(|kind| kind.contains("call") || kind.contains("tool"));
    let lower = line.to_ascii_lowercase();
    skills
        .iter()
        .filter_map(|(key, name)| {
            let dollar = format!("{}{}", "$", key);
            let windows_path = format!(r"\{}\skill.md", key);
            let unix_path = format!("/{}/skill.md", key);
            let escaped_windows_path = format!(r"\\{}\\skill.md", key);
            let dollar_call =
                !matches!(role.as_deref(), Some("assistant")) && lower.contains(&dollar);
            let loader = (matches!(role.as_deref(), Some("user")) || structured_call)
                && [
                    "load skill",
                    "skill loader",
                    "using skill",
                    "invoke skill",
                    "activate skill",
                ]
                .iter()
                .any(|marker| lower.contains(marker) && lower.contains(key));
            (dollar_call
                || lower.contains(&windows_path)
                || lower.contains(&unix_path)
                || lower.contains(&escaped_windows_path)
                || loader)
                .then_some(name.clone())
        })
        .collect()
}

fn strict_session_id(value: &serde_json::Value, path: &Path, agent: &str) -> String {
    for pointer in [
        "/session_id",
        "/conversation_id",
        "/trajectory_id",
        "/payload/session_id",
        "/payload/conversation_id",
        "/payload/trajectory_id",
    ] {
        if let Some(id) = value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.trim().is_empty())
        {
            return id.to_string();
        }
    }
    if agent.eq_ignore_ascii_case("antigravity") {
        let components: Vec<String> = path
            .components()
            .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
            .collect();
        if let Some(index) = components
            .iter()
            .position(|component| component.eq_ignore_ascii_case(".system_generated"))
        {
            if index > 0 {
                return components[index - 1].clone();
            }
        }
    }
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown-session")
        .to_string()
}

fn merge_timeline_draft(drafts: &mut HashMap<String, TimelineDraft>, draft: TimelineDraft) {
    if let Some(existing) = drafts.get_mut(&draft.source_key) {
        existing.occurrences += draft.occurrences;
        if draft.at_order >= existing.at_order {
            existing.at = draft.at;
            existing.at_order = draft.at_order;
            existing.timestamp_quality = draft.timestamp_quality;
        }
    } else {
        drafts.insert(draft.source_key.clone(), draft);
    }
}

#[allow(dead_code)]
fn strict_history_files(
    db: &Connection,
    agent: &str,
    roots: &[PathBuf],
    extensions: &[&str],
    generated_only: bool,
) -> Result<(String, String, Vec<TimelineDraft>), String> {
    let mut skills: Vec<(String, String)> = db
        .prepare("SELECT name,name FROM skills WHERE available=1")
        .map_err(|error| error.to_string())?
        .query_map([], |row| {
            Ok((skill_key(&row.get::<_, String>(0)?), row.get(1)?))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect();
    skills.sort_by(|left, right| left.0.cmp(&right.0));
    skills.dedup_by(|left, right| left.0 == right.0);
    let mut drafts = HashMap::new();
    let mut files = 0;
    let mut found_root = false;
    for root in roots {
        if !root.exists() {
            continue;
        }
        found_root = true;
        for entry in WalkDir::new(root)
            .max_depth(8)
            .into_iter()
            .filter_entry(skip_dir)
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !extensions.iter().any(|value| *value == extension)
                || (generated_only && !history_has_generated_directory(entry.path()))
            {
                continue;
            }
            files += 1;
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .map(DateTime::<Utc>::from)
                .unwrap_or_else(Utc::now);
            let content = match fs::read_to_string(entry.path()) {
                Ok(content) => content,
                Err(_) => continue,
            };
            for line in content.lines() {
                let hits = strict_skill_hits(line, &skills);
                if hits.is_empty() {
                    continue;
                }
                let value = match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                let (at, timestamp_quality) = match history_timestamp(line) {
                    Some(at) => (at, "exact".to_string()),
                    None => (modified.to_rfc3339(), "file".to_string()),
                };
                let at_order = DateTime::parse_from_rfc3339(&at)
                    .map(|date| date.with_timezone(&Utc))
                    .unwrap_or(modified);
                let session_id = strict_session_id(&value, entry.path(), agent);
                for skill in hits {
                    let source_key = format!(
                        "{}:{}:{}",
                        agent.to_ascii_lowercase(),
                        session_id,
                        skill_key(&skill)
                    );
                    merge_timeline_draft(
                        &mut drafts,
                        TimelineDraft {
                            source_key,
                            at: at.clone(),
                            at_order,
                            agent: agent.to_string(),
                            skill,
                            session_id: session_id.clone(),
                            occurrences: 1,
                            summary: format!("Explicit Skill call in {} session", agent),
                            timestamp_quality: timestamp_quality.clone(),
                        },
                    );
                }
            }
        }
    }
    if !found_root {
        return Ok((
            "unavailable".into(),
            "No local history directory found".into(),
            Vec::new(),
        ));
    }
    let mut output: Vec<TimelineDraft> = drafts.into_values().collect();
    output.sort_by(|left, right| right.at_order.cmp(&left.at_order));
    Ok((
        "ok".into(),
        format!(
            "Parsed {} history files and {} session calls",
            files,
            output.len()
        ),
        output,
    ))
}

#[allow(dead_code)]
fn strict_jsonl(
    db: &Connection,
    agent: &str,
    root: PathBuf,
) -> Result<(String, String, Vec<TimelineDraft>), String> {
    strict_history_files(db, agent, &[root], &["jsonl"], false)
}

#[allow(dead_code)]
fn strict_antigravity_history(
    db: &Connection,
) -> Result<(String, String, Vec<TimelineDraft>), String> {
    strict_history_files(
        db,
        "Antigravity",
        &[
            home().join(".gemini\\antigravity\\brain"),
            home().join(".gemini\\antigravity-ide\\brain"),
        ],
        &["json", "jsonl", ""],
        true,
    )
}

fn available_history_skills(db: &Connection) -> Result<Vec<(String, String)>, String> {
    let mut skills: Vec<(String, String)> = db
        .prepare("SELECT name,name FROM skills WHERE available=1")
        .map_err(|error| error.to_string())?
        .query_map([], |row| {
            Ok((skill_key(&row.get::<_, String>(0)?), row.get(1)?))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect();
    skills.sort_by(|left, right| left.0.cmp(&right.0));
    skills.dedup_by(|left, right| left.0 == right.0);
    Ok(skills)
}

fn parse_history_file(
    agent: &str,
    path: &Path,
    skills: &[(String, String)],
) -> Result<Vec<TimelineDraft>, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let modified = metadata
        .modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut drafts = HashMap::new();
    for line in content.lines() {
        let hits = strict_skill_hits(line, skills);
        if hits.is_empty() {
            continue;
        }
        let value = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let (at, timestamp_quality) = match history_timestamp(line) {
            Some(at) => (at, "exact".to_string()),
            None => (modified.to_rfc3339(), "file".to_string()),
        };
        let at_order = DateTime::parse_from_rfc3339(&at)
            .map(|date| date.with_timezone(&Utc))
            .unwrap_or(modified);
        let session_id = strict_session_id(&value, path, agent);
        for skill in hits {
            let source_key = format!(
                "{}:{}:{}",
                agent.to_ascii_lowercase(),
                session_id,
                skill_key(&skill)
            );
            merge_timeline_draft(
                &mut drafts,
                TimelineDraft {
                    source_key,
                    at: at.clone(),
                    at_order,
                    agent: agent.to_string(),
                    skill,
                    session_id: session_id.clone(),
                    occurrences: 1,
                    summary: format!("Explicit Skill call in {} session", agent),
                    timestamp_quality: timestamp_quality.clone(),
                },
            );
        }
    }
    Ok(drafts.into_values().collect())
}

fn sync_history_source_incremental(
    db: &mut Connection,
    agent: &str,
    roots: &[PathBuf],
    extensions: &[&str],
    generated_only: bool,
    skills: &[(String, String)],
    cached_state: &HashMap<(String, String), (String, i64)>,
    seen: &mut HashSet<(String, String)>,
) -> Result<(String, String), String> {
    let mut found_root = false;
    let mut checked_files = 0;
    let mut parsed_files = 0;
    for root in roots {
        if !root.exists() {
            continue;
        }
        found_root = true;
        for entry in WalkDir::new(root)
            .max_depth(8)
            .follow_links(false)
            .into_iter()
            .filter_entry(skip_dir)
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !extensions.iter().any(|value| *value == extension)
                || (generated_only && !history_has_generated_directory(entry.path()))
            {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            let path = normalized(entry.path());
            let state_key = (agent.to_string(), path.clone());
            seen.insert(state_key.clone());
            checked_files += 1;
            let modified_at = metadata
                .modified()
                .map(DateTime::<Utc>::from)
                .unwrap_or_else(|_| Utc::now())
                .to_rfc3339();
            let size = metadata.len() as i64;
            if cached_state
                .get(&state_key)
                .is_some_and(|(cached_modified, cached_size)| {
                    cached_modified == &modified_at && *cached_size == size
                })
            {
                continue;
            }
            let events = match parse_history_file(agent, entry.path(), skills) {
                Ok(events) => events,
                Err(_) => continue,
            };
            parsed_files += 1;
            db.execute(
                "DELETE FROM timeline_file_events WHERE agent=?1 AND path=?2",
                params![agent, path],
            )
            .map_err(|error| error.to_string())?;
            for event in events {
                db.execute(
                    "INSERT INTO timeline_file_events(agent,path,source_key,at,skill,session_id,occurrences,summary,timestamp_quality) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![agent, path, event.source_key, event.at, event.skill, event.session_id, event.occurrences, event.summary, event.timestamp_quality],
                )
                .map_err(|error| error.to_string())?;
            }
            db.execute(
                "INSERT INTO timeline_file_state(agent,path,modified_at,size) VALUES(?1,?2,?3,?4) ON CONFLICT(agent,path) DO UPDATE SET modified_at=excluded.modified_at,size=excluded.size",
                params![agent, path, modified_at, size],
            )
            .map_err(|error| error.to_string())?;
        }
    }
    if found_root {
        Ok((
            "ok".into(),
            format!(
                "Checked {} history files, parsed {} changed files",
                checked_files, parsed_files
            ),
        ))
    } else {
        Ok((
            "unavailable".into(),
            "No local history directory found".into(),
        ))
    }
}

fn rebuild_timeline_events(db: &mut Connection) -> Result<(), String> {
    let mut drafts = HashMap::new();
    let mut statement = db
        .prepare("SELECT agent,source_key,at,skill,session_id,occurrences,summary,timestamp_quality FROM timeline_file_events")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let at: String = row.get(2)?;
            let at_order = DateTime::parse_from_rfc3339(&at)
                .map(|date| date.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(TimelineDraft {
                agent: row.get(0)?,
                source_key: row.get(1)?,
                at,
                at_order,
                skill: row.get(3)?,
                session_id: row.get(4)?,
                occurrences: row.get(5)?,
                summary: row.get(6)?,
                timestamp_quality: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        merge_timeline_draft(&mut drafts, row.map_err(|error| error.to_string())?);
    }
    drop(statement);
    let transaction = db.transaction().map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM timeline_events", [])
        .map_err(|error| error.to_string())?;
    for event in drafts.into_values() {
        transaction
            .execute(
                "INSERT INTO timeline_events(source_key,at,agent,skill,session_id,occurrences,summary,parse_status,timestamp_quality) VALUES(?1,?2,?3,?4,?5,?6,?7,'ok',?8)",
                params![event.source_key, event.at, event.agent, event.skill, event.session_id, event.occurrences, event.summary, event.timestamp_quality],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn sync_timeline(db: &mut Connection) -> Result<(), String> {
    let skills = available_history_skills(db)?;
    let cached_state: HashMap<(String, String), (String, i64)> = db
        .prepare("SELECT agent,path,modified_at,size FROM timeline_file_state")
        .map_err(|error| error.to_string())?
        .query_map([], |row| {
            Ok(((row.get(0)?, row.get(1)?), (row.get(2)?, row.get(3)?)))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect();
    let mut seen = HashSet::new();
    let mut source_status = Vec::new();
    for (agent, root) in [
        ("Codex", home().join(".codex")),
        ("Claude", home().join(".claude\\projects")),
        ("Cursor", home().join(".cursor\\projects")),
    ] {
        let (state, detail) = sync_history_source_incremental(
            db,
            agent,
            &[root],
            &["jsonl"],
            false,
            &skills,
            &cached_state,
            &mut seen,
        )?;
        source_status.push((agent.to_string(), state, detail));
    }
    let (state, detail) = sync_history_source_incremental(
        db,
        "Antigravity",
        &[
            home().join(".gemini\\antigravity\\brain"),
            home().join(".gemini\\antigravity-ide\\brain"),
        ],
        &["json", "jsonl", ""],
        true,
        &skills,
        &cached_state,
        &mut seen,
    )?;
    source_status.push(("Antigravity".into(), state, detail));
    source_status.push((
        "Gemini".into(),
        "unsupported".into(),
        "Gemini history is parsed through Antigravity sources".into(),
    ));
    source_status.push((
        "Agents".into(),
        "unsupported".into(),
        "No supported session history adapter found".into(),
    ));

    let stale_files: Vec<(String, String)> = cached_state
        .keys()
        .filter(|key| !seen.contains(*key))
        .cloned()
        .collect();
    for (agent, path) in stale_files {
        db.execute(
            "DELETE FROM timeline_file_events WHERE agent=?1 AND path=?2",
            params![agent, path],
        )
        .map_err(|error| error.to_string())?;
        db.execute(
            "DELETE FROM timeline_file_state WHERE agent=?1 AND path=?2",
            params![agent, path],
        )
        .map_err(|error| error.to_string())?;
    }
    rebuild_timeline_events(db)?;
    let transaction = db.transaction().map_err(|error| error.to_string())?;
    for (agent, state, detail) in source_status {
        transaction.execute("INSERT INTO skill_sources(agent,last_scan,detail) VALUES(?1,?2,?3) ON CONFLICT(agent) DO UPDATE SET last_scan=excluded.last_scan,detail=excluded.detail",
      params![agent, now(), format!("{}: {}", state, detail)]).map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|v| v.to_str())
            .map(|v| v.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "txt" | "json" | "jsonl" | "csv" | "yaml" | "yml")
    )
}
fn skip_dir(entry: &DirEntry) -> bool {
    !entry.file_type().is_dir()
        || !matches!(
            entry.file_name().to_str(),
            Some(".git" | ".hg" | ".svn" | "node_modules" | "target")
        )
}
fn fnv_hash(content: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in content.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
fn excerpt(content: &str) -> String {
    content
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(MAX_EXCERPT_CHARS)
        .collect::<String>()
        .trim()
        .replace('\n', " ")
}
fn title_for(path: &Path) -> String {
    path.file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("未命名文件")
        .to_string()
}
fn sync_knowledge(db: &Connection) -> Result<(), String> {
    let ignored_sources: HashSet<String> = db
        .prepare("SELECT source_path FROM ignored_sources")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    let mut statement = db
        .prepare("SELECT id,name,kind,path FROM knowledge_roots WHERE enabled=1")
        .map_err(|e| e.to_string())?;
    let roots: Vec<(i64, String, String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    for (root_id, name, kind, root_path) in roots {
        let root = PathBuf::from(&root_path);
        if !root.exists() {
            db.execute(
                "UPDATE knowledge_roots SET detail='目录不存在或无权访问',last_scan=?2 WHERE id=?1",
                params![root_id, now()],
            )
            .map_err(|e| e.to_string())?;
            continue;
        }
        db.execute(
            "UPDATE knowledge_items SET available=0 WHERE source_root_id=?1",
            [root_id],
        )
        .map_err(|e| e.to_string())?;
        let mut indexed = 0;
        let mut skipped = 0;
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(skip_dir)
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if ignored_sources.contains(&normalized(path)) {
                skipped += 1;
                continue;
            }
            if !is_supported(path) {
                skipped += 1;
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(v) => v,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if metadata.len() > MAX_INDEXED_BYTES {
                skipped += 1;
                continue;
            }
            let body = match fs::read_to_string(path) {
                Ok(v) => v,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let item_kind = if kind == "agent_artifact" {
                "agent_artifact"
            } else {
                "file"
            };
            db.execute("INSERT INTO knowledge_items(title,kind,source_root_id,source_path,capture_kind,source_uri,content_hash,excerpt,body,status,available,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,NULL,?6,?7,?8,'inbox',1,?9,?9) ON CONFLICT(source_path) DO UPDATE SET title=excluded.title,kind=excluded.kind,source_root_id=excluded.source_root_id,capture_kind=excluded.capture_kind,content_hash=excluded.content_hash,excerpt=excluded.excerpt,body=excluded.body,available=1,updated_at=excluded.updated_at", params![title_for(path),item_kind,root_id,normalized(path),item_kind,fnv_hash(&body),excerpt(&body),body,now()]).map_err(|e| e.to_string())?;
            indexed += 1;
        }
        db.execute(
            "UPDATE knowledge_roots SET last_scan=?2,detail=?3 WHERE id=?1",
            params![
                root_id,
                now(),
                format!("{name}：已发现 {indexed} 项，跳过 {skipped} 项")
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn tags_for(db: &Connection, item_id: i64) -> Result<Vec<String>, String> {
    let mut s = db.prepare("SELECT t.name FROM tags t JOIN knowledge_item_tags kt ON kt.tag_id=t.id WHERE kt.item_id=?1 ORDER BY t.name").map_err(|e| e.to_string())?;
    let rows = s
        .query_map([item_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())
}
fn skills_for(db: &Connection, item_id: i64) -> Result<Vec<i64>, String> {
    let mut s = db
        .prepare("SELECT skill_id FROM knowledge_item_skills WHERE item_id=?1")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([item_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<i64>, _>>()
        .map_err(|e| e.to_string())
}
fn row_item(db: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeItem> {
    let id: i64 = row.get(0)?;
    Ok(KnowledgeItem {
        id,
        title: row.get(1)?,
        kind: row.get(2)?,
        source_path: row.get(3)?,
        capture_kind: row.get(4)?,
        source_uri: row.get(5)?,
        excerpt: row.get(6)?,
        body: row.get(7)?,
        status: row.get(8)?,
        project_id: row.get(9)?,
        project_title: row.get(10)?,
        available: row.get::<_, i64>(11)? == 1,
        updated_at: row.get(12)?,
        tags: tags_for(db, id).unwrap_or_default(),
        skill_ids: skills_for(db, id).unwrap_or_default(),
    })
}
fn set_sync_state(
    db: &Connection,
    stage: &str,
    state: &str,
    detail: &str,
    started_at: Option<&str>,
    finished_at: Option<&str>,
) -> Result<(), String> {
    for (key, value) in [
        ("sync_stage", stage.to_string()),
        ("sync_state", state.to_string()),
        ("sync_detail", detail.to_string()),
        (
            "sync_started_at",
            started_at.unwrap_or_default().to_string(),
        ),
        (
            "sync_finished_at",
            finished_at.unwrap_or_default().to_string(),
        ),
    ] {
        db.execute(
            "INSERT INTO sync_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn sync_state_value(db: &Connection, key: &str) -> Option<String> {
    db.query_row("SELECT value FROM sync_state WHERE key=?1", [key], |row| {
        row.get(0)
    })
    .ok()
}
fn current_sync_status(db: &Connection) -> SyncStatus {
    SyncStatus {
        stage: sync_state_value(db, "sync_stage").unwrap_or_else(|| "idle".into()),
        state: sync_state_value(db, "sync_state").unwrap_or_else(|| "idle".into()),
        detail: sync_state_value(db, "sync_detail").unwrap_or_default(),
        started_at: sync_state_value(db, "sync_started_at").filter(|value| !value.is_empty()),
        finished_at: sync_state_value(db, "sync_finished_at").filter(|value| !value.is_empty()),
    }
}
fn set_tags(db: &Connection, item_id: i64, tags: &[String]) -> Result<(), String> {
    db.execute(
        "DELETE FROM knowledge_item_tags WHERE item_id=?1",
        [item_id],
    )
    .map_err(|e| e.to_string())?;
    for tag in tags.iter().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        db.execute("INSERT OR IGNORE INTO tags(name) VALUES(?1)", [tag])
            .map_err(|e| e.to_string())?;
        db.execute("INSERT OR IGNORE INTO knowledge_item_tags(item_id,tag_id) SELECT ?1,id FROM tags WHERE name=?2", params![item_id,tag]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn refresh_all(db: State<Db>) -> Result<(), String> {
    let mut db = lock(&db)?;
    let started_at = now();
    set_sync_state(
        &db,
        "skills",
        "running",
        "正在扫描 Skill 来源",
        Some(&started_at),
        None,
    )?;
    if let Err(error) = scan_skills(&db) {
        let detail = format!("Skill 来源扫描失败：{error}");
        let _ = set_sync_state(
            &db,
            "skills",
            "failed",
            &detail,
            Some(&started_at),
            Some(&now()),
        );
        return Err(detail);
    }
    set_sync_state(
        &db,
        "timeline",
        "running",
        "正在同步 Agent 历史",
        Some(&started_at),
        None,
    )?;
    if let Err(error) = sync_timeline(&mut db) {
        let detail = format!("Agent 历史同步失败：{error}");
        let _ = set_sync_state(
            &db,
            "timeline",
            "failed",
            &detail,
            Some(&started_at),
            Some(&now()),
        );
        return Err(detail);
    }
    set_sync_state(
        &db,
        "knowledge",
        "running",
        "正在扫描知识来源",
        Some(&started_at),
        None,
    )?;
    if let Err(error) = sync_knowledge(&db) {
        let detail = format!("知识来源同步失败：{error}");
        let _ = set_sync_state(
            &db,
            "knowledge",
            "failed",
            &detail,
            Some(&started_at),
            Some(&now()),
        );
        return Err(detail);
    }
    set_sync_state(
        &db,
        "idle",
        "success",
        "同步完成",
        Some(&started_at),
        Some(&now()),
    )
}
#[tauri::command]
fn sync_knowledge_now(db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    sync_knowledge(&db)
}
#[tauri::command]
fn list_skills(db: State<Db>) -> Result<Vec<Skill>, String> {
    let db = lock(&db)?;
    let mut s = db.prepare("SELECT s.id,s.name,s.description,s.body,s.agent,s.path,s.custom,s.available,EXISTS(SELECT 1 FROM usage_cards c WHERE c.skill_id=s.id) FROM skills s ORDER BY s.name").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(Skill {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                body: r.get(3)?,
                agent: r.get(4)?,
                path: r.get(5)?,
                custom: r.get::<_, i64>(6)? == 1,
                available: r.get::<_, i64>(7)? == 1,
                has_card: r.get::<_, i64>(8)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
fn skill_rows(db: &Connection) -> Result<Vec<Skill>, String> {
    let mut s = db.prepare("SELECT s.id,s.name,s.description,s.body,s.agent,s.path,s.custom,s.available,EXISTS(SELECT 1 FROM usage_cards c WHERE c.skill_id=s.id) FROM skills s ORDER BY s.name,s.agent,s.path").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(Skill {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                body: r.get(3)?,
                agent: r.get(4)?,
                path: r.get(5)?,
                custom: r.get::<_, i64>(6)? == 1,
                available: r.get::<_, i64>(7)? == 1,
                has_card: r.get::<_, i64>(8)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
fn variant_from_skill(skill: &Skill, metadata: &SkillMetadata) -> SkillVariant {
    SkillVariant {
        id: skill.id,
        name: skill.name.clone(),
        description: skill.description.clone(),
        body: skill.body.clone(),
        content: skill_content(&skill.body),
        agent: skill.agent.clone(),
        path: skill.path.clone(),
        custom: skill.custom,
        available: skill.available,
        has_card: skill.has_card,
        parent_key: metadata.parent.as_ref().map(|name| skill_key(name)),
        children_keys: metadata
            .children
            .iter()
            .map(|name| skill_key(name))
            .collect(),
        github_url: metadata
            .github_url
            .clone()
            .or_else(|| known_github_url(&skill.name, &skill.agent)),
    }
}
fn cycle_nodes(adjacency: &HashMap<String, Vec<String>>, keys: &[String]) -> HashSet<String> {
    fn visit(
        node: &str,
        adjacency: &HashMap<String, Vec<String>>,
        state: &mut HashMap<String, u8>,
        stack: &mut Vec<String>,
        cycles: &mut HashSet<String>,
    ) {
        state.insert(node.to_string(), 1);
        stack.push(node.to_string());
        if let Some(children) = adjacency.get(node) {
            for child in children {
                match state.get(child).copied().unwrap_or(0) {
                    0 => visit(child, adjacency, state, stack, cycles),
                    1 => {
                        if let Some(index) = stack.iter().position(|item| item == child) {
                            cycles.extend(stack[index..].iter().cloned());
                        }
                    }
                    _ => {}
                }
            }
        }
        stack.pop();
        state.insert(node.to_string(), 2);
    }
    let mut state = HashMap::new();
    let mut stack = Vec::new();
    let mut cycles = HashSet::new();
    for key in keys {
        if state.get(key).copied().unwrap_or(0) == 0 {
            visit(key, adjacency, &mut state, &mut stack, &mut cycles);
        }
    }
    cycles
}
fn function_catalog(key: &str) -> (&'static str, &'static str) {
    match key {
        "design" => ("设计与产品", "界面、体验、原型和产品设计流程"),
        "planning" => ("规划与执行", "任务拆解、持续规划和多阶段执行"),
        "skill-lifecycle" => ("Skill 生命周期", "创建、安装、更新和扩展 Skill 能力"),
        "quality" => ("审查与质量", "代码审查、审计、回顾和质量保障"),
        "research" => ("研究与知识", "资料检索、文档理解和上下文收集"),
        "platform" => ("平台与集成", "Agent 平台、插件、仓库和工具集成"),
        _ => ("其他能力", "暂未匹配到明确功能的 Skill"),
    }
}
fn function_keys(name: &str, description: &str, _content: &str) -> Vec<String> {
    let text = format!("{name} {description}").to_ascii_lowercase();
    let mut keys = Vec::new();
    if [
        "design",
        "product",
        "prototype",
        "ui",
        "ux",
        "visual",
        "interface",
        "ideate",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        keys.push("design".to_string());
    }
    if [
        "plan",
        "planning",
        "task",
        "workflow",
        "progress",
        "session",
        "grill",
        "interview",
        "roadmap",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        keys.push("planning".to_string());
    }
    if [
        "skill-creator",
        "skill-installer",
        "plugin-creator",
        "skill lifecycle",
        "create skill",
        "install skill",
        "update skill",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        keys.push("skill-lifecycle".to_string());
    }
    if [
        "review",
        "audit",
        "qa",
        "quality",
        "retrospective",
        "test",
        "verify",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        keys.push("quality".to_string());
    }
    if [
        "research",
        "docs",
        "document",
        "knowledge",
        "context",
        "search",
        "findings",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        keys.push("research".to_string());
    }
    if [
        "antigravity",
        "openai",
        "github",
        "repository",
        "integration",
        "adapter",
    ]
    .iter()
    .any(|word| text.contains(word))
    {
        keys.push("platform".to_string());
    }
    if keys.is_empty() {
        keys.push("other".to_string());
    }
    keys.sort();
    keys.dedup();
    keys
}
fn build_skill_library(skills: &[Skill]) -> SkillLibrary {
    let mut groups: HashMap<
        String,
        (
            String,
            String,
            Vec<SkillVariant>,
            BTreeSet<String>,
            BTreeSet<String>,
            BTreeSet<String>,
        ),
    > = HashMap::new();
    for skill in skills {
        let metadata = parse_skill_metadata(&skill.body, Path::new(&skill.path));
        let key = skill_key(&skill.name);
        let entry = groups.entry(key.clone()).or_insert_with(|| {
            (
                skill.name.clone(),
                skill.description.clone(),
                Vec::new(),
                BTreeSet::new(),
                BTreeSet::new(),
                BTreeSet::new(),
            )
        });
        if entry.1.is_empty() && !skill.description.is_empty() {
            entry.1 = skill.description.clone();
        }
        entry.2.push(variant_from_skill(skill, &metadata));
    }
    let group_keys: HashSet<String> = groups.keys().cloned().collect();
    let mut edge_keys = BTreeSet::new();
    for skill in skills {
        let key = skill_key(&skill.name);
        let metadata = parse_skill_metadata(&skill.body, Path::new(&skill.path));
        if let Some(parent) = metadata.parent {
            let parent_key = skill_key(&parent);
            edge_keys.insert((parent_key, key.clone(), "parent".to_string()));
        }
        for child in metadata.children {
            edge_keys.insert((key.clone(), skill_key(&child), "parent".to_string()));
        }
    }
    let mut relations = Vec::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for (source, target, relation) in edge_keys {
        let unresolved = !group_keys.contains(&source) || !group_keys.contains(&target);
        relations.push(SkillRelation {
            source: source.clone(),
            target: target.clone(),
            relation,
            unresolved,
        });
        if unresolved {
            if let Some(group) = groups.get_mut(&if group_keys.contains(&source) {
                source.clone()
            } else {
                target.clone()
            }) {
                group.5.insert(if group_keys.contains(&source) {
                    target.clone()
                } else {
                    source.clone()
                });
            }
        } else {
            groups.get_mut(&source).unwrap().4.insert(target.clone());
            groups.get_mut(&target).unwrap().3.insert(source.clone());
            adjacency.entry(source).or_default().push(target);
        }
    }
    let keys: Vec<String> = groups.keys().cloned().collect();
    let cycles = cycle_nodes(&adjacency, &keys);
    let mut output = groups
        .into_iter()
        .map(
            |(key, (name, description, mut variants, parents, children, unresolved))| {
                variants.sort_by(|a, b| a.agent.cmp(&b.agent).then(a.path.cmp(&b.path)));
                let cycle = cycles.contains(&key);
                let root = parents.is_empty() || cycle;
                SkillGroup {
                    key: key.clone(),
                    name,
                    description,
                    variants,
                    parents: parents.into_iter().collect(),
                    children: children.into_iter().collect(),
                    unresolved_relations: unresolved.into_iter().collect(),
                    function_keys: Vec::new(),
                    cycle,
                    root,
                }
            },
        )
        .collect::<Vec<_>>();
    output.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    let mut function_map: HashMap<String, (BTreeSet<String>, BTreeSet<String>)> = HashMap::new();
    for group in &mut output {
        let content = group
            .variants
            .iter()
            .map(|variant| variant.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        group.function_keys = function_keys(&group.name, &group.description, &content);
        for key in &group.function_keys {
            let entry = function_map
                .entry(key.clone())
                .or_insert_with(|| (BTreeSet::new(), BTreeSet::new()));
            entry.0.insert(group.key.clone());
            for variant in &group.variants {
                if let Some(url) = &variant.github_url {
                    entry.1.insert(url.clone());
                }
            }
        }
    }
    let mut function_groups = function_map
        .into_iter()
        .map(|(key, (skill_keys, github_references))| {
            let (name, description) = function_catalog(&key);
            SkillFunctionGroup {
                key,
                name: name.to_string(),
                description: description.to_string(),
                skill_keys: skill_keys.into_iter().collect(),
                github_references: github_references.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();
    function_groups.sort_by(|a, b| a.name.cmp(&b.name));
    let function_relations = function_groups
        .iter()
        .flat_map(|group| {
            group
                .skill_keys
                .iter()
                .map(|skill_key| SkillFunctionRelation {
                    source: group.key.clone(),
                    target: skill_key.clone(),
                    relation: "contains".to_string(),
                })
        })
        .collect();
    SkillLibrary {
        groups: output,
        relations,
        function_groups,
        function_relations,
    }
}
#[tauri::command]
fn list_skill_groups(db: State<Db>) -> Result<SkillLibrary, String> {
    let db = lock(&db)?;
    Ok(build_skill_library(&skill_rows(&db)?))
}
#[tauri::command]
fn get_skill_group_detail(group_key: String, db: State<Db>) -> Result<SkillGroupDetail, String> {
    let db = lock(&db)?;
    let library = build_skill_library(&skill_rows(&db)?);
    let key = skill_key(&group_key);
    let group = library
        .groups
        .iter()
        .find(|item| item.key == key)
        .cloned()
        .ok_or_else(|| "Skill 不存在".to_string())?;
    let related_keys = group
        .parents
        .iter()
        .chain(group.children.iter())
        .cloned()
        .collect::<HashSet<_>>();
    let related_groups = library
        .groups
        .iter()
        .filter(|item| related_keys.contains(&item.key))
        .cloned()
        .collect();
    Ok(SkillGroupDetail {
        group,
        related_groups,
        relations: library.relations,
    })
}
#[tauri::command]
fn get_usage_card(skill_id: i64, db: State<Db>) -> Result<Option<Card>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT skill_id,scenarios,triggers,steps,notes,pitfalls,links,tags FROM usage_cards WHERE skill_id=?1").map_err(|e|e.to_string())?;
    let mut rows = s.query([skill_id]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(r) => Ok(Some(Card {
            skill_id: r.get(0).map_err(|e| e.to_string())?,
            scenarios: r.get(1).map_err(|e| e.to_string())?,
            triggers: r.get(2).map_err(|e| e.to_string())?,
            steps: r.get(3).map_err(|e| e.to_string())?,
            notes: r.get(4).map_err(|e| e.to_string())?,
            pitfalls: r.get(5).map_err(|e| e.to_string())?,
            links: r.get(6).map_err(|e| e.to_string())?,
            tags: r.get(7).map_err(|e| e.to_string())?,
        })),
        None => Ok(None),
    }
}
#[tauri::command]
fn save_usage_card(card: Card, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    db.execute("INSERT INTO usage_cards(skill_id,scenarios,triggers,steps,notes,pitfalls,links,tags,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(skill_id) DO UPDATE SET scenarios=excluded.scenarios,triggers=excluded.triggers,steps=excluded.steps,notes=excluded.notes,pitfalls=excluded.pitfalls,links=excluded.links,tags=excluded.tags,updated_at=excluded.updated_at",params![card.skill_id,card.scenarios,card.triggers,card.steps,card.notes,card.pitfalls,card.links,card.tags,now()]).map_err(|e|e.to_string())?;
    Ok(())
}
#[tauri::command]
fn list_timeline(db: State<Db>) -> Result<Vec<Event>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT id,at,agent,skill,session_id,occurrences,project_path,summary,parse_status,timestamp_quality FROM timeline_events ORDER BY at DESC").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(Event {
                id: r.get(0)?,
                at: r.get(1)?,
                agent: r.get(2)?,
                skill: r.get(3)?,
                session_id: r.get(4)?,
                occurrences: r.get(5)?,
                project_path: r.get(6)?,
                summary: r.get(7)?,
                parse_status: r.get(8)?,
                timestamp_quality: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn adapter_status(db: State<Db>) -> Result<Vec<AdapterStatus>, String> {
    let db = lock(&db)?;
    let mut s = db
        .prepare("SELECT agent,detail,last_scan FROM skill_sources ORDER BY agent")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            let d: String = r.get(1)?;
            let (state, detail) = d.split_once(": ").unwrap_or(("pending", &d));
            Ok(AdapterStatus {
                agent: r.get(0)?,
                state: state.into(),
                detail: detail.into(),
                last_sync: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
fn first_command_path(command: &str) -> Option<String> {
    Command::new("where.exe")
        .arg(command)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let paths = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            paths
                .iter()
                .find(|path| {
                    matches!(
                        Path::new(path)
                            .extension()
                            .and_then(|value| value.to_str())
                            .map(|value| value.to_ascii_lowercase())
                            .as_deref(),
                        Some("cmd" | "exe" | "bat" | "ps1")
                    )
                })
                .cloned()
                .or_else(|| paths.into_iter().next())
        })
}
fn powershell_literal(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}
fn powershell_argument_literal(path: &Path) -> String {
    format!(
        "'\"{}\"'",
        path.to_string_lossy().replace('\'', "''").replace('"', "'")
    )
}
#[tauri::command]
fn probe_agents() -> Result<Vec<AgentProbe>, String> {
    let specs = [
        ("Codex", "codex"),
        ("Claude", "claude"),
        ("Cursor", "cursor"),
    ];
    Ok(specs
        .into_iter()
        .map(|(agent, command)| {
            let executable = first_command_path(command);
            let (state, detail) = if executable.is_some() {
                (
                    "available",
                    format!("已找到 {command} 命令，可继续验证启动参数。"),
                )
            } else {
                (
                    "missing",
                    format!("未在 PATH 中找到 {command}，可在设置中补充命令路径。"),
                )
            };
            AgentProbe {
                agent: agent.into(),
                state: state.into(),
                command: command.into(),
                executable,
                launch_mode: "打开应用 + 复制 Prompt（自动发送待验证）".into(),
                detail,
            }
        })
        .collect())
}
#[tauri::command]
fn launch_agent(agent: String, working_dir: Option<String>) -> Result<(), String> {
    let command = match agent.trim().to_ascii_lowercase().as_str() {
        "codex" => "codex",
        "claude" => "claude",
        "cursor" => "cursor",
        _ => return Err("暂不支持该 Agent".into()),
    };
    let executable = first_command_path(command)
        .ok_or_else(|| format!("未找到 {agent} 命令，请先检查 PATH 或安装配置"))?;
    let executable_path = PathBuf::from(&executable);
    let cwd = match working_dir.filter(|value| !value.trim().is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value)
                .canonicalize()
                .map_err(|_| "工作目录不存在或无权访问".to_string())?;
            if !path.is_dir() {
                return Err("工作目录必须是文件夹".into());
            }
            Some(path)
        }
        None => None,
    };
    let cwd_arg = cwd
        .as_ref()
        .map(|path| format!(" -WorkingDirectory {}", powershell_literal(path)))
        .unwrap_or_default();
    let extension = executable_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let script = if extension == "cmd" || extension == "bat" {
        format!(
            "Start-Process -FilePath $env:ComSpec{} -ArgumentList @('/D','/C',{})",
            cwd_arg,
            powershell_argument_literal(&executable_path)
        )
    } else if extension == "ps1" {
        format!("Start-Process -FilePath 'powershell.exe'{} -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',{})", cwd_arg, powershell_argument_literal(&executable_path))
    } else {
        format!(
            "Start-Process -FilePath {}{}",
            powershell_literal(&executable_path),
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
fn get_sync_status(db: State<Db>) -> Result<SyncStatus, String> {
    let db = lock(&db)?;
    Ok(current_sync_status(&db))
}
#[tauri::command]
fn list_scan_roots(db: State<Db>) -> Result<Vec<ScanRoot>, String> {
    let db = lock(&db)?;
    let mut s = db
        .prepare("SELECT id,agent,path,enabled,custom FROM scan_roots ORDER BY agent")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(ScanRoot {
                id: r.get(0)?,
                agent: r.get(1)?,
                path: r.get(2)?,
                enabled: r.get::<_, i64>(3)? == 1,
                custom: r.get::<_, i64>(4)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn add_scan_root(agent: String, path: String, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|_| "目录不存在或无权访问".to_string())?;
    if !path.is_dir() {
        return Err("扫描路径必须是目录".into());
    }
    db.execute(
        "INSERT INTO scan_roots(agent,path,custom) VALUES(?1,?2,1)",
        params![agent, path.to_string_lossy()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn remove_scan_root(id: i64, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    if db
        .execute("DELETE FROM scan_roots WHERE id=?1 AND custom=1", [id])
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("只能删除自定义目录".into());
    }
    Ok(())
}
#[tauri::command]
fn clear_timeline(db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    db.execute("DELETE FROM timeline_events", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn list_projects(db: State<Db>) -> Result<Vec<Project>, String> {
    let db = lock(&db)?;
    let mut s = db
        .prepare("SELECT id,title,path,updated_at FROM projects ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                title: r.get(1)?,
                path: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn save_project(project: ProjectInput, db: State<Db>) -> Result<Project, String> {
    let db = lock(&db)?;
    let title = project.title.trim();
    if title.is_empty() {
        return Err("项目名称不能为空".into());
    }
    let path = match project.path.filter(|v| !v.trim().is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value)
                .canonicalize()
                .map_err(|_| "项目目录不存在或无权访问".to_string())?;
            if !path.is_dir() {
                return Err("项目路径必须是目录".into());
            }
            Some(normalized(&path))
        }
        None => None,
    };
    if let Some(id) = project.id {
        db.execute(
            "UPDATE projects SET title=?1,path=?2,updated_at=?3 WHERE id=?4",
            params![title, path, now(), id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        db.execute(
            "INSERT INTO projects(title,path,updated_at) VALUES(?1,?2,?3)",
            params![title, path, now()],
        )
        .map_err(|e| e.to_string())?;
    }
    let id = project.id.unwrap_or_else(|| db.last_insert_rowid());
    db.query_row(
        "SELECT id,title,path,updated_at FROM projects WHERE id=?1",
        [id],
        |r| {
            Ok(Project {
                id: r.get(0)?,
                title: r.get(1)?,
                path: r.get(2)?,
                updated_at: r.get(3)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn delete_project(id: i64, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    db.execute(
        "UPDATE knowledge_items SET project_id=NULL WHERE project_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE knowledge_roots SET project_id=NULL WHERE project_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    db.execute("DELETE FROM projects WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn list_skill_update_commands(db: State<Db>) -> Result<Vec<SkillUpdateCommand>, String> {
    let db = lock(&db)?;
    let mut s = db
        .prepare("SELECT agent,command,enabled FROM skill_update_commands ORDER BY agent")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(SkillUpdateCommand {
                agent: r.get(0)?,
                command: r.get(1)?,
                enabled: r.get::<_, i64>(2)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn save_skill_update_command(config: SkillUpdateCommandInput, db: State<Db>) -> Result<(), String> {
    let agent = config.agent.trim();
    if agent.is_empty() {
        return Err("Agent 不能为空".into());
    }
    let db = lock(&db)?;
    db.execute("INSERT INTO skill_update_commands(agent,command,enabled,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(agent) DO UPDATE SET command=excluded.command,enabled=excluded.enabled,updated_at=excluded.updated_at",params![agent,config.command.trim(),config.enabled as i64,now()]).map_err(|e|e.to_string())?;
    Ok(())
}
#[tauri::command]
fn refresh_skills(db: State<Db>) -> Result<SkillLibrary, String> {
    let db = lock(&db)?;
    scan_skills(&db)?;
    Ok(build_skill_library(&skill_rows(&db)?))
}
#[tauri::command]
fn list_skill_copy_targets(_db: State<Db>) -> Result<Vec<SkillCopyTarget>, String> {
    Ok(built_in_skill_roots()
        .into_iter()
        .map(|(agent, path)| SkillCopyTarget {
            agent,
            path: path.to_string_lossy().to_string(),
            available: path.is_dir(),
        })
        .collect())
}
#[tauri::command]
fn copy_skill_to_agent(
    skill_id: i64,
    target_agent: String,
    db: State<Db>,
) -> Result<CopySkillResult, String> {
    let target_agent = target_agent.trim().to_string();
    if target_agent.is_empty() {
        return Err("请选择目标 Agent".into());
    }
    let db = lock(&db)?;
    let (_source_name, source_agent, source_path) = db
        .query_row(
            "SELECT name,agent,path FROM skills WHERE id=?1",
            [skill_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|_| "Skill 不存在".to_string())?;
    if source_agent.eq_ignore_ascii_case(&target_agent) {
        return Err("不能复制到当前来源 Agent".into());
    }
    let source_path = PathBuf::from(&source_path);
    if !source_path.is_file() {
        return Err("当前 Skill 来源文件不存在或已失效".into());
    }
    let source_dir = source_path
        .parent()
        .ok_or_else(|| "无法确定 Skill 所在目录".to_string())?
        .to_path_buf();
    let target_root = default_skill_root(&db, &target_agent)?;
    fs::create_dir_all(&target_root)
        .map_err(|error| format!("无法创建目标 Agent 目录：{error}"))?;
    let folder_name = source_dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法确定 Skill 目录名称".to_string())?
        .to_string();
    let target_dir = target_root.join(&folder_name);
    if normalized(&source_dir) == normalized(&target_dir) {
        return Err("来源目录和目标目录相同，已停止复制".into());
    }
    let temp_dir = target_root.join(format!(
        ".{folder_name}.copying-{}",
        fnv_hash(&format!("{skill_id}:{}", now()))
    ));
    remove_path(&temp_dir)?;
    let copied_files = match copy_directory(&source_dir, &temp_dir) {
        Ok(count) => count,
        Err(error) => {
            let _ = remove_path(&temp_dir);
            return Err(error);
        }
    };
    let backup = if target_dir.exists() {
        let path = backup_path(&target_root, &folder_name);
        fs::rename(&target_dir, &path).map_err(|error| {
            let _ = remove_path(&temp_dir);
            format!("备份目标 Skill 失败：{error}")
        })?;
        Some(path)
    } else {
        None
    };
    let install_result = fs::rename(&temp_dir, &target_dir);
    if let Err(error) = install_result {
        let _ = remove_path(&temp_dir);
        if let Some(backup_path) = &backup {
            let _ = remove_path(&target_dir);
            let _ = fs::rename(backup_path, &target_dir);
        }
        return Err(format!("写入目标 Skill 失败：{error}"));
    }
    let rescanned = scan_skills(&db).is_ok();
    Ok(CopySkillResult {
        source_agent,
        source_path: source_path.to_string_lossy().to_string(),
        target_agent,
        target_path: target_dir.to_string_lossy().to_string(),
        backup_path: backup.map(|path| path.to_string_lossy().to_string()),
        copied_files,
        rescanned,
    })
}
#[tauri::command]
fn run_skill_update(
    skill_id: i64,
    app: AppHandle,
    db: State<Db>,
) -> Result<SkillUpdateRun, String> {
    let (name, agent, path, template, enabled) = {
        let db = lock(&db)?;
        let skill = db
            .query_row(
                "SELECT name,agent,path FROM skills WHERE id=?1",
                [skill_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .map_err(|_| "Skill 不存在".to_string())?;
        let config = db
            .query_row(
                "SELECT command,enabled FROM skill_update_commands WHERE agent=?1",
                [&skill.1],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? == 1)),
            )
            .map_err(|_| "该 Agent 尚未配置更新命令".to_string())?;
        (skill.0, skill.1, PathBuf::from(skill.2), config.0, config.1)
    };
    if !enabled || template.trim().is_empty() {
        return Err(format!("{} 尚未启用更新命令", agent));
    }
    if !path.is_file() {
        return Err("当前 Skill 文件不存在或已失效".into());
    }
    let cwd = path
        .parent()
        .ok_or_else(|| "无法确定 Skill 工作目录".to_string())?
        .to_path_buf();
    let command = expand_skill_update_command(&template, &name, &path, &agent);
    let run_id = format!("skill-{}", fnv_hash(&format!("{}:{}", skill_id, now())));
    let mut child = Command::new("cmd.exe")
        .args(["/D", "/S", "/C", &command])
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("更新命令启动失败：{e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let output_app = app.clone();
    let output_run_id = run_id.clone();
    let error_app = app.clone();
    let error_run_id = run_id.clone();
    let completion_app = app.clone();
    let completion_run_id = run_id.clone();
    thread::spawn(move || {
        let out_thread = thread::spawn(move || {
            if let Some(stream) = stdout {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    emit_skill_update(
                        &output_app,
                        SkillUpdateEvent {
                            run_id: output_run_id.clone(),
                            status: "output".into(),
                            stream: "stdout".into(),
                            line,
                            exit_code: None,
                        },
                    );
                }
            }
        });
        let err_thread = thread::spawn(move || {
            if let Some(stream) = stderr {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    emit_skill_update(
                        &error_app,
                        SkillUpdateEvent {
                            run_id: error_run_id.clone(),
                            status: "output".into(),
                            stream: "stderr".into(),
                            line,
                            exit_code: None,
                        },
                    );
                }
            }
        });
        let result = child.wait();
        let _ = out_thread.join();
        let _ = err_thread.join();
        let (status, code) = match result {
            Ok(value) if value.success() => ("success", value.code()),
            Ok(value) => ("failed", value.code()),
            Err(_) => ("failed", None),
        };
        emit_skill_update(
            &completion_app,
            SkillUpdateEvent {
                run_id: completion_run_id,
                status: status.into(),
                stream: "system".into(),
                line: if status == "success" {
                    "更新命令执行完成".into()
                } else {
                    "更新命令执行失败".into()
                },
                exit_code: code,
            },
        );
    });
    emit_skill_update(
        &app,
        SkillUpdateEvent {
            run_id: run_id.clone(),
            status: "started".into(),
            stream: "system".into(),
            line: "更新命令已启动".into(),
            exit_code: None,
        },
    );
    Ok(SkillUpdateRun {
        run_id,
        skill_id,
        command,
        cwd: cwd.to_string_lossy().to_string(),
        status: "running".into(),
    })
}
fn task_projects(db: &Connection, task_id: i64) -> Result<Vec<TaskProjectRef>, String> {
    let mut statement = db.prepare("SELECT p.id,p.title FROM task_projects tp JOIN projects p ON p.id=tp.project_id WHERE tp.task_id=?1 ORDER BY p.title COLLATE NOCASE").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([task_id], |row| {
            Ok(TaskProjectRef {
                id: row.get(0)?,
                title: row.get(1)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}
fn task_source(db: &Connection, task_id: i64) -> Result<Option<TaskSource>, String> {
    let mut statement = db.prepare("SELECT ts.id,ts.kind,ts.title,ts.uri,ts.content,ts.knowledge_item_id,ki.status FROM task_sources ts LEFT JOIN knowledge_items ki ON ki.id=ts.knowledge_item_id WHERE ts.task_id=?1 ORDER BY ts.id DESC LIMIT 1").map_err(|error| error.to_string())?;
    let mut rows = statement
        .query([task_id])
        .map_err(|error| error.to_string())?;
    match rows.next().map_err(|error| error.to_string())? {
        Some(row) => Ok(Some(TaskSource {
            id: row.get(0).map_err(|error| error.to_string())?,
            kind: row.get(1).map_err(|error| error.to_string())?,
            title: row.get(2).map_err(|error| error.to_string())?,
            uri: row.get(3).map_err(|error| error.to_string())?,
            content: row.get(4).map_err(|error| error.to_string())?,
            knowledge_item_id: row.get(5).map_err(|error| error.to_string())?,
            knowledge_item_status: row.get(6).map_err(|error| error.to_string())?,
        })),
        None => Ok(None),
    }
}
fn task_from_row(db: &Connection, row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let id: i64 = row.get(0)?;
    Ok(Task {
        id,
        title: row.get(1)?,
        objective: row.get(2)?,
        steps: row.get(3)?,
        status: row.get(4)?,
        priority: row.get(5)?,
        recommended_agent: row.get(6)?,
        recommended_skill: row.get(7)?,
        projects: task_projects(db, id).unwrap_or_default(),
        source: task_source(db, id).unwrap_or_default(),
        updated_at: row.get(8)?,
        created_at: row.get(9)?,
    })
}
fn task_query() -> &'static str {
    "SELECT id,title,objective,steps,status,priority,recommended_agent,recommended_skill,updated_at,created_at FROM tasks"
}
fn validate_task_status(status: &str) -> Result<(), String> {
    if matches!(
        status,
        "draft" | "ready" | "in_progress" | "blocked" | "done"
    ) {
        Ok(())
    } else {
        Err("未知任务状态".into())
    }
}
fn save_task_links(db: &Connection, task_id: i64, input: &TaskInput) -> Result<(), String> {
    db.execute("DELETE FROM task_projects WHERE task_id=?1", [task_id])
        .map_err(|error| error.to_string())?;
    for project_id in &input.project_ids {
        db.execute(
            "INSERT OR IGNORE INTO task_projects(task_id,project_id) VALUES(?1,?2)",
            params![task_id, project_id],
        )
        .map_err(|error| error.to_string())?;
    }
    db.execute("DELETE FROM task_sources WHERE task_id=?1", [task_id])
        .map_err(|error| error.to_string())?;
    let kind = input.source_kind.as_deref().unwrap_or("").trim();
    let title = input.source_title.as_deref().unwrap_or("").trim();
    let uri = input.source_uri.as_deref().unwrap_or("").trim();
    let content = input.source_content.as_deref().unwrap_or("");
    if let Some(knowledge_item_id) = input.source_knowledge_item_id {
        let linked_task: Option<i64> = db
            .query_row(
                "SELECT task_id FROM task_sources WHERE knowledge_item_id=?1 AND task_id<>?2 LIMIT 1",
                params![knowledge_item_id, task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if linked_task.is_some() {
            return Err("该收件箱来源已关联其他任务".into());
        }
    }
    if !kind.is_empty() || !title.is_empty() || !uri.is_empty() || !content.trim().is_empty() {
        db.execute("INSERT INTO task_sources(task_id,kind,title,uri,content,knowledge_item_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![task_id, if kind.is_empty() { "note" } else { kind }, title, uri, content, input.source_knowledge_item_id, now()]).map_err(|error| error.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn list_tasks(db: State<Db>) -> Result<Vec<Task>, String> {
    let db = lock(&db)?;
    let mut statement = db.prepare(&format!("{} ORDER BY CASE status WHEN 'done' THEN 2 WHEN 'blocked' THEN 1 ELSE 0 END,priority DESC,updated_at DESC", task_query())).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| task_from_row(&db, row))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}
#[tauri::command]
fn create_task(input: TaskInput, db: State<Db>) -> Result<Task, String> {
    if input.title.trim().is_empty() {
        return Err("任务标题不能为空".into());
    }
    validate_task_status(&input.status)?;
    let db = lock(&db)?;
    let time = now();
    db.execute("INSERT INTO tasks(title,objective,steps,status,priority,recommended_agent,recommended_skill,updated_at,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)", params![input.title.trim(), input.objective.trim(), input.steps.trim(), input.status, input.priority, input.recommended_agent.as_deref().map(str::trim).filter(|value| !value.is_empty()), input.recommended_skill.as_deref().map(str::trim).filter(|value| !value.is_empty()), time]).map_err(|error| error.to_string())?;
    let id = db.last_insert_rowid();
    save_task_links(&db, id, &input)?;
    db.query_row(&format!("{} WHERE id=?1", task_query()), [id], |row| {
        task_from_row(&db, row)
    })
    .map_err(|error| error.to_string())
}
#[tauri::command]
fn update_task(id: i64, input: TaskInput, db: State<Db>) -> Result<Task, String> {
    if input.title.trim().is_empty() {
        return Err("任务标题不能为空".into());
    }
    validate_task_status(&input.status)?;
    let db = lock(&db)?;
    if db.execute("UPDATE tasks SET title=?1,objective=?2,steps=?3,status=?4,priority=?5,recommended_agent=?6,recommended_skill=?7,updated_at=?8 WHERE id=?9", params![input.title.trim(), input.objective.trim(), input.steps.trim(), input.status, input.priority, input.recommended_agent.as_deref().map(str::trim).filter(|value| !value.is_empty()), input.recommended_skill.as_deref().map(str::trim).filter(|value| !value.is_empty()), now(), id]).map_err(|error| error.to_string())? == 0 { return Err("任务不存在".into()); }
    save_task_links(&db, id, &input)?;
    db.query_row(&format!("{} WHERE id=?1", task_query()), [id], |row| {
        task_from_row(&db, row)
    })
    .map_err(|error| error.to_string())
}
fn task_by_id(db: &Connection, task_id: i64) -> Result<Task, String> {
    db.query_row(&format!("{} WHERE id=?1", task_query()), [task_id], |row| {
        task_from_row(db, row)
    })
    .map_err(|error| error.to_string())
}
fn promote_knowledge_item_to_task_db(
    knowledge_item_id: i64,
    db: &Connection,
) -> Result<Task, String> {
    if let Some(task_id) = db
        .query_row::<i64, _, _>(
            "SELECT task_id FROM task_sources WHERE knowledge_item_id=?1 ORDER BY id DESC LIMIT 1",
            [knowledge_item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        return task_by_id(db, task_id);
    }
    let item = db
        .query_row(
            "SELECT title,kind,capture_kind,source_uri,body,status,project_id FROM knowledge_items WHERE id=?1",
            [knowledge_item_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "来源条目不存在".to_string())?;
    if item.5 != "inbox" {
        return Err("只有收件箱中的来源才能生成任务".into());
    }
    let project_id = item.6.ok_or_else(|| "请先将来源归属到工作区".to_string())?;
    let time = now();
    let tx = db
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO tasks(title,objective,steps,status,priority,updated_at,created_at) VALUES(?1,?2,'','ready',0,?3,?3)",
        params![item.0.trim(), item.4.trim(), time],
    )
    .map_err(|error| error.to_string())?;
    let task_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO task_projects(task_id,project_id) VALUES(?1,?2)",
        params![task_id, project_id],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO task_sources(task_id,kind,title,uri,content,knowledge_item_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            task_id,
            item.1,
            item.0,
            item.3.unwrap_or_default(),
            item.4,
            knowledge_item_id,
            time
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE knowledge_items SET status='archived',updated_at=?1 WHERE id=?2 AND status='inbox'",
        params![now(), knowledge_item_id],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    task_by_id(db, task_id)
}
#[tauri::command]
fn promote_knowledge_item_to_task(knowledge_item_id: i64, db: State<Db>) -> Result<Task, String> {
    let db = lock(&db)?;
    promote_knowledge_item_to_task_db(knowledge_item_id, &db)
}
fn link_knowledge_item_to_task_db(
    knowledge_item_id: i64,
    task_id: i64,
    db: &Connection,
) -> Result<Task, String> {
    if let Some(existing_task_id) = db
        .query_row::<i64, _, _>(
            "SELECT task_id FROM task_sources WHERE knowledge_item_id=?1 ORDER BY id DESC LIMIT 1",
            [knowledge_item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        if existing_task_id == task_id {
            return task_by_id(db, task_id);
        }
        return Err("该收件箱来源已关联其他任务".into());
    }
    let item = db
        .query_row(
            "SELECT title,kind,capture_kind,source_uri,body,status,project_id FROM knowledge_items WHERE id=?1",
            [knowledge_item_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "来源条目不存在".to_string())?;
    if item.5 != "inbox" {
        return Err("只有收件箱中的来源才能关联任务".into());
    }
    let project_id = item.6.ok_or_else(|| "请先将来源归属到工作区".to_string())?;
    if db
        .query_row("SELECT 1 FROM tasks WHERE id=?1", [task_id], |_| Ok(()))
        .optional()
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Err("目标任务不存在".into());
    }
    let project_matches: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM task_projects WHERE task_id=?1 AND project_id=?2",
            params![task_id, project_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if project_matches == 0 {
        return Err("来源和目标任务不属于同一工作区".into());
    }
    let source_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM task_sources WHERE task_id=?1",
            [task_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if source_count > 0 {
        return Err("一个任务最多关联一个来源".into());
    }
    let tx = db
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let time = now();
    tx.execute(
        "INSERT INTO task_sources(task_id,kind,title,uri,content,knowledge_item_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            task_id,
            item.1,
            item.0,
            item.3.unwrap_or_default(),
            item.4,
            knowledge_item_id,
            time
        ],
    )
    .map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE knowledge_items SET status='archived',updated_at=?1 WHERE id=?2 AND status='inbox'",
        params![now(), knowledge_item_id],
    )
    .map_err(|error| error.to_string())?;
    tx.commit().map_err(|error| error.to_string())?;
    task_by_id(db, task_id)
}
#[tauri::command]
fn link_knowledge_item_to_task(
    knowledge_item_id: i64,
    task_id: i64,
    db: State<Db>,
) -> Result<Task, String> {
    let db = lock(&db)?;
    link_knowledge_item_to_task_db(knowledge_item_id, task_id, &db)
}
fn workspace_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        path: row.get(3)?,
        color: row.get(4)?,
        updated_at: row.get(5)?,
        last_opened_at: row.get(6)?,
        inbox_count: row.get(7)?,
        knowledge_count: row.get(8)?,
        source_count: row.get(9)?,
    })
}
fn workspace_query() -> &'static str {
    "SELECT p.id,p.title,p.description,p.path,p.color,p.updated_at,p.last_opened_at,(SELECT COUNT(*) FROM knowledge_items k WHERE k.project_id=p.id AND k.status='inbox' AND k.available=1),(SELECT COUNT(*) FROM knowledge_items k WHERE k.project_id=p.id AND k.status='archived'),(SELECT COUNT(*) FROM knowledge_roots r WHERE r.project_id=p.id AND r.enabled=1) FROM projects p"
}
#[tauri::command]
fn list_workspaces(db: State<Db>) -> Result<Vec<Workspace>, String> {
    let db = lock(&db)?;
    let mut s = db
        .prepare(&format!(
            "{} ORDER BY COALESCE(p.last_opened_at,p.updated_at) DESC,p.title COLLATE NOCASE",
            workspace_query()
        ))
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], workspace_from_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn save_workspace(workspace: WorkspaceInput, db: State<Db>) -> Result<Workspace, String> {
    let db = lock(&db)?;
    let title = workspace.title.trim();
    if title.is_empty() {
        return Err("工作区名称不能为空".into());
    }
    let path = match workspace.path.filter(|v| !v.trim().is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value)
                .canonicalize()
                .map_err(|_| "工作区目录不存在或无权访问".to_string())?;
            if !path.is_dir() {
                return Err("工作区路径必须是目录".into());
            }
            Some(normalized(&path))
        }
        None => None,
    };
    let color = if workspace.color.trim().is_empty() {
        "violet"
    } else {
        workspace.color.trim()
    };
    if let Some(id) = workspace.id {
        db.execute("UPDATE projects SET title=?1,description=?2,path=?3,color=?4,updated_at=?5 WHERE id=?6",params![title,workspace.description.trim(),path,color,now(),id]).map_err(|e|e.to_string())?;
    } else {
        db.execute("INSERT INTO projects(title,description,path,color,updated_at,last_opened_at) VALUES(?1,?2,?3,?4,?5,?5)",params![title,workspace.description.trim(),path,color,now()]).map_err(|e|e.to_string())?;
    }
    let id = workspace.id.unwrap_or_else(|| db.last_insert_rowid());
    db.query_row(
        &format!("{} WHERE p.id=?1", workspace_query()),
        [id],
        workspace_from_row,
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn mark_workspace_opened(id: i64, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    if db
        .execute(
            "UPDATE projects SET last_opened_at=?1 WHERE id=?2",
            params![now(), id],
        )
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("工作区不存在".into());
    }
    Ok(())
}
#[tauri::command]
fn delete_workspace(id: i64, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    delete_workspace_db(id, &db)
}
fn delete_workspace_db(id: i64, db: &Connection) -> Result<(), String> {
    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE knowledge_items SET project_id=NULL WHERE project_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE knowledge_roots SET project_id=NULL WHERE project_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM task_projects WHERE project_id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM projects WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn list_knowledge_roots(db: State<Db>) -> Result<Vec<KnowledgeRoot>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT id,name,kind,path,project_id,enabled,last_scan,detail FROM knowledge_roots ORDER BY name").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(KnowledgeRoot {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                path: r.get(3)?,
                project_id: r.get(4)?,
                enabled: r.get::<_, i64>(5)? == 1,
                last_scan: r.get(6)?,
                detail: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn add_knowledge_root(
    name: String,
    kind: String,
    path: String,
    project_id: Option<i64>,
    db: State<Db>,
) -> Result<(), String> {
    if !matches!(kind.as_str(), "project" | "agent_artifact") {
        return Err("未知知识来源类型".into());
    }
    let db = lock(&db)?;
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|_| "目录不存在或无权访问".to_string())?;
    if !path.is_dir() {
        return Err("知识来源必须是目录".into());
    }
    db.execute(
        "INSERT INTO knowledge_roots(name,kind,path,project_id) VALUES(?1,?2,?3,?4)",
        params![name.trim(), kind, path.to_string_lossy(), project_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn remove_knowledge_root(id: i64, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    db.execute("DELETE FROM knowledge_roots WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn purge_knowledge_root(id: i64, db: State<Db>) -> Result<(), String> {
    let db = lock(&db)?;
    let tx = db.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM knowledge_item_tags WHERE item_id IN (SELECT id FROM knowledge_items WHERE source_root_id=?1)",[id]).map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM knowledge_item_skills WHERE item_id IN (SELECT id FROM knowledge_items WHERE source_root_id=?1)",[id]).map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM knowledge_items WHERE source_root_id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM knowledge_roots WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
#[allow(dead_code)]
fn clear_pending_sources(db: State<Db>) -> Result<i64, String> {
    let db = lock(&db)?;
    let tx = db
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let pending_paths: Vec<(String, Option<i64>)> = {
        let mut statement = tx
            .prepare("SELECT source_path,source_root_id FROM knowledge_items WHERE status='inbox' AND source_path IS NOT NULL")
            .map_err(|error| error.to_string())?;
        let paths = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .collect();
        paths
    };
    for (source_path, root_id) in pending_paths {
        tx.execute(
            "INSERT INTO ignored_sources(source_path,root_id,ignored_at,reason) VALUES(?1,?2,?3,'cleared_from_inbox') ON CONFLICT(source_path) DO UPDATE SET root_id=excluded.root_id,ignored_at=excluded.ignored_at,reason=excluded.reason",
            params![source_path, root_id, now()],
        )
        .map_err(|error| error.to_string())?;
    }
    tx.execute("DELETE FROM knowledge_item_tags WHERE item_id IN (SELECT id FROM knowledge_items WHERE status='inbox')", [])
        .map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM knowledge_item_skills WHERE item_id IN (SELECT id FROM knowledge_items WHERE status='inbox')", [])
        .map_err(|error| error.to_string())?;
    let deleted = tx
        .execute("DELETE FROM knowledge_items WHERE status='inbox'", [])
        .map_err(|error| error.to_string())? as i64;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(deleted)
}
#[tauri::command]
fn list_knowledge_items(db: State<Db>) -> Result<Vec<KnowledgeItem>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT k.id,k.title,k.kind,k.source_path,k.capture_kind,k.source_uri,k.excerpt,k.body,k.status,k.project_id,p.title,k.available,k.updated_at FROM knowledge_items k LEFT JOIN projects p ON p.id=k.project_id ORDER BY CASE k.status WHEN 'inbox' THEN 0 WHEN 'archived' THEN 1 ELSE 2 END,k.updated_at DESC").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| row_item(&db, r))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn list_workspace_items(workspace_id: i64, db: State<Db>) -> Result<Vec<KnowledgeItem>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT k.id,k.title,k.kind,k.source_path,k.capture_kind,k.source_uri,k.excerpt,k.body,k.status,k.project_id,p.title,k.available,k.updated_at FROM knowledge_items k LEFT JOIN projects p ON p.id=k.project_id WHERE k.project_id=?1 AND k.status!='ignored' ORDER BY CASE k.status WHEN 'inbox' THEN 0 WHEN 'archived' THEN 1 ELSE 2 END,k.updated_at DESC").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([workspace_id], |r| row_item(&db, r))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn list_workspace_roots(workspace_id: i64, db: State<Db>) -> Result<Vec<KnowledgeRoot>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT id,name,kind,path,project_id,enabled,last_scan,detail FROM knowledge_roots WHERE project_id=?1 ORDER BY name").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([workspace_id], |r| {
            Ok(KnowledgeRoot {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                path: r.get(3)?,
                project_id: r.get(4)?,
                enabled: r.get::<_, i64>(5)? == 1,
                last_scan: r.get(6)?,
                detail: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn list_workspace_events(workspace_id: i64, db: State<Db>) -> Result<Vec<Event>, String> {
    let db = lock(&db)?;
    let mut s=db.prepare("SELECT e.id,e.at,e.agent,e.skill,e.session_id,e.occurrences,e.project_path,e.summary,e.parse_status,e.timestamp_quality FROM timeline_events e JOIN projects p ON p.path=e.project_path WHERE p.id=?1 ORDER BY e.at DESC").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([workspace_id], |r| {
            Ok(Event {
                id: r.get(0)?,
                at: r.get(1)?,
                agent: r.get(2)?,
                skill: r.get(3)?,
                session_id: r.get(4)?,
                occurrences: r.get(5)?,
                project_path: r.get(6)?,
                summary: r.get(7)?,
                parse_status: r.get(8)?,
                timestamp_quality: r.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_workspace_detail(workspace_id: i64, db: State<Db>) -> Result<WorkspaceDetail, String> {
    let db = lock(&db)?;
    let workspace = db
        .query_row(
            &format!("{} WHERE p.id=?1", workspace_query()),
            [workspace_id],
            workspace_from_row,
        )
        .map_err(|_| "工作区不存在".to_string())?;
    let mut items_stmt=db.prepare("SELECT k.id,k.title,k.kind,k.source_path,k.capture_kind,k.source_uri,k.excerpt,k.body,k.status,k.project_id,p.title,k.available,k.updated_at FROM knowledge_items k LEFT JOIN projects p ON p.id=k.project_id WHERE k.project_id=?1 AND k.status!='ignored' ORDER BY CASE k.status WHEN 'inbox' THEN 0 WHEN 'archived' THEN 1 ELSE 2 END,k.updated_at DESC").map_err(|e|e.to_string())?;
    let items = items_stmt
        .query_map([workspace_id], |r| row_item(&db, r))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut roots_stmt=db.prepare("SELECT id,name,kind,path,project_id,enabled,last_scan,detail FROM knowledge_roots WHERE project_id=?1 ORDER BY name").map_err(|e|e.to_string())?;
    let roots = roots_stmt
        .query_map([workspace_id], |r| {
            Ok(KnowledgeRoot {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                path: r.get(3)?,
                project_id: r.get(4)?,
                enabled: r.get::<_, i64>(5)? == 1,
                last_scan: r.get(6)?,
                detail: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut events_stmt=db.prepare("SELECT e.id,e.at,e.agent,e.skill,e.project_path,e.summary,e.parse_status FROM timeline_events e JOIN projects p ON p.path=e.project_path WHERE p.id=?1 ORDER BY e.at DESC").map_err(|e|e.to_string())?;
    let events = events_stmt
        .query_map([workspace_id], |r| {
            Ok(Event {
                id: r.get(0)?,
                at: r.get(1)?,
                agent: r.get(2)?,
                skill: r.get(3)?,
                project_path: r.get(4)?,
                summary: r.get(5)?,
                parse_status: r.get(6)?,
                ..Default::default()
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut tasks_stmt = db
        .prepare("SELECT t.id,t.title,t.status,t.updated_at FROM tasks t JOIN task_projects tp ON tp.task_id=t.id WHERE tp.project_id=?1 ORDER BY CASE t.status WHEN 'done' THEN 2 WHEN 'blocked' THEN 1 ELSE 0 END,t.updated_at DESC LIMIT 8")
        .map_err(|e| e.to_string())?;
    let tasks = tasks_stmt
        .query_map([workspace_id], |r| {
            let id: i64 = r.get(0)?;
            Ok(WorkspaceTaskSummary {
                id,
                title: r.get(1)?,
                status: r.get(2)?,
                updated_at: r.get(3)?,
                source: task_source(&db, id).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(WorkspaceDetail {
        workspace,
        items,
        roots,
        events,
        tasks,
    })
}
#[tauri::command]
fn create_note(
    title: String,
    body: String,
    project_id: Option<i64>,
    tags: Vec<String>,
    status: Option<String>,
    capture_kind: Option<String>,
    source_uri: Option<String>,
    db: State<Db>,
) -> Result<i64, String> {
    let db = lock(&db)?;
    if title.trim().is_empty() {
        return Err("笔记标题不能为空".into());
    }
    let status = status.unwrap_or_else(|| "archived".into());
    if !matches!(status.as_str(), "inbox" | "archived") {
        return Err("invalid capture status".into());
    }
    let capture_kind = capture_kind.unwrap_or_else(|| "note".into());
    if !matches!(
        capture_kind.as_str(),
        "note" | "idea" | "web" | "github" | "conversation" | "file"
    ) {
        return Err("invalid capture kind".into());
    }
    let time = now();
    db.execute("INSERT INTO knowledge_items(title,kind,excerpt,body,status,project_id,capture_kind,source_uri,available,created_at,updated_at) VALUES(?1,'note',?2,?3,?4,?5,?6,?7,1,?8,?8)",params![title.trim(),excerpt(&body),body,status,project_id,capture_kind,source_uri,time]).map_err(|e|e.to_string())?;
    let id = db.last_insert_rowid();
    set_tags(&db, id, &tags)?;
    Ok(id)
}
#[tauri::command]
fn update_knowledge_item(
    id: i64,
    title: String,
    body: Option<String>,
    status: String,
    project_id: Option<i64>,
    tags: Vec<String>,
    skill_ids: Vec<i64>,
    db: State<Db>,
) -> Result<(), String> {
    if !matches!(status.as_str(), "inbox" | "archived" | "ignored") {
        return Err("未知处理状态".into());
    }
    let db = lock(&db)?;
    let old_kind: String = db
        .query_row("SELECT kind FROM knowledge_items WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .map_err(|_| "知识条目不存在".to_string())?;
    if old_kind == "note" {
        let body = body.unwrap_or_default();
        db.execute("UPDATE knowledge_items SET title=?1,body=?2,excerpt=?3,status=?4,project_id=?5,updated_at=?6 WHERE id=?7",params![title.trim(),body,excerpt(&body),status,project_id,now(),id]).map_err(|e|e.to_string())?;
    } else {
        db.execute(
            "UPDATE knowledge_items SET title=?1,status=?2,project_id=?3,updated_at=?4 WHERE id=?5",
            params![title.trim(), status, project_id, now(), id],
        )
        .map_err(|e| e.to_string())?;
    }
    set_tags(&db, id, &tags)?;
    db.execute("DELETE FROM knowledge_item_skills WHERE item_id=?1", [id])
        .map_err(|e| e.to_string())?;
    for skill_id in skill_ids {
        db.execute(
            "INSERT OR IGNORE INTO knowledge_item_skills(item_id,skill_id) VALUES(?1,?2)",
            params![id, skill_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn get_dashboard(db: State<Db>) -> Result<Dashboard, String> {
    let db = lock(&db)?;
    let inbox_count = db
        .query_row(
            "SELECT COUNT(*) FROM knowledge_items WHERE status='inbox' AND available=1",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let project_count = db
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let knowledge_count = db
        .query_row(
            "SELECT COUNT(*) FROM knowledge_items WHERE status='archived'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut s=db.prepare("SELECT k.id,k.title,k.kind,k.source_path,k.capture_kind,k.source_uri,k.excerpt,k.body,k.status,k.project_id,p.title,k.available,k.updated_at FROM knowledge_items k LEFT JOIN projects p ON p.id=k.project_id WHERE k.status!='ignored' ORDER BY k.updated_at DESC LIMIT 6").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| row_item(&db, r))
        .map_err(|e| e.to_string())?;
    Ok(Dashboard {
        inbox_count,
        project_count,
        knowledge_count,
        recent_items: rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
    })
}
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err("文件不存在或已失效".into());
    }
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(p)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn run() {
    let db = init_db();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Db(Mutex::new(db)))
        .invoke_handler(tauri::generate_handler![
            refresh_all,
            sync_knowledge_now,
            list_skills,
            list_skill_groups,
            get_skill_group_detail,
            get_usage_card,
            save_usage_card,
            list_skill_update_commands,
            save_skill_update_command,
            run_skill_update,
            refresh_skills,
            list_skill_copy_targets,
            copy_skill_to_agent,
            list_timeline,
            adapter_status,
            get_sync_status,
            probe_agents,
            launch_agent,
            agent_runs::start_agent_run,
            agent_runs::list_agent_runs,
            agent_runs::refresh_agent_runs,
            agent_runs::save_agent_run_result,
            agent_runs::resolve_agent_run,
            cursor::inspect_cursor_launch,
            cursor::launch_cursor_task,
            list_scan_roots,
            add_scan_root,
            remove_scan_root,
            clear_timeline,
            list_projects,
            save_project,
            delete_project,
            list_workspaces,
            save_workspace,
            mark_workspace_opened,
            delete_workspace,
            get_workspace_detail,
            list_workspace_items,
            list_workspace_roots,
            list_workspace_events,
            list_knowledge_roots,
            add_knowledge_root,
            remove_knowledge_root,
            purge_knowledge_root,
            clear_pending_sources,
            list_knowledge_items,
            create_note,
            update_knowledge_item,
            get_dashboard,
            list_tasks,
            create_task,
            update_task,
            promote_knowledge_item_to_task,
            link_knowledge_item_to_task,
            open_path
        ])
        .run(tauri::generate_context!())
        .expect("run application")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn skill(id: i64, name: &str, agent: &str, body: &str) -> Skill {
        Skill {
            id,
            name: name.into(),
            description: format!("{name} description"),
            body: body.into(),
            agent: agent.into(),
            path: format!("C:/skills/{agent}/{name}/SKILL.md"),
            custom: false,
            available: true,
            has_card: false,
        }
    }
    #[test]
    fn reads_skill_frontmatter() {
        let (name, description) = parse_skill(
            "---\nname: my-skill\ndescription: concise\n---\n# body",
            Path::new("C:/skills/fallback/SKILL.md"),
        );
        assert_eq!(name, "my-skill");
        assert_eq!(description, "concise");
    }
    #[test]
    fn parses_skill_relations() {
        let metadata = parse_skill_metadata(
            "---\nname: child\nparent: parent\nchildren: [grandchild, extra]\n---",
            Path::new("C:/skills/child/SKILL.md"),
        );
        assert_eq!(metadata.parent.as_deref(), Some("parent"));
        assert_eq!(metadata.children, ["grandchild", "extra"]);
    }
    #[test]
    fn groups_same_name_variants() {
        let library = build_skill_library(&[
            skill(1, "grill-me", "Claude", "---\nname: grill-me\n---"),
            skill(2, "grill-me", "Cursor", "---\nname: grill-me\n---"),
            skill(3, "grill-me", "Agents", "---\nname: grill-me\n---"),
        ]);
        assert_eq!(library.groups.len(), 1);
        assert_eq!(library.groups[0].variants.len(), 3);
        assert!(library.groups[0].root);
    }
    #[test]
    fn hides_resolved_children_from_roots() {
        let library = build_skill_library(&[
            skill(
                1,
                "parent",
                "Codex",
                "---\nname: parent\nchildren: child\n---",
            ),
            skill(2, "child", "Codex", "---\nname: child\nparent: parent\n---"),
        ]);
        let parent = library
            .groups
            .iter()
            .find(|item| item.key == "parent")
            .unwrap();
        let child = library
            .groups
            .iter()
            .find(|item| item.key == "child")
            .unwrap();
        assert_eq!(parent.children, ["child"]);
        assert_eq!(child.parents, ["parent"]);
        assert!(parent.root);
        assert!(!child.root);
    }
    #[test]
    fn keeps_unresolved_relations_visible() {
        let library = build_skill_library(&[skill(
            1,
            "child",
            "Codex",
            "---\nname: child\nparent: missing\n---",
        )]);
        assert!(library.groups[0].root);
        assert_eq!(library.groups[0].unresolved_relations, ["missing"]);
    }
    #[test]
    fn marks_cycles_without_recursing() {
        let library = build_skill_library(&[
            skill(1, "a", "Codex", "---\nname: a\nparent: b\n---"),
            skill(2, "b", "Codex", "---\nname: b\nparent: a\n---"),
        ]);
        assert!(library.groups.iter().all(|item| item.cycle && item.root));
    }
    #[test]
    fn hashes_are_stable() {
        assert_eq!(fnv_hash("knowledge"), fnv_hash("knowledge"));
        assert_ne!(fnv_hash("knowledge"), fnv_hash("other"));
    }
    #[test]
    fn indexes_only_safe_text_formats() {
        assert!(is_supported(Path::new("note.md")));
        assert!(!is_supported(Path::new("image.png")));
    }
}

#[cfg(test)]
mod function_group_tests {
    use super::*;
    #[test]
    fn classifies_functions_and_keeps_github_source() {
        let library=build_skill_library(&[Skill{id:1,name:"planning-with-files-zh".into(),description:"Persistent planning workflow for tasks and progress".into(),body:"---\nname: planning-with-files-zh\nmetadata:\n  source-repo: \"https://github.com/othmanadi/planning-with-files.git\"\n---\n# Planning\n\nUse task_plan.md and progress.md.".into(),agent:"Claude".into(),path:"C:/skills/planning-with-files-zh/SKILL.md".into(),custom:false,available:true,has_card:false}]);
        let group = &library.groups[0];
        assert!(group.function_keys.contains(&"planning".to_string()));
        assert!(library.function_groups.iter().any(
            |item| item.key == "planning" && item.skill_keys == vec!["planning-with-files-zh"]
        ));
        assert_eq!(
            group.variants[0].github_url.as_deref(),
            Some("https://github.com/othmanadi/planning-with-files.git")
        );
        assert!(library
            .function_relations
            .iter()
            .any(|item| item.source == "planning" && item.target == "planning-with-files-zh"));
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;
    fn temp_path(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "agent-skill-workbench-{label}-{}",
            fnv_hash(&format!("{label}:{}", now()))
        ))
    }
    #[test]
    fn exposes_all_built_in_copy_targets() {
        let targets = built_in_skill_roots();
        assert_eq!(targets.len(), 5);
        assert!(targets.iter().any(|(agent, _)| agent == "Codex"));
        assert!(targets.iter().any(|(agent, _)| agent == "Cursor"));
        assert!(targets.iter().any(|(agent, _)| agent == "Agents"));
    }
    #[test]
    fn copies_complete_skill_directory() {
        let source = temp_path("source");
        let target = temp_path("target");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n# Demo").unwrap();
        fs::write(source.join("scripts/run.ps1"), "Write-Output demo").unwrap();
        let count = copy_directory(&source, &target).unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "---\nname: demo\n---\n# Demo"
        );
        assert_eq!(
            fs::read_to_string(target.join("scripts/run.ps1")).unwrap(),
            "Write-Output demo"
        );
        let _ = remove_path(&source);
        let _ = remove_path(&target);
    }
    #[test]
    fn backup_path_changes_when_candidate_exists() {
        let root = temp_path("backup");
        fs::create_dir_all(&root).unwrap();
        let first = backup_path(&root, "demo");
        fs::create_dir_all(&first).unwrap();
        let second = backup_path(&root, "demo");
        assert_ne!(first, second);
        let _ = remove_path(&root);
    }
}

#[cfg(test)]
mod task_source_tests {
    use super::*;

    fn task_db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE projects(id INTEGER PRIMARY KEY,title TEXT NOT NULL,description TEXT NOT NULL DEFAULT '',path TEXT,color TEXT,updated_at TEXT NOT NULL,last_opened_at TEXT);
             CREATE TABLE knowledge_items(id INTEGER PRIMARY KEY,title TEXT NOT NULL,kind TEXT NOT NULL,capture_kind TEXT NOT NULL,source_uri TEXT,body TEXT NOT NULL,status TEXT NOT NULL,project_id INTEGER,updated_at TEXT NOT NULL);
             CREATE TABLE knowledge_roots(id INTEGER PRIMARY KEY,project_id INTEGER);
             CREATE TABLE tasks(id INTEGER PRIMARY KEY,title TEXT NOT NULL,objective TEXT NOT NULL,steps TEXT NOT NULL,status TEXT NOT NULL,priority INTEGER NOT NULL,recommended_agent TEXT,recommended_skill TEXT,updated_at TEXT NOT NULL,created_at TEXT NOT NULL);
             CREATE TABLE task_projects(task_id INTEGER NOT NULL,project_id INTEGER NOT NULL,PRIMARY KEY(task_id,project_id));
             CREATE TABLE task_sources(id INTEGER PRIMARY KEY,task_id INTEGER NOT NULL,kind TEXT NOT NULL,title TEXT NOT NULL,uri TEXT NOT NULL,content TEXT NOT NULL,created_at TEXT NOT NULL);",
        )
        .unwrap();
        migrate_task_sources_schema(&db).unwrap();
        db.execute(
            "INSERT INTO projects(id,title,updated_at) VALUES(1,'Workspace',?1)",
            [now()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO knowledge_items(id,title,kind,capture_kind,source_uri,body,status,project_id,updated_at) VALUES(1,'Inbox item','note','note',NULL,'body','inbox',1,?1)",
            [now()],
        )
        .unwrap();
        db
    }

    #[test]
    fn migrates_old_task_source_schema_and_keeps_snapshot_readable() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE task_sources(id INTEGER PRIMARY KEY,task_id INTEGER NOT NULL,kind TEXT NOT NULL,title TEXT NOT NULL,uri TEXT NOT NULL,content TEXT NOT NULL,created_at TEXT NOT NULL);
             CREATE TABLE tasks(id INTEGER PRIMARY KEY,title TEXT NOT NULL,objective TEXT NOT NULL,steps TEXT NOT NULL,status TEXT NOT NULL,priority INTEGER NOT NULL,recommended_agent TEXT,recommended_skill TEXT,updated_at TEXT NOT NULL,created_at TEXT NOT NULL);
             CREATE TABLE task_projects(task_id INTEGER NOT NULL,project_id INTEGER NOT NULL,PRIMARY KEY(task_id,project_id));
             CREATE TABLE projects(id INTEGER PRIMARY KEY,title TEXT NOT NULL,updated_at TEXT NOT NULL);
             CREATE TABLE knowledge_items(id INTEGER PRIMARY KEY,status TEXT);",
        )
        .unwrap();
        migrate_task_sources_schema(&db).unwrap();
        let column: String = db
            .query_row("PRAGMA table_info(task_sources)", [], |row| row.get(1))
            .unwrap();
        assert_eq!(column, "id");
        let has_column: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('task_sources') WHERE name='knowledge_item_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_column, 1);
        db.execute(
            "INSERT INTO projects(id,title,updated_at) VALUES(1,'Legacy',?1)",
            [now()],
        )
        .unwrap();
        db.execute("INSERT INTO tasks(id,title,objective,steps,status,priority,updated_at,created_at) VALUES(1,'Legacy task','','','ready',0,?1,?1)", [now()])
            .unwrap();
        db.execute(
            "INSERT INTO task_projects(task_id,project_id) VALUES(1,1)",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO task_sources(task_id,kind,title,uri,content,created_at) VALUES(1,'file','Legacy source','file:///legacy','snapshot',?1)", [now()])
            .unwrap();
        let task = task_by_id(&db, 1).unwrap();
        assert_eq!(task.source.as_ref().unwrap().title, "Legacy source");
        assert_eq!(task.source.as_ref().unwrap().knowledge_item_id, None);
    }

    #[test]
    fn promotes_source_idempotently_and_archives_it() {
        let db = task_db();
        let first = promote_knowledge_item_to_task_db(1, &db).unwrap();
        assert_eq!(first.title, "Inbox item");
        assert_eq!(first.source.as_ref().unwrap().knowledge_item_id, Some(1));
        let status: String = db
            .query_row("SELECT status FROM knowledge_items WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(status, "archived");
        let second = promote_knowledge_item_to_task_db(1, &db).unwrap();
        assert_eq!(first.id, second.id);
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn promotion_rolls_back_task_and_source_state_on_failure() {
        let db = task_db();
        db.execute_batch(
            "CREATE TRIGGER fail_task_source BEFORE INSERT ON task_sources BEGIN SELECT RAISE(ABORT,'test failure'); END;",
        )
        .unwrap();
        assert!(promote_knowledge_item_to_task_db(1, &db).is_err());
        let task_count: i64 = db
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        let status: String = db
            .query_row("SELECT status FROM knowledge_items WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(task_count, 0);
        assert_eq!(status, "inbox");
    }

    #[test]
    fn links_source_to_existing_task_and_rejects_second_source() {
        let db = task_db();
        db.execute(
            "INSERT INTO tasks(id,title,objective,steps,status,priority,updated_at,created_at) VALUES(7,'Existing','','','ready',0,?1,?1)",
            [now()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO task_projects(task_id,project_id) VALUES(7,1)",
            [],
        )
        .unwrap();
        let task = link_knowledge_item_to_task_db(1, 7, &db).unwrap();
        assert_eq!(task.id, 7);
        assert_eq!(link_knowledge_item_to_task_db(1, 7, &db).unwrap().id, 7);
        db.execute(
            "INSERT INTO knowledge_items(id,title,kind,capture_kind,body,status,project_id,updated_at) VALUES(2,'Second','note','note','','inbox',1,?1)",
            [now()],
        )
        .unwrap();
        assert!(link_knowledge_item_to_task_db(2, 7, &db).is_err());
    }

    #[test]
    fn deleting_workspace_detaches_tasks_without_deleting_content() {
        let db = task_db();
        db.execute(
            "INSERT INTO task_projects(task_id,project_id) VALUES(9,1)",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO knowledge_roots(id,project_id) VALUES(1,1)", [])
            .unwrap();
        delete_workspace_db(1, &db).unwrap();
        let task_links: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM task_projects WHERE project_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let item_project: Option<i64> = db
            .query_row(
                "SELECT project_id FROM knowledge_items WHERE id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_links, 0);
        assert_eq!(item_project, None);
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM projects", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
