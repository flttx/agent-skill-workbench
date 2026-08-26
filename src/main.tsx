import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import * as Select from "@radix-ui/react-select";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  animatePortalState,
  createWorkbenchMotion,
  type WorkbenchMotion,
} from "./motion";
import {
  Archive,
  BookOpen,
  Check,
  ChevronDown,
  CircleDashed,
  ExternalLink,
  FileText,
  FolderOpen,
  History,
  Inbox,
  Library,
  PanelTop,
  Palette,
  PenLine,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import {
  applyTheme,
  getStoredTheme,
  persistTheme,
  THEMES,
  type ThemeDefinition,
  type ThemeId,
} from "./theme";
import "./styles.css";
import "./design-system.css";
import "./agent-probe.css";

type Tab = "today" | "workspace" | "inbox" | "skills" | "review" | "settings";
type WorkspaceView =
  "overview" | "knowledge" | "sources" | "skills" | "activity";
type Workspace = {
  id: number;
  title: string;
  description: string;
  path: string | null;
  color: string;
  updated_at: string;
  last_opened_at: string | null;
  inbox_count: number;
  knowledge_count: number;
  source_count: number;
};
type KnowledgeRoot = {
  id: number;
  name: string;
  kind: "project" | "agent_artifact";
  path: string;
  project_id: number | null;
  enabled: boolean;
  last_scan: string | null;
  detail: string;
};
type SkillVariant = {
  id: number;
  name: string;
  description: string;
  body: string;
  content: string;
  agent: string;
  path: string;
  custom: boolean;
  available: boolean;
  has_card: boolean;
  parent_key: string | null;
  children_keys: string[];
  github_url: string | null;
};
type SkillGroup = {
  key: string;
  name: string;
  description: string;
  variants: SkillVariant[];
  parents: string[];
  children: string[];
  unresolved_relations: string[];
  function_keys: string[];
  cycle: boolean;
  root: boolean;
};
type SkillRelation = {
  source: string;
  target: string;
  relation: string;
  unresolved: boolean;
};
type SkillFunctionGroup = {
  key: string;
  name: string;
  description: string;
  skill_keys: string[];
  github_references: string[];
};
type SkillFunctionRelation = {
  source: string;
  target: string;
  relation: string;
};
type SkillLibrary = {
  groups: SkillGroup[];
  relations: SkillRelation[];
  function_groups: SkillFunctionGroup[];
  function_relations: SkillFunctionRelation[];
};
type Card = {
  skill_id: number;
  scenarios: string;
  triggers: string;
  steps: string;
  notes: string;
  pitfalls: string;
  links: string;
  tags: string;
};
type Event = {
  id: number;
  at: string;
  agent: string;
  skill: string;
  session_id: string;
  occurrences: number;
  project_path: string | null;
  summary: string;
  parse_status: string;
  timestamp_quality: string;
};
type Status = {
  agent: string;
  state: string;
  detail: string;
  last_sync: string | null;
};
type SyncStatus = {
  stage: string;
  state: string;
  detail: string;
  started_at: string | null;
  finished_at: string | null;
};
type AgentProbe = {
  agent: string;
  state: "available" | "missing" | string;
  command: string;
  executable: string | null;
  launch_mode: string;
  detail: string;
};
type CursorLaunchPlan = {
  workspace_path: string;
  window_mode: "reuse" | "new" | string;
  matched_window: number | null;
  cursor_running: boolean;
};
type CursorLaunchResult = {
  run_id: string;
  status: "filled" | "fallback" | "failed" | string;
  transport: "cursor_ide" | "cursor_agent_terminal" | string;
  window_mode: "reuse" | "new" | string;
  window_id: number | null;
  error: string | null;
};
type AgentRun = {
  id: string;
  task_id: number | null;
  agent: string;
  workspace_path: string;
  window_mode: string;
  transport: string;
  window_handle: number | null;
  prompt_snapshot: string;
  status: string;
  error_message: string | null;
  session_id: string | null;
  match_state: "matched" | "pending" | "ignored" | string;
  result_state: "none" | "draft" | "saved" | string;
  result_summary: string;
  changed_files: string;
  verification: string;
  unresolved_issues: string;
  raw_excerpt: string;
  result_source_path: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
  change_source: "git_baseline" | "snapshot" | "legacy_history" | string;
  baseline_at: string | null;
  intermediate_files: string;
  change_error: string | null;
};
type ChangedFileKind = "added" | "modified" | "deleted" | "unknown";
type ChangedFileLine = {
  kind: ChangedFileKind;
  path: string;
};

function parseChangedFileLine(line: string): ChangedFileLine | null {
  const trimmed = line.trim();
  if (!trimmed) return null;
  const marker = trimmed[0];
  if (marker === "+" || marker === "~" || marker === "-") {
    return {
      kind: marker === "+" ? "added" : marker === "-" ? "deleted" : "modified",
      path: trimmed.slice(1).trim(),
    };
  }
  return { kind: "unknown", path: trimmed };
}

function ChangedFilesPreview({ value }: { value: string }) {
  const files = value.split(/\r?\n/).map(parseChangedFileLine).filter((file): file is ChangedFileLine => Boolean(file));
  if (!files.length) return null;
  return (
    <div className="changed-files-preview" role="list" aria-label="文件变更预览">
      {files.map((file, index) => (
        <div className={`changed-file-row changed-file-${file.kind}`} key={`${file.path}-${index}`} role="listitem">
          <span className="changed-file-marker" aria-hidden="true">
            {file.kind === "deleted" ? "−" : file.kind === "unknown" ? "·" : file.kind === "added" ? "+" : "~"}
          </span>
          <code>{file.path}</code>
        </div>
      ))}
      <div className="changed-files-legend" aria-hidden="true">
        <span className="changed-file-added">+ 新增</span>
        <span className="changed-file-modified">~ 修改</span>
        <span className="changed-file-deleted">− 删除</span>
      </div>
    </div>
  );
}

function changeSourceLabel(source: string): string {
  if (source === "git_baseline") return "基线差异";
  if (source === "snapshot") return "文件快照";
  return "历史推断";
}
function captureKindLabel(kind: string): string {
  return (
    {
      idea: "想法",
      web: "网页",
      github: "GitHub",
      conversation: "AI 对话",
      file: "本地文件",
      agent_artifact: "Agent 产物",
      note: "笔记",
    }[kind] ?? "来源"
  );
}
type AgentRunStartResult = {
  run_id: string;
  status: string;
  transport: string;
  error: string | null;
};
type AgentRunResultDraft = {
  runId: string;
  result_summary: string;
  changed_files: string;
  verification: string;
  unresolved_issues: string;
};
type KnowledgeItem = {
  id: number;
  title: string;
  kind: "note" | "file" | "agent_artifact";
  source_path: string | null;
  capture_kind: string;
  source_uri: string | null;
  excerpt: string;
  body: string;
  status: "inbox" | "archived" | "ignored";
  project_id: number | null;
  project_title: string | null;
  available: boolean;
  updated_at: string;
  tags: string[];
  skill_ids: number[];
};
type TaskProject = { id: number; title: string };
type TaskSource = {
  id: number;
  kind: string;
  title: string;
  uri: string;
  content: string;
  knowledge_item_id?: number | null;
  knowledge_item_status?: KnowledgeItem["status"] | null;
};
type Task = {
  id: number;
  title: string;
  objective: string;
  steps: string;
  status: "draft" | "ready" | "in_progress" | "blocked" | "done";
  priority: number;
  recommended_agent: string | null;
  recommended_skill: string | null;
  projects: TaskProject[];
  source: TaskSource | null;
  updated_at: string;
  created_at: string;
};
type TaskDraft = {
  title: string;
  objective: string;
  steps: string;
  status: Task["status"];
  priority: number;
  project_ids: number[];
  source_kind?: string;
  source_title?: string;
  source_uri?: string;
  source_content?: string;
  source_knowledge_item_id?: number | null;
  recommended_agent?: string;
  recommended_skill?: string;
};
type CaptureDraft = {
  kind: "idea" | "web" | "github" | "conversation" | "file";
  title: string;
  body: string;
  project_id: number | null;
};
type WorkspaceDetail = {
  workspace: Workspace;
  items: KnowledgeItem[];
  roots: KnowledgeRoot[];
  events: Event[];
  tasks: WorkspaceTaskSummary[];
};
type WorkspaceTaskSummary = {
  id: number;
  title: string;
  status: Task["status"];
  updated_at: string;
  source: TaskSource | null;
};
type Confirmation = {
  title: string;
  description: string;
  confirmLabel: string;
  danger?: boolean;
  action: () => void | Promise<void>;
};
type SkillUpdateCommand = { agent: string; command: string; enabled: boolean };
interface SkillOptimizationRequest {
  agent: "Cursor" | "Codex";
  skillName: string;
  skillPath: string;
  workspacePath: string;
  goal: string;
  prompt: string;
}
type SkillUpdateRun = {
  run_id: string;
  skill_id: number;
  command: string;
  cwd: string;
  status: string;
};
type SkillUpdateEvent = {
  run_id: string;
  status: "started" | "output" | "success" | "failed";
  stream: "stdout" | "stderr" | "system";
  line: string;
  exit_code: number | null;
};
type SkillCopyTarget = { agent: string; path: string; available: boolean };
type CopySkillResult = {
  source_agent: string;
  source_path: string;
  target_agent: string;
  target_path: string;
  backup_path: string | null;
  copied_files: number;
  rescanned: boolean;
};

const call = <T,>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args);
const emptyCard = (skill_id: number): Card => ({
  skill_id,
  scenarios: "",
  triggers: "",
  steps: "",
  notes: "",
  pitfalls: "",
  links: "",
  tags: "",
});

function EnhancedSkillsPane({
  library,
  workspaceItems,
  updateCommands,
  copyTargets,
  agentProbes,
  openSettings,
  launchOptimization,
  reloadSkills,
  confirm,
}: {
  library: SkillLibrary;
  workspaceItems?: KnowledgeItem[];
  updateCommands: SkillUpdateCommand[];
  copyTargets: SkillCopyTarget[];
  agentProbes: AgentProbe[];
  openSettings: () => void;
  launchOptimization: (request: SkillOptimizationRequest) => void;
  reloadSkills: () => Promise<void>;
  confirm: (config: Confirmation) => void;
}) {
  const [query, setQuery] = useState(""),
    [selectedKey, setSelectedKey] = useState(""),
    [selectedVariantId, setSelectedVariantId] = useState<number>(),
    [selectedFunction, setSelectedFunction] = useState(""),
    [copyPanelOpen, setCopyPanelOpen] = useState(false),
    [copyTargetAgent, setCopyTargetAgent] = useState(""),
    [copying, setCopying] = useState(false),
    [copyResult, setCopyResult] = useState<CopySkillResult>(),
    [copyError, setCopyError] = useState(""),
    [card, setCard] = useState<Card>(),
    [cardLoaded, setCardLoaded] = useState(false),
    [dirty, setDirty] = useState(false),
    [preview, setPreview] = useState(true),
    [updateRun, setUpdateRun] = useState<SkillUpdateRun>(),
    [updateEvents, setUpdateEvents] = useState<SkillUpdateEvent[]>([]),
    [updateError, setUpdateError] = useState(""),
    [optimizationVariant, setOptimizationVariant] = useState<SkillVariant>();
  const timer = useRef<number | undefined>(undefined),
    runIdRef = useRef("");
  const linked = useMemo(
    () => new Set((workspaceItems ?? []).flatMap((item) => item.skill_ids)),
    [workspaceItems],
  );
  const linkedGroups = useMemo(
    () =>
      new Set(
        library.groups
          .filter((group) =>
            group.variants.some((variant) => linked.has(variant.id)),
          )
          .map((group) => group.key),
      ),
    [library.groups, linked],
  );
  const list = library.groups.filter(
    (group) =>
      group.root &&
      (!selectedFunction || group.function_keys.includes(selectedFunction)) &&
      `${group.name} ${group.description} ${group.variants.map((variant) => `${variant.agent} ${variant.path} ${variant.description}`).join(" ")}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const selected = library.groups.find((group) => group.key === selectedKey),
    selectedVariant =
      selected?.variants.find((variant) => variant.id === selectedVariantId) ??
      selected?.variants[0];
  const selectedCommand =
    selectedVariant &&
    updateCommands.find((command) => command.agent === selectedVariant.agent);
  const chooseVariant = async (variant: SkillVariant) => {
    setSelectedVariantId(variant.id);
    setCard(undefined);
    setCardLoaded(false);
    setDirty(false);
    setUpdateEvents([]);
    setUpdateRun(undefined);
    setCopyPanelOpen(false);
    setCopyResult(undefined);
    setCopyError("");
    try {
      setCard(
        (await call<Card | null>("get_usage_card", { skillId: variant.id })) ??
          undefined,
      );
    } finally {
      setCardLoaded(true);
    }
  };
  const chooseGroup = (group: SkillGroup) => {
    setSelectedKey(group.key);
    setPreview(true);
    const variant =
      group.variants.find((item) => item.available) ?? group.variants[0];
    if (variant) void chooseVariant(variant);
  };
  useEffect(() => {
    if (!selectedKey && list.length) chooseGroup(list[0]);
  }, [list.length]);
  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void listen<SkillUpdateEvent>("skill-update-output", (event) => {
      const payload = event.payload;
      if (payload.run_id !== runIdRef.current) {
        if (runIdRef.current) return;
        runIdRef.current = payload.run_id;
      }
      setUpdateEvents((events) => [...events, payload].slice(-300));
      if (payload.status === "success" || payload.status === "failed") {
        setUpdateRun((run) => (run ? { ...run, status: payload.status } : run));
        if (payload.status === "success") void reloadSkills();
      }
    }).then((stop) => {
      if (active) unsubscribe = stop;
      else stop();
    });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [reloadSkills]);
  useEffect(() => {
    if (!card || !dirty) return;
    clearTimeout(timer.current);
    timer.current = window.setTimeout(async () => {
      try {
        await call("save_usage_card", { card });
        setDirty(false);
      } catch {
        setUpdateError("使用卡片保存失败");
      }
    }, 800);
    return () => clearTimeout(timer.current);
  }, [card, dirty]);
  const startUpdate = (variant: SkillVariant) => {
    if (!selectedCommand?.enabled || !selectedCommand.command.trim()) {
      setUpdateError(`${variant.agent} 尚未配置更新命令`);
      openSettings();
      return;
    }
    const command = expandUpdateTemplate(selectedCommand.command, variant);
    confirm({
      title: `更新 ${variant.name}？`,
      description: `命令：${command}\n工作目录：${variant.path.replace(/[\\/][^\\/]*$/, "")}`,
      confirmLabel: "执行更新",
      action: async () => {
        setUpdateError("");
        setUpdateEvents([]);
        setUpdateRun(undefined);
        runIdRef.current = "";
        try {
          const run = await call<SkillUpdateRun>("run_skill_update", {
            skillId: variant.id,
          });
          runIdRef.current = run.run_id;
          setUpdateRun(run);
        } catch (error) {
          setUpdateError(String(error));
        }
      },
    });
  };
  const copyTargetsForVariant = selectedVariant
    ? copyTargets.filter((target) => target.agent !== selectedVariant.agent)
    : [];
  const openCopyPanel = () => {
    setCopyError("");
    setCopyResult(undefined);
    setCopyPanelOpen(true);
    if (!copyTargetAgent && copyTargetsForVariant[0]) {
      setCopyTargetAgent(copyTargetsForVariant[0].agent);
    }
  };
  const startCopy = (variant: SkillVariant) => {
    const target = copyTargetsForVariant.find(
      (item) => item.agent === copyTargetAgent,
    );
    if (!target) {
      setCopyError("请选择目标 Agent");
      return;
    }
    confirm({
      title: `复制 ${variant.name} 到 ${target.agent}`,
      description: `来源：${variant.path}\n目标：${target.path}\n将复制完整 Skill 目录；如果目标已存在，将先备份原目录。`,
      confirmLabel: "复制并备份",
      action: async () => {
        setCopying(true);
        setCopyError("");
        try {
          const result = await call<CopySkillResult>("copy_skill_to_agent", {
            skillId: variant.id,
            targetAgent: target.agent,
          });
          setCopyResult(result);
          await reloadSkills();
        } catch (error) {
          setCopyError(String(error));
        } finally {
          setCopying(false);
        }
      },
    });
  };
  const linkedCount = workspaceItems ? linkedGroups.size : undefined;

  return (
    <div className="view skills-view">
      <div className="workspace-toolbar">
        <label className="input-with-icon">
          <Search aria-hidden="true" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            aria-label="搜索本机 Skill"
            placeholder="搜索 Skill、Agent 或路径"
          />
        </label>
        {linkedCount !== undefined && (
          <span className="toolbar-count">{linkedCount} 个已关联</span>
        )}
      </div>
      <div className="function-filter-strip" aria-label="按功能筛选 Skill">
        <button className={!selectedFunction ? "active" : ""} onClick={() => setSelectedFunction("")}>全部 <small>{library.groups.filter((group) => group.root).length}</small></button>
        {library.function_groups.map((group) => <button className={selectedFunction === group.key ? "active" : ""} key={group.key} onClick={() => setSelectedFunction(group.key)}>{group.name} <small>{group.skill_keys.length}</small></button>)}
      </div>
      <div className="skills-library-grid">
        <section className="skills-grid skill-group-list">
          {list.map((group) => (
            <SkillGroupCard
              key={group.key}
              group={group}
              selected={selectedKey === group.key}
              linked={linkedGroups.has(group.key)}
              onClick={() => chooseGroup(group)}
            />
          ))}
          {!list.length && <Empty text="没有匹配的 Skill。" />}
        </section>
        <section className="detail-panel skill-group-detail">
          {selected && selectedVariant ? (
            <>
              <div className="detail-title">
                <div>
                  <span className="eyebrow">
                    SKILL GROUP · {selected.variants.length} 个版本
                  </span>
                  <h2>{selected.name}</h2>
                </div>
                <div className="detail-title-actions">
                  <button
                    className="icon-button detail-icon-action"
                    title="打开来源"
                    aria-label={`打开 ${selectedVariant.agent} 来源`}
                    onClick={() =>
                      void call("open_path", { path: selectedVariant.path })
                    }
                  >
                    <ExternalLink aria-hidden="true" />
                  </button>
                </div>
              </div>
              <div className="detail-action-row">
                  <button className="button button-primary" onClick={() => setOptimizationVariant(selectedVariant)} disabled={!selectedVariant.available}>
                    <Sparkles aria-hidden="true" />优化本机 Skill
                  </button>
                  <button
                    className="button button-secondary"
                    disabled={
                      !selectedCommand?.enabled ||
                      !selectedCommand.command.trim()
                    }
                    onClick={() => startUpdate(selectedVariant)}
                  >
                    <RefreshCw aria-hidden="true" />
                    更新 Skill
                  </button>
                  <button className="button button-ghost" onClick={() => void call("open_path", { path: selectedVariant.path })}>
                    <ExternalLink aria-hidden="true" />打开目录
                  </button>
                  {(!selectedCommand?.enabled ||
                    !selectedCommand.command.trim()) && (
                    <button className="button button-ghost" onClick={openSettings}>
                      配置更新命令
                    </button>
                  )}
              </div>
              {copyPanelOpen && (
                <section className="skill-copy-panel">
                  <div className="skill-copy-panel-heading">
                    <div>
                      <span className="card-kicker">COPY SKILL</span>
                      <b>复制完整 Skill 目录</b>
                    </div>
                    <button className="text-button" onClick={() => setCopyPanelOpen(false)}>关闭</button>
                  </div>
                  <p>包含 SKILL.md、scripts、references 和 assets。目标目录已有同名 Skill 时会先生成备份。</p>
                  <div className="skill-copy-controls">
                    <SelectControl
                      value={copyTargetAgent}
                      values={copyTargetsForVariant.map((target) => target.agent)}
                      labels={Object.fromEntries(copyTargetsForVariant.map((target) => [target.agent, `${target.agent} · ${target.path}`]))}
                      onChange={setCopyTargetAgent}
                    />
                    <button className="button button-primary" disabled={!copyTargetAgent || copying} onClick={() => startCopy(selectedVariant)}>
                      {copying ? "复制中…" : "确认复制"}
                    </button>
                  </div>
                  {copyError && <p className="form-error">{copyError}</p>}
                  {copyResult && <div className="skill-copy-result"><span className="live">已复制 {copyResult.copied_files} 个文件</span><small>目标：{copyResult.target_path}</small>{copyResult.backup_path && <small>备份：{copyResult.backup_path}</small>}</div>}
                </section>
              )}
              <SkillMetadata group={selected} variant={selectedVariant} />
              <details className="skill-secondary-details">
                <summary>查看完整用法、版本和关系</summary>
                <SkillGraph
                  group={selected}
                  library={library}
                  selectGroup={chooseGroup}
                />
                <section className="skill-source-section">
                  <div className="skill-section-heading">
                    <div>
                      <span className="card-kicker">SKILL.MD CONTENT</span>
                      <b>Skill 正文</b>
                    </div>
                    <div className="source-view-toggle">
                      <button className={preview ? "active" : ""} onClick={() => setPreview(true)}>渲染视图</button>
                      <button className={!preview ? "active" : ""} onClick={() => setPreview(false)}>原文视图</button>
                    </div>
                  </div>
                  {preview ? <MarkdownPreview content={selectedVariant.content || selectedVariant.body} /> : <pre className="skill-source-raw">{selectedVariant.content || selectedVariant.body}</pre>}
                </section>
                <VariantList group={selected} selectedId={selectedVariant.id} choose={chooseVariant} />
                {cardLoaded && (
                  <section className="skill-usage-section">
                    <div className="skill-section-heading">
                      <div>
                        <span className="card-kicker">MY OPERATING NOTES</span>
                        <b>我的使用笔记</b>
                      </div>
                      {card && <small>自动保存</small>}
                    </div>
                    {card ? <UsageCard card={card} change={(next) => { setCard(next); setDirty(true); }} /> : <div className="skill-card-empty"><p>这个版本还没有使用卡片。</p><button className="button button-secondary" onClick={() => { setCard(emptyCard(selectedVariant.id)); setDirty(true); }}>创建使用卡片</button></div>}
                  </section>
                )}
                <div className="skill-secondary-actions">
                  <button className="button button-ghost" onClick={openCopyPanel} disabled={!copyTargetsForVariant.length || copying}>复制到其他 Agent</button>
                </div>
              </details>
              {(updateRun || updateEvents.length > 0 || updateError) && (
                <UpdateConsole
                  run={updateRun}
                  events={updateEvents}
                  error={updateError}
                />
              )}
            </>
          ) : (
            <Empty text="从左侧选择一个主 Skill，查看详情。" />
          )}
        </section>
      </div>
      {optimizationVariant && (
        <SkillOptimizationDialog
          variant={optimizationVariant}
          agentProbes={agentProbes}
          close={() => setOptimizationVariant(undefined)}
          submit={(request) => {
            setOptimizationVariant(undefined);
            launchOptimization(request);
          }}
        />
      )}
    </div>
  );
}

function expandUpdateTemplate(template: string, variant: SkillVariant) {
  return template
    .replaceAll("{name}", variant.name)
    .replaceAll("{path}", variant.path)
    .replaceAll("{agent}", variant.agent)
    .replaceAll("{dir}", variant.path.replace(/[\\/][^\\/]*$/, ""));
}
function SkillMetadata({
  group,
  variant,
}: {
  group: SkillGroup;
  variant: SkillVariant;
}) {
  return (
    <section className="skill-metadata">
      <div>
        <span className="eyebrow">SOURCE METADATA</span>
        <h3>{variant.name}</h3>
      </div>
      <dl>
        <div>
          <dt>Agent</dt>
          <dd>{variant.agent}</dd>
        </div>
        <div>
          <dt>状态</dt>
          <dd>
            <span className={variant.available ? "live" : "offline"}>
              {variant.available ? "可用" : "来源失效"}
            </span>
          </dd>
        </div>
        <div>
          <dt>父 Skill</dt>
          <dd>{group.parents.length ? group.parents.join("、") : "无"}</dd>
        </div>
        <div>
          <dt>子 Skill</dt>
          <dd>{group.children.length ? group.children.join("、") : "无"}</dd>
        </div>
      </dl>
      <div className="skill-metadata-description markdown-description">
        <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml>
          {variant.description || "暂无描述"}
        </ReactMarkdown>
      </div>
      <p className="variant-path">{variant.path}</p>
      {variant.github_url && (
        <a className="skill-github-source" href={variant.github_url} target="_blank" rel="noreferrer noopener">
          查看 GitHub 使用说明
        </a>
      )}
    </section>
  );
}
function MarkdownPreview({ content }: { content: string }) {
  return (
    <article className="markdown-preview">
      <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml>
        {content || "暂无正文"}
      </ReactMarkdown>
    </article>
  );
}
function UpdateConsole({
  run,
  events,
  error,
}: {
  run?: SkillUpdateRun;
  events: SkillUpdateEvent[];
  error: string;
}) {
  return (
    <section className="skill-update-console">
      <div className="skill-section-heading">
        <div>
          <span className="card-kicker">UPDATE OUTPUT</span>
          <b>更新执行记录</b>
        </div>
        {run && (
          <small
            className={
              run.status === "success"
                ? "live"
                : run.status === "failed"
                  ? "offline"
                  : ""
            }
          >
            {run.status === "running"
              ? "执行中"
              : run.status === "success"
                ? "已完成"
                : "失败"}
          </small>
        )}
      </div>
      {run && (
        <p className="update-command-line">
          {run.command}
          <br />
          工作目录：{run.cwd}
        </p>
      )}
      {error && <p className="form-error">{error}</p>}
      <pre className="update-output">
        {events
          .map((event, index) => `[${event.stream}] ${event.line}`)
          .join("\n") || "等待命令输出…"}
      </pre>
    </section>
  );
}

function SkillOptimizationDialog({
  variant,
  agentProbes,
  close,
  submit,
}: {
  variant: SkillVariant;
  agentProbes: AgentProbe[];
  close: () => void;
  submit: (request: SkillOptimizationRequest) => void;
}) {
  const workspacePath = variant.path.replace(/[\\/][^\\/]*$/, "");
  const [agent, setAgent] = useState<SkillOptimizationRequest["agent"]>(() =>
    agentProbes.some((probe) => probe.agent === "Cursor" && probe.state === "available")
      ? "Cursor"
      : "Codex",
  );
  const [goal, setGoal] = useState(
    "检查触发条件、执行步骤和边界情况，提出可直接落地的 SKILL.md 优化建议。",
  );
  const selectedProbe = agentProbes.find((probe) => probe.agent === agent);
  const prompt = buildSkillOptimizationPrompt(variant, goal);
  const canSubmit = Boolean(
    variant.available && workspacePath && goal.trim() && selectedProbe?.state === "available",
  );

  return (
    <AlertDialog.Root open onOpenChange={(open) => !open && close()}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay forceMount className="dialog-overlay" />
        <AlertDialog.Content forceMount className="skill-optimization-dialog">
          <div className="dialog-icon"><Sparkles aria-hidden="true" /></div>
          <AlertDialog.Title>优化本机 Skill</AlertDialog.Title>
          <AlertDialog.Description>
            为 <strong>{variant.name}</strong> 生成带本地路径和当前内容的优化 Prompt，打开客户端后由你检查并手动发送。
          </AlertDialog.Description>
          <div className="skill-optimization-form">
            <label>
              客户端
              <SelectControl
                value={agent}
                values={["Cursor", "Codex"]}
                labels={{ Cursor: "Cursor", Codex: "Codex" }}
                onChange={(value) => setAgent(value as SkillOptimizationRequest["agent"])}
              />
            </label>
            <label>
              本次优化目标
              <textarea
                className="field-control"
                autoFocus
                value={goal}
                onChange={(event) => setGoal(event.target.value)}
              />
            </label>
            <div className="skill-optimization-context">
              <span>工作目录</span>
              <code>{workspacePath || "无法从 Skill 路径推断"}</code>
              <span>客户端状态</span>
              <em className={selectedProbe?.state === "available" ? "live" : "offline"}>
                {selectedProbe?.state === "available" ? "可用" : "未发现，请先安装或配置"}
              </em>
            </div>
            <details className="skill-prompt-details">
              <summary>预览 Prompt</summary>
              <pre>{prompt}</pre>
            </details>
          </div>
          <div className="dialog-actions">
            <AlertDialog.Cancel className="button button-secondary">取消</AlertDialog.Cancel>
            <AlertDialog.Action
              className="button button-primary"
              disabled={!canSubmit}
              onClick={() => submit({
                agent,
                skillName: variant.name,
                skillPath: variant.path,
                workspacePath,
                goal: goal.trim(),
                prompt,
              })}
            >
              {agent === "Cursor" ? "打开 Cursor 并填入" : "复制 Prompt 并打开"}
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function SkillUpdateSettings({
  commands,
  save,
}: {
  commands: SkillUpdateCommand[];
  save: (config: SkillUpdateCommand) => Promise<void>;
}) {
  const [draft, setDraft] = useState(commands);
  useEffect(() => setDraft(commands), [commands]);
  return (
    <div className="view settings">
      <section className="setting-card">
        <span className="eyebrow">SKILL UPDATE COMMANDS</span>
        <h2>Skill 更新命令</h2>
        <p>
          配置后，Skill 详情中的“更新 Skill”按钮会在对应目录执行命令。支持{" "}
          <code>{"{name}"}</code>、<code>{"{path}"}</code>、
          <code>{"{dir}"}</code> 和 <code>{"{agent}"}</code>。
        </p>
        {draft.map((command) => (
          <div className="skill-command-row" key={command.agent}>
            <b>{command.agent}</b>
            <input
              className="field-control skill-command-input"
              value={command.command}
              onChange={(event) =>
                setDraft((items) =>
                  items.map((item) =>
                    item.agent === command.agent
                      ? { ...item, command: event.target.value }
                      : item,
                  ),
                )
              }
              placeholder="例如：npx skills update {name}"
            />
            <label>
              <input
                type="checkbox"
                className="field-checkbox"
                checked={command.enabled}
                onChange={(event) =>
                  setDraft((items) =>
                    items.map((item) =>
                      item.agent === command.agent
                        ? { ...item, enabled: event.target.checked }
                        : item,
                    ),
                  )
                }
              />
              启用
            </label>
            <button
              className="button button-secondary"
              onClick={() => void save(command)}
            >
              保存
            </button>
          </div>
        ))}
      </section>
    </div>
  );
}

function App() {
  const [tab, setTab] = useState<Tab>("skills");
  const [theme, setTheme] = useState<ThemeId>(() => getStoredTheme());
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("overview");
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<number>();
  const [detail, setDetail] = useState<WorkspaceDetail>();
  const [inboxItems, setInboxItems] = useState<KnowledgeItem[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [agentRuns, setAgentRuns] = useState<AgentRun[]>([]);
  const [taskDetailId, setTaskDetailId] = useState<number>();
  const [roots, setRoots] = useState<KnowledgeRoot[]>([]);
  const [skillLibrary, setSkillLibrary] = useState<SkillLibrary>({
    groups: [],
    relations: [],
    function_groups: [],
    function_relations: [],
  });
  const [events, setEvents] = useState<Event[]>([]);
  const [statuses, setStatuses] = useState<Status[]>([]);
  const [syncStatus, setSyncStatus] = useState<SyncStatus>({
    stage: "idle",
    state: "idle",
    detail: "",
    started_at: null,
    finished_at: null,
  });
  const [agentProbes, setAgentProbes] = useState<AgentProbe[]>([]);
  const [updateCommands, setUpdateCommands] = useState<SkillUpdateCommand[]>(
    [],
  );
  const [copyTargets, setCopyTargets] = useState<SkillCopyTarget[]>([]);
  const [syncing, setSyncing] = useState(false);
  const [notice, setNotice] = useState("就绪");
  const [palette, setPalette] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation>();
  const [workspaceComposer, setWorkspaceComposer] = useState(false);
  const [captureComposer, setCaptureComposer] = useState(false);
  const motionRoot = useRef<HTMLElement>(null);
  const pageScroll = useRef<HTMLDivElement>(null);
  const motion = useRef<WorkbenchMotion | undefined>(undefined);
  const currentTheme =
    THEMES.find((candidate) => candidate.id === theme) ?? THEMES[0];

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const changeTheme = (nextTheme: ThemeId) => {
    const selectedTheme =
      THEMES.find((candidate) => candidate.id === nextTheme) ?? currentTheme;
    setTheme(selectedTheme.id);
    applyTheme(selectedTheme.id);
    persistTheme(selectedTheme.id);
    setNotice(`已应用：${selectedTheme.name} · 下次启动自动恢复`);
  };

  const load = async (preferredId?: number, syncRuns = false) => {
    if (syncRuns) {
      try {
        await call("refresh_agent_runs");
      } catch {
        setNotice("历史轨迹暂未刷新，工作区和 Skill 数据仍可用");
      }
    }
    const [
      nextWorkspaces,
      nextInbox,
      nextTasks,
      nextRoots,
      nextSkillLibrary,
      nextEvents,
      nextStatuses,
      nextSyncStatus,
      nextAgentProbes,
      nextUpdateCommands,
      nextCopyTargets,
      nextAgentRuns,
    ] = await Promise.all([
      call<Workspace[]>("list_workspaces"),
      call<KnowledgeItem[]>("list_knowledge_items"),
      call<Task[]>("list_tasks"),
      call<KnowledgeRoot[]>("list_knowledge_roots"),
      call<SkillLibrary>("list_skill_groups"),
      call<Event[]>("list_timeline"),
      call<Status[]>("adapter_status"),
      call<SyncStatus>("get_sync_status"),
      call<AgentProbe[]>("probe_agents"),
      call<SkillUpdateCommand[]>("list_skill_update_commands"),
      call<SkillCopyTarget[]>("list_skill_copy_targets"),
      call<AgentRun[]>("list_agent_runs", { taskId: null }),
    ]);
    setWorkspaces(nextWorkspaces);
    setInboxItems(nextInbox.filter((item) => item.status === "inbox"));
    setTasks(nextTasks);
    setAgentRuns(nextAgentRuns);
    setRoots(nextRoots);
    setSkillLibrary(nextSkillLibrary);
    setEvents(nextEvents);
    setStatuses(nextStatuses);
    setSyncStatus(nextSyncStatus);
    setAgentProbes(nextAgentProbes);
    setUpdateCommands(nextUpdateCommands);
    setCopyTargets(nextCopyTargets);
    const storedId = Number(
      localStorage.getItem("agent-workbench-active-workspace"),
    );
    const nextId =
      preferredId ??
      (nextWorkspaces.some((workspace) => workspace.id === storedId)
        ? storedId
        : nextWorkspaces[0]?.id);
    setActiveWorkspaceId(nextId);
    if (nextId) {
      const nextDetail = await call<WorkspaceDetail>("get_workspace_detail", {
        workspaceId: nextId,
      });
      setDetail(nextDetail);
    } else {
      setDetail(undefined);
    }
  };

  useEffect(() => {
    void load(undefined, false).catch(() => setNotice("读取失败，请稍后重试。"));
  }, []);
  useEffect(() => {
    if (!motionRoot.current) return;
    const controller = createWorkbenchMotion(motionRoot.current);
    motion.current = controller;
    controller.enterShell();
    return () => {
      controller.dispose();
      motion.current = undefined;
    };
  }, []);
  useEffect(() => {
    motion.current?.enterView();
  }, [
    tab,
    workspaceView,
    Boolean(detail),
    detail?.items.length,
    detail?.roots.length,
    detail?.events.length,
    inboxItems.length,
    tasks.length,
    skillLibrary.groups.length,
  ]);
  useEffect(() => {
    pageScroll.current?.scrollTo({ top: 0, left: 0, behavior: "auto" });
  }, [tab, workspaceView]);
  useEffect(() => {
    motion.current?.animateWorkspaceRows();
  }, [workspaces.length]);
  useEffect(() => {
    let frameId: number | null = null;
    let latestEvent: MouseEvent | null = null;
    const handleMouseMove = (event: MouseEvent) => {
      latestEvent = event;
      if (frameId !== null) return;
      frameId = requestAnimationFrame(() => {
        const currentEvent = latestEvent;
        if (!currentEvent) {
          frameId = null;
          return;
        }
        document.documentElement.style.setProperty("--mouse-x", `${currentEvent.clientX}px`);
        document.documentElement.style.setProperty("--mouse-y", `${currentEvent.clientY}px`);
        const target = currentEvent.target;
        if (target instanceof Element) {
          const spotlight = target.closest<HTMLElement>(
            ".spotlight-glow, .metric, .panel, .detail-panel, .setting-card, .danger-card, .task-card, .workbench-surface-card, .project-card, .skill-card, .skill-group-card, .sync-summary-card, .function-topology-panel, .skill-metadata, .skill-source-section, .skill-update-console",
          );
          if (spotlight) {
            const bounds = spotlight.getBoundingClientRect();
            spotlight.style.setProperty("--spotlight-x", `${currentEvent.clientX - bounds.left}px`);
            spotlight.style.setProperty("--spotlight-y", `${currentEvent.clientY - bounds.top}px`);
          }
        }
        frameId = null;
      });
    };
    window.addEventListener("mousemove", handleMouseMove, { passive: true });
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      if (frameId !== null) cancelAnimationFrame(frameId);
    };
  }, []);
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPalette(true);
      }
    };
    addEventListener("keydown", listener);
    return () => removeEventListener("keydown", listener);
  }, []);

  const selectWorkspace = async (id: number) => {
    setTab("workspace");
    setWorkspaceView("overview");
    setActiveWorkspaceId(id);
    localStorage.setItem("agent-workbench-active-workspace", String(id));
    try {
      await call("mark_workspace_opened", { id });
      const nextDetail = await call<WorkspaceDetail>("get_workspace_detail", {
        workspaceId: id,
      });
      setDetail(nextDetail);
      setWorkspaces(await call<Workspace[]>("list_workspaces"));
    } catch {
      setNotice("工作区打开失败，请稍后重试。");
    }
  };

  const refresh = async () => {
    setSyncing(true);
    setNotice("正在同步 Skill 与调用历史…");
    try {
      await call("refresh_all");
      await load(activeWorkspaceId, false);
      setNotice("同步完成 · Skill 与调用历史已更新");
    } catch (error) {
      setNotice(`同步失败：${callErrorMessage(error)}`);
    } finally {
      setSyncing(false);
    }
  };

  const clearPendingSources = () => {
    setConfirmation({
      title: "清空待整理来源？",
      description:
        "这会删除本地工作台中的待整理索引和捕获记录，但不会删除来源目录、原始文件或已归档知识。",
      confirmLabel: "清空待整理",
      danger: true,
      action: async () => {
        try {
          const deleted = await call<number>("clear_pending_sources");
          await load(activeWorkspaceId, false);
          setNotice(`已清空 ${deleted} 条待整理来源`);
        } catch (error) {
          setNotice(`清空失败：${callErrorMessage(error)}`);
        }
      },
    });
  };

  const createWorkspace = async (title: string, path: string) => {
    const workspace = await call<Workspace>("save_workspace", {
      workspace: {
        title,
        description: "",
        path: path || null,
        color: "violet",
      },
    });
    await load(workspace.id);
    setTab("workspace");
    setNotice("工作区已创建");
  };

  const createNote = async () => {
    if (!activeWorkspaceId) {
      setTab("settings");
      setNotice("请先创建一个工作区");
      return;
    }
    await call("create_note", {
      title: "未命名笔记",
      body: "",
      projectId: activeWorkspaceId,
      tags: [],
    });
    await load(activeWorkspaceId);
    setTab("workspace");
    setWorkspaceView("knowledge");
    setNotice("已创建一条新笔记");
  };

  const createCapture = async (draft: CaptureDraft) => {
    const projectId = draft.project_id ?? activeWorkspaceId ?? null;
    if (!projectId) {
      setTab("workspace");
      setNotice("请先创建或选择一个项目作为上下文");
      return;
    }
    await call("create_note", {
      title: draft.title.trim() || "未命名捕获",
      body: draft.body.trim(),
      projectId,
      tags: [draft.kind],
      status: "inbox",
      captureKind: draft.kind,
      sourceUri:
        draft.kind === "web" || draft.kind === "github"
          ? draft.body.trim() || null
          : null,
    });
    await load(projectId);
    setCaptureComposer(false);
    setTab("today");
    setNotice("已捕获到工作台，等待整理");
  };

  const promoteCaptureToTask = async (item: KnowledgeItem) => {
    if (!item.project_id) {
      setTab("workspace");
      setNotice("璇峰厛涓鸿繖鏉℃潵婧愬叧鑱旈」鐩啀鐢熸垚浠诲姟");
      return;
    }
    await call<Task>("promote_knowledge_item_to_task", {
      knowledgeItemId: item.id,
    });
    await load(activeWorkspaceId);
    setNotice("鏉ユ簮宸插彉鎴愬緟鎵ц浠诲姟");
  };

  const linkCaptureToTask = async (itemId: number, taskId: number) => {
    await call<Task>("link_knowledge_item_to_task", {
      knowledgeItemId: itemId,
      taskId,
    });
    await load(activeWorkspaceId);
    setNotice("鏉ユ簮宸插叧鑱旀埅瀹氫换鍔?");
  };

  const createTask = async (draft: TaskDraft) => {
    await call<Task>("create_task", {
      input: {
        title: draft.title,
        objective: draft.objective,
        steps: draft.steps,
        status: draft.status,
        priority: draft.priority,
        project_ids: draft.project_ids,
        source_kind: draft.source_kind ?? null,
        source_title: draft.source_title ?? null,
        source_uri: draft.source_uri ?? null,
        source_content: draft.source_content ?? null,
        source_knowledge_item_id: null,
        recommended_agent: draft.recommended_agent ?? null,
        recommended_skill: draft.recommended_skill ?? null,
      },
    });
    await load(activeWorkspaceId);
    setTab("today");
    setNotice("任务已创建");
  };

  const updateTaskStatus = async (task: Task, status: Task["status"]) => {
    await call<Task>("update_task", {
      id: task.id,
      input: {
        title: task.title,
        objective: task.objective,
        steps: task.steps,
        status,
        priority: task.priority,
        project_ids: task.projects.map((project) => project.id),
        source_kind: task.source?.kind ?? null,
        source_title: task.source?.title ?? null,
        source_uri: task.source?.uri ?? null,
        source_content: task.source?.content ?? null,
        source_knowledge_item_id: task.source?.knowledge_item_id ?? null,
        recommended_agent: task.recommended_agent,
        recommended_skill: task.recommended_skill,
      },
    });
    await load(activeWorkspaceId);
    setNotice("任务状态已更新");
  };

  const launchAgent = (task: Task) => {
    if (task.recommended_agent?.trim().toLowerCase() === "cursor") {
      void launchCursorTask(task);
      return;
    }
    const agent = task.recommended_agent?.trim();
    if (!agent) {
      setNotice("请先为任务指定推荐 Agent");
      return;
    }
    const probe = agentProbes.find(
      (item) => item.agent.toLowerCase() === agent.toLowerCase(),
    );
    if (!probe || probe.state !== "available") {
      setNotice(`${agent} 当前不可用，请先检查设置中的 Agent 探针`);
      return;
    }
    const workingDir = task.projects
      .map((project) => workspaces.find((item) => item.id === project.id)?.path)
      .find((path): path is string => Boolean(path));
    const prompt = buildTaskPrompt(task);
    const preview = prompt.length > 1600 ? `${prompt.slice(0, 1600)}\n…` : prompt;
    setConfirmation({
      title: `启动 ${agent} 执行任务？`,
      description: `工作目录：${workingDir || "未指定"}\n\n确认后将复制完整 Prompt，并打开 ${agent}。当前不会自动发送。\n\n${preview}`,
      confirmLabel: "复制并打开",
      action: async () => {
        try {
          await copyTextToClipboard(prompt);
          const result = await call<AgentRunStartResult>("start_agent_run", {
            request: {
              taskId: task.id,
              agent,
              workspacePath: workingDir ?? null,
              prompt,
            },
          });
          setAgentRuns(await call<AgentRun[]>("list_agent_runs", { taskId: null }));
          void result;
          setNotice(`${agent} 已打开，Prompt 已复制`);
        } catch {
          setNotice("Agent 启动失败，请检查设置中的命令和工作目录");
        }
      },
    });
  };

  const launchCursorTask = async (task: Task) => {
    const agent = task.recommended_agent?.trim();
    if (!agent) {
      setNotice("请先为任务指定推荐 Agent");
      return;
    }
    const probe = agentProbes.find(
      (item) => item.agent.toLowerCase() === agent.toLowerCase(),
    );
    if (!probe || probe.state !== "available") {
      setNotice(`${agent} 当前不可用，请先检查设置中的 Agent 探针`);
      return;
    }
    const workingDir = task.projects
      .map((project) => workspaces.find((item) => item.id === project.id)?.path)
      .find((path): path is string => Boolean(path));
    if (!workingDir) {
      const projectNames = task.projects.map((project) => project.title).join("、");
      setNotice(
        `${projectNames || "关联项目"} 未绑定项目目录，请到设置 → 工作区管理绑定目录`,
      );
      return;
    }
    const prompt = buildTaskPrompt(task);
    let plan: CursorLaunchPlan;
    try {
      plan = await call<CursorLaunchPlan>("inspect_cursor_launch", {
        workspacePath: workingDir,
      });
    } catch (error) {
      setNotice(`无法检查 Cursor 工作区：${String(error)}`);
      return;
    }
    const preview = prompt.length > 1600 ? `${prompt.slice(0, 1600)}\n…` : prompt;
    const windowDescription =
      plan.window_mode === "reuse"
        ? "将复用已打开的相同工作区窗口，并新建 Agent 会话"
        : "将新开 Cursor 窗口，并新建 Agent 会话";
    setConfirmation({
      title: "打开 Cursor 并准备新会话",
      description: `工作目录：${workingDir}\n\n${windowDescription}\nPrompt 会自动填入，但不会自动发送。\n\n${preview}`,
      confirmLabel: "打开 Cursor 并填入",
      action: async () => {
        try {
          await copyTextToClipboard(prompt);
          const result = await call<CursorLaunchResult>("launch_cursor_task", {
            request: {
              task_id: task.id,
              workspace_path: workingDir,
              prompt,
              auto_send: false,
            },
          });
          await reloadAgentRuns();
          if (result.status === "filled") {
            setNotice("Cursor 已准备好新会话，Prompt 已填入，请检查后手动发送");
          } else if (result.status === "fallback") {
            setNotice("Cursor 桌面会话未能自动填入，已切换到终端 Agent");
          } else {
            setNotice(result.error || "Cursor 启动失败");
          }
        } catch (error) {
          setNotice(`Cursor 启动失败：${String(error)}`);
        }
      },
    });
  };

  const launchSkillOptimization = (request: SkillOptimizationRequest) => {
    const preview = request.prompt.length > 1800
      ? `${request.prompt.slice(0, 1800)}\n…`
      : request.prompt;
    setConfirmation({
      title: `启动 ${request.agent} 优化 ${request.skillName}？`,
      description: `工作目录：${request.workspacePath}\n\nPrompt 会填入客户端，但不会自动发送。\n\n${preview}`,
      confirmLabel: request.agent === "Cursor" ? "打开 Cursor 并填入" : "复制 Prompt 并打开",
      action: async () => {
        try {
          if (request.agent === "Cursor") {
            const plan = await call<CursorLaunchPlan>("inspect_cursor_launch", {
              workspacePath: request.workspacePath,
            });
            const result = await call<CursorLaunchResult>("launch_cursor_task", {
              request: {
                task_id: null,
                workspace_path: request.workspacePath,
                prompt: request.prompt,
                auto_send: false,
              },
            });
            await reloadAgentRuns();
            if (result.status === "filled") {
              setNotice(plan.window_mode === "reuse" ? "Cursor 已复用工作区并填入 Prompt" : "Cursor 已打开并填入 Prompt");
            } else if (result.status === "fallback") {
              setNotice("Cursor 桌面会话未能自动填入，已切换到终端 Agent");
            } else {
              setNotice(result.error || "Cursor 启动失败");
            }
            return;
          }
          await copyTextToClipboard(request.prompt);
          await call<AgentRunStartResult>("start_agent_run", {
            request: {
              taskId: null,
              agent: "Codex",
              workspacePath: request.workspacePath,
              prompt: request.prompt,
            },
          });
          await reloadAgentRuns();
          setNotice("Codex 已打开，Prompt 已复制，请检查后手动发送");
        } catch (error) {
          setNotice(`${request.agent} 启动失败：${callErrorMessage(error)}`);
        }
      },
    });
  };

  const reloadAgentRuns = async (sync = false) => {
    if (sync) await call("refresh_agent_runs");
    setAgentRuns(await call<AgentRun[]>("list_agent_runs", { taskId: null }));
  };

  const saveAgentRunResult = async (
    input: AgentRunResultDraft,
  ) => {
    await call<AgentRun>("save_agent_run_result", {
      input: {
        runId: input.runId,
        resultSummary: input.result_summary,
        changedFiles: input.changed_files,
        verification: input.verification,
        unresolvedIssues: input.unresolved_issues,
      },
    });
    await reloadAgentRuns();
    setNotice("执行结果已保存，任务状态保持不变");
  };

  const resolveAgentRun = async (
    runId: string,
    action: "link" | "ignore",
    taskId?: number,
  ) => {
    await call<AgentRun>("resolve_agent_run", {
      input: { runId, action, taskId: taskId ?? null },
    });
    await reloadAgentRuns();
    setNotice(action === "link" ? "执行记录已关联任务" : "执行记录已忽略");
  };

  const selectedTask = tasks.find((task) => task.id === taskDetailId);

  const openCreateWorkspace = () => {
    setPalette(false);
    setTab("workspace");
    setWorkspaceComposer(true);
  };
  const currentTitle =
    tab === "today"
      ? "今日行动"
      : tab === "workspace"
      ? (detail?.workspace.title ?? "工作区")
      : {
          inbox: "收件箱",
          skills: "本机 Skill",
          review: "调用历史",
          settings: "设置",
        }[tab];
  const workbenchTitle =
    tab === "today"
      ? "工作台"
      : tab === "workspace"
      ? "项目上下文"
      : tab === "review"
      ? "调用历史"
      : tab === "skills"
      ? "本机 Skill"
      : tab === "inbox"
      ? "收件箱"
      : tab === "settings"
      ? "设置"
      : "工作台";
  void currentTitle;

  return (
    <main ref={motionRoot} className="app-shell knowledge-shell">
      <div className="ambient-nebula-container" aria-hidden="true">
        <span className="ambient-orb ambient-orb-purple" />
        <span className="ambient-orb ambient-orb-cyan" />
        <span className="ambient-grid" />
      </div>
      <Sidebar
        tab={tab}
        setTab={setTab}
        refreshing={syncing}
        refresh={refresh}
      />
      <section className="workspace">
        <header className="topbar workbench-topbar">
          <div>
            <p className="eyebrow">LOCAL SKILL WORKSPACE</p>
            <h1>{workbenchTitle}</h1>
          </div>
          <div className="top-actions">
            <button
              className="button button-ghost shortcut"
              onClick={() => setPalette(true)}
            >
              <Search aria-hidden="true" />
              <span>搜索 Skill</span>
              <kbd>Ctrl K</kbd>
            </button>
            <ThemeSwitcher theme={theme} selectTheme={changeTheme} />
            <button
              className="button button-primary"
              disabled={syncing}
              aria-busy={syncing}
              onClick={() => void refresh()}
            >
              <RefreshCw className={syncing ? "spin" : ""} aria-hidden="true" />
              {syncing ? "同步中…" : "同步 Skill"}
            </button>
            <div className="sync-state" aria-live="polite">
              <i className={syncing || syncStatus.state === "running" ? "pulse" : ""} />
              <span title={syncStatus.detail || undefined}>{notice}</span>
            </div>
          </div>
        </header>
        <div ref={pageScroll} className="page-scroll">
          {tab === "today" && (
            <TodayPage
              tasks={tasks}
              workspaces={workspaces}
              inboxItems={inboxItems}
              agentRuns={agentRuns}
              createTask={createTask}
              promoteCapture={promoteCaptureToTask}
              updateTaskStatus={updateTaskStatus}
              launchAgent={launchAgent}
              openTaskDetail={(task) => setTaskDetailId(task.id)}
              openCapture={() => setCaptureComposer(true)}
              openInbox={() => setTab("inbox")}
              openExecution={() => setTab("review")}
              clearPendingSources={clearPendingSources}
            />
          )}
          {tab === "workspace" && (
            <WorkspacePage
              detail={detail}
              view={workspaceView}
              setView={setWorkspaceView}
              skillLibrary={skillLibrary}
              updateCommands={updateCommands}
              copyTargets={copyTargets}
              agentProbes={agentProbes}
              launchOptimization={launchSkillOptimization}
              statuses={statuses}
              workspaces={workspaces}
              tasks={tasks}
              promoteToTask={promoteCaptureToTask}
              linkToTask={linkCaptureToTask}
              openTaskDetail={(task) => setTaskDetailId(task.id)}
              reload={() => load(activeWorkspaceId)}
              confirm={setConfirmation}
              goSettings={() => setTab("settings")}
              createWorkspace={openCreateWorkspace}
              createNote={createNote}
            />
          )}
          {tab === "inbox" && (
            <KnowledgePane
              mode="inbox"
              items={inboxItems}
              workspaces={workspaces}
              tasks={tasks}
              promoteToTask={promoteCaptureToTask}
              linkToTask={linkCaptureToTask}
              skillLibrary={skillLibrary}
              reload={() => load(activeWorkspaceId)}
            />
          )}
          {tab === "skills" && (
            <EnhancedSkillsPane
              library={skillLibrary}
              updateCommands={updateCommands}
              copyTargets={copyTargets}
              agentProbes={agentProbes}
              openSettings={() => setTab("settings")}
              launchOptimization={launchSkillOptimization}
              reloadSkills={async () =>
                setSkillLibrary(await call<SkillLibrary>("refresh_skills"))
              }
              confirm={setConfirmation}
            />
          )}
          {tab === "review" && (
            <SkillHistory events={events} refresh={refresh} refreshing={syncing} />
          )}
          {tab === "settings" && (
            <>
              <WorkspaceSettings
                roots={roots}
                workspaces={workspaces}
                agentProbes={agentProbes}
                syncStatus={syncStatus}
                theme={theme}
                selectTheme={changeTheme}
                reload={() => load(activeWorkspaceId)}
                confirm={setConfirmation}
              />
              <SkillUpdateSettings
                commands={updateCommands}
                save={async (config) => {
                  await call("save_skill_update_command", { config });
                  setUpdateCommands(
                    await call<SkillUpdateCommand[]>(
                      "list_skill_update_commands",
                    ),
                  );
                }}
              />
            </>
          )}
        </div>
      </section>
      {captureComposer && (
        <CaptureComposer
          workspaces={workspaces}
          activeWorkspaceId={activeWorkspaceId}
          save={createCapture}
          close={() => setCaptureComposer(false)}
        />
      )}
      {selectedTask && (
        <TaskDetailDialog
          task={selectedTask}
          runs={agentRuns.filter((run) => run.task_id === selectedTask.id)}
          close={() => setTaskDetailId(undefined)}
          refresh={() => reloadAgentRuns(true)}
          saveResult={saveAgentRunResult}
          updateStatus={updateTaskStatus}
        />
      )}
      <CommandPalette
        open={palette}
        close={() => setPalette(false)}
        skillLibrary={skillLibrary}
        setTab={setTab}
      />
      <ConfirmDialog
        confirmation={confirmation}
        close={() => setConfirmation(undefined)}
      />
    </main>
  );
}

function Sidebar({
  tab,
  setTab,
  refreshing,
  refresh,
}: {
  tab: Tab;
  setTab: (tab: Tab) => void;
  refreshing: boolean;
  refresh: () => Promise<void>;
}) {
  const nav: { id: Tab; label: string; icon: typeof Inbox }[] = [
    { id: "skills", label: "本机 Skill", icon: Sparkles },
    { id: "review", label: "调用历史", icon: History },
    { id: "settings", label: "设置", icon: Settings },
  ];
  return (
    <aside className="sidebar workspace-sidebar skill-shell-sidebar">
      <div className="brand">
        <span className="brand-mark">
          <PanelTop aria-hidden="true" />
        </span>
        <span>
          Agent
          <br />
          <strong>Workbench</strong>
        </span>
      </div>
      <p className="skill-shell-intro">本地 Skill 管理、调用追踪与客户端协作。</p>
      <nav className="global-nav" aria-label="Skill 工作台导航">
        {nav.map(({ id, label, icon: Icon }) => (
          <button
            className={tab === id ? "active border-beam" : ""}
            key={id}
            onClick={() => setTab(id)}
          >
            <Icon aria-hidden="true" />
            <span>{label}</span>
          </button>
        ))}
      </nav>
      <div className="sidebar-bottom">
        <button
          className="button button-secondary sync-button"
          disabled={refreshing}
          aria-busy={refreshing}
          onClick={() => void refresh()}
        >
          <RefreshCw className={refreshing ? "spin" : ""} aria-hidden="true" />
          {refreshing ? "同步中…" : "立即同步"}
        </button>
        <p>
          LOCAL FIRST
          <br />
          PRIVATE BY DESIGN
        </p>
      </div>
    </aside>
  );
}

function CaptureComposer({
  workspaces,
  activeWorkspaceId,
  save,
  close,
}: {
  workspaces: Workspace[];
  activeWorkspaceId?: number;
  save: (draft: CaptureDraft) => Promise<void>;
  close: () => void;
}) {
  const [kind, setKind] = useState<CaptureDraft["kind"]>("idea");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [projectId, setProjectId] = useState(
    activeWorkspaceId?.toString() ?? "",
  );
  const [saving, setSaving] = useState(false);
  const submit = async () => {
    if ((!title.trim() && !body.trim()) || saving) return;
    setSaving(true);
    try {
      await save({
        kind,
        title,
        body,
        project_id: projectId ? Number(projectId) : null,
      });
    } finally {
      setSaving(false);
    }
  };
  return (
    <AlertDialog.Root open onOpenChange={(open) => !open && close()}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay forceMount className="dialog-overlay" />
        <AlertDialog.Content forceMount className="capture-dialog">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">UNIVERSAL CAPTURE</span>
              <AlertDialog.Title>先记录，再决定下一步</AlertDialog.Title>
              <AlertDialog.Description>
                想法、网页、AI 对话、GitHub 或本地文件先进入待整理状态。
              </AlertDialog.Description>
            </div>
            <button className="icon-button" aria-label="关闭" onClick={close}>
              <X aria-hidden="true" />
            </button>
          </div>
          <div className="capture-kind-row" aria-label="来源类型">
            {(
              [
                ["idea", "想法"],
                ["web", "网页"],
                ["conversation", "AI 对话"],
                ["github", "GitHub"],
                ["file", "本地文件"],
              ] as const
            ).map(([value, label]) => (
              <button
                type="button"
                className={kind === value ? "active" : ""}
                key={value}
                onClick={() => setKind(value)}
              >
                {label}
              </button>
            ))}
          </div>
          <label className="capture-field">
            标题
            <input
              className="field-control"
              autoFocus
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="例如：把这篇性能优化文章应用到 Jupiter"
            />
          </label>
          <label className="capture-field">
            内容、URL 或摘录
            <textarea
              className="field-control"
              value={body}
              onChange={(event) => setBody(event.target.value)}
              placeholder="粘贴文章链接、AI 对话片段、GitHub Issue 或记录你的想法"
            />
          </label>
          <label className="capture-field">
            先放入哪个项目上下文？
            <SelectControl
              value={projectId}
              values={["", ...workspaces.map((workspace) => String(workspace.id))]}
              labels={{
                "": "稍后再关联",
                ...Object.fromEntries(
                  workspaces.map((workspace) => [String(workspace.id), workspace.title]),
                ),
              }}
              onChange={setProjectId}
            />
          </label>
          <div className="capture-dialog-footer">
            <span>保存后会出现在工作台的“待整理来源”中，不会自动生成任务。</span>
            <button
              className="button button-primary"
              disabled={(!title.trim() && !body.trim()) || saving}
              onClick={() => void submit()}
            >
              <Check aria-hidden="true" />
              {saving ? "保存中" : "保存到工作台"}
            </button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function TodayPage({
  tasks,
  workspaces,
  inboxItems,
  agentRuns,
  createTask,
  promoteCapture,
  updateTaskStatus,
  launchAgent,
  openTaskDetail,
  openCapture,
  openInbox,
  openExecution,
  clearPendingSources,
}: {
  tasks: Task[];
  workspaces: Workspace[];
  inboxItems: KnowledgeItem[];
  agentRuns: AgentRun[];
  createTask: (draft: TaskDraft) => Promise<void>;
  promoteCapture: (item: KnowledgeItem) => Promise<void>;
  updateTaskStatus: (task: Task, status: Task["status"]) => Promise<void>;
  launchAgent: (task: Task) => void;
  openTaskDetail: (task: Task) => void;
  openCapture: () => void;
  openInbox: () => void;
  openExecution: () => void;
  clearPendingSources: () => void;
}) {
  const [query, setQuery] = useState("");
  const [composerOpen, setComposerOpen] = useState(false);
  const visibleTasks = tasks.filter((task) =>
    [task.title, task.objective, ...task.projects.map((project) => project.title)]
      .join(" ")
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const activeCount = tasks.filter(
    (task) => task.status === "ready" || task.status === "in_progress",
  ).length;
  const blockedCount = tasks.filter((task) => task.status === "blocked").length;
  const completedCount = tasks.filter((task) => task.status === "done").length;
  return (
    <div className="view today-view">
      <section className="today-hero panel">
        <div>
          <span className="eyebrow">PERSONAL AI WORK OS</span>
          <h2>今天推进什么？</h2>
          <p>
            先把想法变成可追踪任务，再交给合适的 Agent 执行。启动前可以检查上下文，确认后复制 Prompt 并打开工具。
          </p>
        </div>
        <button className="button button-secondary" onClick={openCapture}>
          <Plus aria-hidden="true" />
          快速捕获
        </button>
        <button
          className="button button-primary"
          onClick={() => setComposerOpen((open) => !open)}
        >
          <Plus aria-hidden="true" />
          新建任务
        </button>
      </section>
      <section className="metrics today-metrics">
        <Metric value={activeCount} label="待推进" note="准备开始或正在执行" />
        <Metric value={blockedCount} label="被阻塞" note="需要你做决定" />
        <Metric value={completedCount} label="已完成" note="保留在执行历史中" />
      </section>
      {composerOpen && (
        <TaskComposer
          workspaces={workspaces}
          createTask={async (draft) => {
            await createTask(draft);
            setComposerOpen(false);
          }}
          close={() => setComposerOpen(false)}
        />
      )}
      <section className="workbench-surface-grid">
        <article className="panel workbench-surface-card">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">CAPTURE QUEUE</span>
              <h3>待整理来源</h3>
            </div>
            <div className="surface-heading-actions">
              <span className="surface-count">{inboxItems.length}</span>
              {inboxItems.length > 0 && (
                <button className="text-button surface-clear-button" onClick={clearPendingSources}>
                  清空
                </button>
              )}
            </div>
          </div>
          {inboxItems.slice(0, 3).map((item) => (
            <div className="surface-row" key={item.id}>
              <Inbox aria-hidden="true" />
              <div>
                <b>{item.title}</b>
                <span className="surface-meta">
                  {captureKindLabel(item.capture_kind)} · {item.project_title || "未关联项目"}
                </span>
                <button
                  className="surface-row-action"
                  onClick={() => void promoteCapture(item)}
                >
                  生成任务
                </button>
                <p>{item.excerpt || "等待关联项目或生成任务"}</p>
              </div>
            </div>
          ))}
          {!inboxItems.length && (
            <p className="surface-empty">暂无待整理来源，先捕获一个想法。</p>
          )}
          <button className="text-button" onClick={openCapture}>
            + 捕获新来源
          </button>
          <button className="text-button" onClick={openInbox}>
            处理待整理来源
          </button>
        </article>
        <article className="panel workbench-surface-card">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">AGENT ACTIVITY</span>
              <h3>执行记录</h3>
              <p className="surface-card-note">Agent 任务的启动、运行和完成状态</p>
            </div>
            <span className="surface-count">{agentRuns.length}</span>
          </div>
          {agentRuns.slice(0, 3).map((run) => (
            <div className="surface-row" key={run.id}>
              <History aria-hidden="true" />
              <div>
                <b>{run.agent} · {run.status}</b>
                <p>{run.workspace_path || "未绑定工作区"}</p>
              </div>
            </div>
          ))}
          {!agentRuns.length && (
            <p className="surface-empty">还没有执行记录，任务准备好后会出现在这里。</p>
          )}
          <button className="text-button" onClick={openExecution}>
            查看执行记录 →
          </button>
        </article>
      </section>
      <div className="today-toolbar">
        <label className="input-with-icon">
          <Search aria-hidden="true" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索任务或项目"
          />
        </label>
        <span className="toolbar-count">{visibleTasks.length} 个任务</span>
      </div>
      <section className="today-task-list">
        {visibleTasks.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            updateTaskStatus={updateTaskStatus}
            launchAgent={launchAgent}
            openDetails={() => openTaskDetail(task)}
          />
        ))}
        {!visibleTasks.length && (
          <Empty text="还没有任务。先记录一个想法，建立第一个行动闭环。" />
        )}
      </section>
    </div>
  );
}

function TaskComposer({
  workspaces,
  createTask,
  close,
}: {
  workspaces: Workspace[];
  createTask: (draft: TaskDraft) => Promise<void>;
  close: () => void;
}) {
  const [title, setTitle] = useState("");
  const [objective, setObjective] = useState("");
  const [steps, setSteps] = useState("");
  const [sourceContent, setSourceContent] = useState("");
  const [projectIds, setProjectIds] = useState<number[]>([]);
  const [agent, setAgent] = useState("");
  const [skill, setSkill] = useState("");
  const [saving, setSaving] = useState(false);
  const submit = async () => {
    if (!title.trim() || saving) return;
    setSaving(true);
    try {
      await createTask({
        title: title.trim(),
        objective: objective.trim(),
        steps: steps.trim(),
        status: "ready",
        priority: 0,
        project_ids: projectIds,
        source_kind: sourceContent.trim() ? "note" : undefined,
        source_title: sourceContent.trim() ? "手动捕获" : undefined,
        source_content: sourceContent.trim() || undefined,
        recommended_agent: agent || undefined,
        recommended_skill: skill.trim() || undefined,
      });
    } finally {
      setSaving(false);
    }
  };
  return (
    <section className="task-composer panel">
      <div className="panel-heading">
        <div>
          <span>QUICK CAPTURE</span>
          <b>把想法变成任务</b>
        </div>
        <button className="icon-button" aria-label="关闭" onClick={close}>
          <X aria-hidden="true" />
        </button>
      </div>
      <div className="task-form-grid">
        <label>
          任务标题
          <input
            className="field-control"
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="例如：把性能优化思路应用到 Jupiter"
          />
        </label>
        <label>
          目标
          <textarea
            className="field-control"
            value={objective}
            onChange={(event) => setObjective(event.target.value)}
            placeholder="完成后希望得到什么结果？"
          />
        </label>
        <label>
          执行步骤
          <textarea
            className="field-control"
            value={steps}
            onChange={(event) => setSteps(event.target.value)}
            placeholder="可以先写成多行步骤，后续由 AI 细化"
          />
        </label>
        <label>
          来源或背景
          <textarea
            className="field-control"
            value={sourceContent}
            onChange={(event) => setSourceContent(event.target.value)}
            placeholder="粘贴想法、网页摘要或对话片段"
          />
        </label>
      </div>
      <div className="task-form-meta">
        <div>
          <span className="form-label">关联项目（可多选）</span>
          <div className="task-project-options">
            {workspaces.map((workspace) => (
              <label key={workspace.id} className="check-option">
                <input
                  className="field-checkbox"
                  type="checkbox"
                  checked={projectIds.includes(workspace.id)}
                  onChange={(event) =>
                    setProjectIds((current) =>
                      event.target.checked
                        ? [...current, workspace.id]
                        : current.filter((id) => id !== workspace.id),
                    )
                  }
                />
                {workspace.title}
              </label>
            ))}
            {!workspaces.length && (
              <small className="muted">还没有项目，可先从左侧创建工作区。</small>
            )}
          </div>
        </div>
        <label>
          推荐 Agent
          <SelectControl
            value={agent}
            values={["", "Codex", "Claude", "Cursor"]}
            labels={{ "": "稍后推荐" }}
            onChange={setAgent}
          />
        </label>
        <label>
          推荐 Skill
          <input
            className="field-control"
            value={skill}
            onChange={(event) => setSkill(event.target.value)}
            placeholder="可选，例如 product-design"
          />
        </label>
      </div>
      <div className="detail-actions">
        <button
          className="button button-primary"
          disabled={!title.trim() || saving}
          onClick={() => void submit()}
        >
          <Check aria-hidden="true" />
          {saving ? "保存中" : "保存任务"}
        </button>
        <span className="form-hint">保存后可在任务卡片中预览上下文、复制 Prompt 并打开 Agent。</span>
      </div>
    </section>
  );
}

function PendingRuns({
  runs,
  tasks,
  resolveAgentRun,
}: {
  runs: AgentRun[];
  tasks: Task[];
  resolveAgentRun: (
    runId: string,
    action: "link" | "ignore",
    taskId?: number,
  ) => Promise<void>;
}) {
  const [selectedTasks, setSelectedTasks] = useState<Record<string, string>>(
    {},
  );
  return (
    <section className="panel pending-runs-panel">
      <div className="panel-heading">
        <div>
          <span>UNMATCHED EXECUTIONS</span>
          <b>待确认执行</b>
        </div>
        <small>自动解析到的会话尚未可靠匹配到任务</small>
      </div>
      <div className="pending-runs-list">
        {runs.slice(0, 5).map((run) => (
          <article className="pending-run" key={run.id}>
            <div>
              <strong>
                {run.agent} · {formatDate(run.created_at)}
              </strong>
              <p>{run.result_summary || run.prompt_snapshot || "暂无摘要"}</p>
              {run.result_source_path && <small>{run.result_source_path}</small>}
            </div>
            <div className="pending-run-actions">
              <select
                className="field-control"
                aria-label="选择关联任务"
                value={selectedTasks[run.id] ?? ""}
                onChange={(event) =>
                  setSelectedTasks((current) => ({
                    ...current,
                    [run.id]: event.target.value,
                  }))
                }
              >
                <option value="">选择任务</option>
                {tasks
                  .filter((task) => task.status !== "done")
                  .map((task) => (
                    <option key={task.id} value={task.id}>
                      {task.title}
                    </option>
                  ))}
              </select>
              <button
                className="button button-primary"
                disabled={!selectedTasks[run.id]}
                onClick={() =>
                  void resolveAgentRun(
                    run.id,
                    "link",
                    Number(selectedTasks[run.id]),
                  )
                }
              >
                关联
              </button>
              <button
                className="button button-ghost"
                onClick={() => void resolveAgentRun(run.id, "ignore")}
              >
                忽略
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function TaskDetailDialog({
  task,
  runs,
  close,
  refresh,
  saveResult,
  updateStatus,
}: {
  task: Task;
  runs: AgentRun[];
  close: () => void;
  refresh: () => Promise<void>;
  saveResult: (input: AgentRunResultDraft) => Promise<void>;
  updateStatus: (task: Task, status: Task["status"]) => Promise<void>;
}) {
  return (
    <div className="task-detail-overlay" role="presentation">
      <section
        className="task-detail-dialog panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="task-detail-title"
      >
        <header className="task-detail-header">
          <div>
            <span className="eyebrow">TASK EXECUTION</span>
            <h2 id="task-detail-title">{task.title}</h2>
            <p>{task.objective || "暂无任务目标"}</p>
          </div>
          <button className="icon-button" aria-label="关闭" onClick={close}>
            <X aria-hidden="true" />
          </button>
        </header>
        <div className="task-detail-toolbar">
          <span>{runs.length} 条执行记录</span>
          <div>
            <button className="button button-secondary" onClick={() => void refresh()}>
              <RefreshCw aria-hidden="true" />
              刷新历史
            </button>
            <button
              className="button button-primary"
              disabled={task.status === "done"}
              onClick={() => void updateStatus(task, "done")}
            >
              <Check aria-hidden="true" />
              标记任务完成
            </button>
          </div>
        </div>
        {task.source && (
          <section className="task-source-trace panel">
            <span className="eyebrow">SOURCE TRACE</span>
            <strong>{task.source.title || "未命名来源"}</strong>
            <p>
              {task.source.kind} · {task.source.knowledge_item_status ?? "快照来源"}
            </p>
            {task.source.uri && <a href={task.source.uri} target="_blank" rel="noreferrer">{task.source.uri}</a>}
            <small>{task.source.content.slice(0, 320)}</small>
          </section>
        )}
        <div className="task-run-list">
          {runs.map((run) => (
            <AgentRunEditor key={run.id} run={run} saveResult={saveResult} />
          ))}
          {!runs.length && (
            <div className="empty-state">
              <History aria-hidden="true" />
              <p>还没有执行记录。启动推荐 Agent 后，结果会在这里回填。</p>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function AgentRunEditor({
  run,
  saveResult,
}: {
  run: AgentRun;
  saveResult: (input: AgentRunResultDraft) => Promise<void>;
}) {
  const [summary, setSummary] = useState(run.result_summary);
  const [changedFiles, setChangedFiles] = useState(run.changed_files);
  const [verification, setVerification] = useState(run.verification);
  const [unresolved, setUnresolved] = useState(run.unresolved_issues);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    setSummary(run.result_summary);
    setChangedFiles(run.changed_files);
    setVerification(run.verification);
    setUnresolved(run.unresolved_issues);
  }, [
    run.id,
    run.result_summary,
    run.changed_files,
    run.verification,
    run.unresolved_issues,
  ]);
  const submit = async () => {
    setSaving(true);
    setError("");
    try {
      await saveResult({
        runId: run.id,
        result_summary: summary,
        changed_files: changedFiles,
        verification,
        unresolved_issues: unresolved,
      });
    } catch (caught) {
      setError(String(caught));
    } finally {
      setSaving(false);
    }
  };
  return (
    <article className="task-run-card">
      <header>
        <div>
          <strong>{run.agent}</strong>
          <span>{formatDate(run.created_at)}</span>
          <span className={"run-badge run-" + run.status}>{run.status}</span>
          <span className={"run-badge match-" + run.match_state}>
            {run.match_state === "matched" ? "已匹配" : "待确认"}
          </span>
        </div>
      <small>
        {run.transport} · {changeSourceLabel(run.change_source)}
      </small>
      </header>
      {run.workspace_path && <p className="run-workspace">{run.workspace_path}</p>}
      <details className="run-prompt">
        <summary>查看 Prompt 快照</summary>
        <pre>{run.prompt_snapshot}</pre>
      </details>
      {run.error_message && <p className="run-error">{run.error_message}</p>}
      <div className="run-result-grid">
        <label>
          结果摘要
          <textarea
            className="field-control"
            value={summary}
            onChange={(event) => setSummary(event.target.value)}
          />
        </label>
        <label>
          改动文件 / 产物
          <textarea
            className="field-control"
            value={changedFiles}
            onChange={(event) => setChangedFiles(event.target.value)}
          />
          <ChangedFilesPreview value={changedFiles} />
          {run.change_error && <small className="change-warning">{run.change_error}</small>}
        </label>
        <label>
          验证结果
          <textarea
            className="field-control"
            value={verification}
            onChange={(event) => setVerification(event.target.value)}
          />
        </label>
        <label>
          未解决问题
          <textarea
            className="field-control"
            value={unresolved}
            onChange={(event) => setUnresolved(event.target.value)}
          />
        </label>
      </div>
      {run.intermediate_files && (
        <details className="run-intermediate">
          <summary>会话中曾触碰但最终未保留</summary>
          <pre>{run.intermediate_files}</pre>
        </details>
      )}
      {run.raw_excerpt && (
        <details className="run-raw">
          <summary>查看解析片段</summary>
          <pre>{run.raw_excerpt}</pre>
        </details>
      )}
      {error && <p className="run-error">{error}</p>}
      <footer>
        <small>
          {run.result_state === "saved"
            ? "已保存用户确认结果"
            : "自动提取草稿，可编辑后保存"}
        </small>
        <button
          className="button button-primary"
          disabled={saving}
          onClick={() => void submit()}
        >
          <Check aria-hidden="true" />
          {saving ? "保存中…" : "保存结果"}
        </button>
      </footer>
    </article>
  );
}

function TaskCard({
  task,
  updateTaskStatus,
  launchAgent,
  openDetails,
}: {
  task: Task;
  updateTaskStatus: (task: Task, status: Task["status"]) => Promise<void>;
  launchAgent: (task: Task) => void;
  openDetails: () => void;
}) {
  const statusLabels: Record<Task["status"], string> = {
    draft: "草稿",
    ready: "待推进",
    in_progress: "执行中",
    blocked: "被阻塞",
    done: "已完成",
  };
  const statusValues: Task["status"][] = [
    "draft",
    "ready",
    "in_progress",
    "blocked",
    "done",
  ];
  return (
    <article className={"task-card panel task-" + task.status}>
      <div className="task-card-main">
        <div className="task-card-title">
          <span className="task-status-dot" />
          <div>
            <h3>{task.title}</h3>
            <div className="task-card-meta">
              {task.projects.map((project) => (
                <span key={project.id}>{project.title}</span>
              ))}
              {!task.projects.length && <span>未关联项目</span>}
              {task.recommended_agent && (
                <span>Agent：{task.recommended_agent}</span>
              )}
              {task.recommended_skill && (
                <span>Skill：{task.recommended_skill}</span>
              )}
            </div>
          </div>
        </div>
        <SelectControl
          value={task.status}
          values={statusValues}
          labels={statusLabels}
          onChange={(status) =>
            void updateTaskStatus(task, status as Task["status"])
          }
        />
      </div>
      {task.objective && <p className="task-objective">{task.objective}</p>}
      {task.steps && <p className="task-steps">{task.steps}</p>}
      {task.source && (
        <div className="task-source">
          <span>{task.source.title || "来源"}</span>
          <p>{task.source.content || task.source.uri}</p>
        </div>
      )}
      <footer className="task-card-footer">
        <small>更新于 {formatDate(task.updated_at)}</small>
        <div className="task-card-actions">
          <button className="button button-ghost" onClick={openDetails}>
            <History aria-hidden="true" />
            执行记录
          </button>
          {task.recommended_agent ? (
            <button
              className="button button-secondary"
              onClick={() => launchAgent(task)}
            >
              <ExternalLink aria-hidden="true" />
              {task.recommended_agent?.toLowerCase() === "cursor"
                ? "打开 Cursor 并准备会话"
                : `复制 Prompt 并打开 ${task.recommended_agent}`}
            </button>
          ) : (
            <span>请先指定推荐 Agent</span>
          )}
        </div>
      </footer>
    </article>
  );
}

function WorkspacePage({
  detail,
  view,
  setView,
  skillLibrary,
  updateCommands,
  copyTargets,
  agentProbes,
  launchOptimization,
  statuses,
  workspaces,
  tasks,
  promoteToTask,
  linkToTask,
  openTaskDetail,
  reload,
  confirm,
  goSettings,
  createWorkspace,
  createNote,
}: {
  detail?: WorkspaceDetail;
  view: WorkspaceView;
  setView: (view: WorkspaceView) => void;
  skillLibrary: SkillLibrary;
  updateCommands: SkillUpdateCommand[];
  copyTargets: SkillCopyTarget[];
  agentProbes: AgentProbe[];
  launchOptimization: (request: SkillOptimizationRequest) => void;
  statuses: Status[];
  workspaces: Workspace[];
  tasks: Task[];
  promoteToTask: (item: KnowledgeItem) => Promise<void>;
  linkToTask: (itemId: number, taskId: number) => Promise<void>;
  openTaskDetail: (task: WorkspaceTaskSummary) => void;
  reload: () => Promise<void>;
  confirm: (config: Confirmation) => void;
  goSettings: () => void;
  createWorkspace: () => void;
  createNote: () => Promise<void>;
}) {
  if (!detail)
    return (
      <div className="view workspace-empty-state">
        <div className="empty-illustration">
          <FolderOpen aria-hidden="true" />
        </div>
        <span className="eyebrow">WORKSPACE FIRST</span>
        <h2>从一个工作区开始</h2>
        <p>把项目资料、Agent 产物和可复用 Skill 放到同一个本地上下文里。</p>
        <button className="button button-primary" onClick={createWorkspace}>
          <Plus aria-hidden="true" />
          创建工作区
        </button>
      </div>
    );
  const tabs: { id: WorkspaceView; label: string; icon: typeof Library }[] = [
    { id: "overview", label: "概览", icon: Library },
    { id: "knowledge", label: "知识", icon: BookOpen },
    { id: "sources", label: "来源", icon: FolderOpen },
    { id: "skills", label: "Skill", icon: Sparkles },
    { id: "activity", label: "活动", icon: History },
  ];
  return (
    <div className="view workspace-view">
      <section className="workspace-head panel">
        <div className={`workspace-hero-mark ${detail.workspace.color}`}>
          <FolderOpen aria-hidden="true" />
        </div>
        <div className="workspace-head-copy">
          <span className="eyebrow">WORKSPACE</span>
          <h2>{detail.workspace.title}</h2>
          <p>
            {detail.workspace.description ||
              "本地知识与 Agent 能力的工作上下文"}
          </p>
          {detail.workspace.path && (
            <small>
              <FolderOpen aria-hidden="true" />
              {detail.workspace.path}
            </small>
          )}
        </div>
        <div className="workspace-head-actions">
          <button
            className="button button-primary"
            onClick={() => void createNote()}
          >
            <PenLine aria-hidden="true" />
            新建笔记
          </button>
        </div>
      </section>
      <div className="workspace-tabs" role="tablist">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            role="tab"
            aria-selected={view === id}
            className={view === id ? "active" : ""}
            key={id}
            onClick={() => setView(id)}
          >
            <Icon aria-hidden="true" />
            {label}
            {id === "knowledge" && detail.workspace.inbox_count > 0 && (
              <em>{detail.workspace.inbox_count}</em>
            )}
          </button>
        ))}
      </div>
      {view === "overview" && (
        <WorkspaceOverview
          detail={detail}
          statuses={statuses}
          setView={setView}
          openTaskDetail={openTaskDetail}
        />
      )}
      {view === "knowledge" && (
        <KnowledgePane
          mode="workspace"
          items={detail.items}
          workspaces={workspaces}
          tasks={tasks}
          promoteToTask={promoteToTask}
          linkToTask={linkToTask}
          skillLibrary={skillLibrary}
          reload={reload}
        />
      )}
      {view === "sources" && (
        <SourcesPane roots={detail.roots} goSettings={goSettings} />
      )}
      {view === "skills" && (
        <EnhancedSkillsPane
          library={skillLibrary}
          workspaceItems={detail.items}
          updateCommands={updateCommands}
          copyTargets={copyTargets}
          agentProbes={agentProbes}
          launchOptimization={launchOptimization}
          openSettings={goSettings}
          reloadSkills={async () => {
            await reload();
          }}
          confirm={confirm}
        />
      )}
      {view === "activity" && <ActivityPane events={detail.events} />}
    </div>
  );
}

function WorkspaceOverview({
  detail,
  statuses,
  setView,
  openTaskDetail,
}: {
  detail: WorkspaceDetail;
  statuses: Status[];
  setView: (view: WorkspaceView) => void;
  openTaskDetail: (task: WorkspaceTaskSummary) => void;
}) {
  return (
    <div className="workspace-overview">
      <section className="metrics workspace-metrics">
        <Metric
          value={detail.workspace.inbox_count}
          label="待处理内容"
          note="来自当前工作区的本地来源"
        />
        <Metric
          value={detail.workspace.knowledge_count}
          label="已归档知识"
          note="笔记与文件摘录"
        />
        <Metric
          value={detail.workspace.source_count}
          label="来源目录"
          note="持续同步的本地路径"
        />
      </section>
      <section className="overview-grid">
        <section className="panel">
          <PanelHeading
            kicker="RECENT TASKS"
            title="最近任务 / 待推进"
          />
          {detail.tasks.map((task) => (
            <button
              className="signal task-summary-row"
              key={task.id}
              onClick={() => openTaskDetail(task)}
            >
              <span>{task.status}</span>
              <b>{task.title}</b>
              <p>{formatDate(task.updated_at)}{task.source ? ` · 来源：${task.source.title}` : ""}</p>
            </button>
          ))}
          {!detail.tasks.length && <Empty text="当前工作区还没有任务。" />}
        </section>
        <section className="panel">
          <PanelHeading
            kicker="RECENT KNOWLEDGE"
            title="最近沉淀"
            action="查看全部"
            onClick={() => setView("knowledge")}
          />
          {detail.items
            .filter((item) => item.status === "archived")
            .slice(0, 6)
            .map((item) => (
              <KnowledgeCompact item={item} key={item.id} />
            ))}
          {!detail.items.filter((item) => item.status === "archived")
            .length && <Empty text="归档后的知识会出现在这里。" />}
        </section>
        <section className="panel">
          <PanelHeading
            kicker="SOURCE HEALTH"
            title="来源健康"
            action="管理来源"
            onClick={() => setView("sources")}
          />
          {detail.roots.map((root) => (
            <div className="source-health" key={root.id}>
              <i className={root.detail.includes("不存在") ? "warn" : "good"} />
              <div>
                <b>{root.name}</b>
                <span>{root.detail}</span>
              </div>
              <small>
                {root.kind === "agent_artifact" ? "Agent 产物" : "项目文件"}
              </small>
            </div>
          ))}
          {!detail.roots.length && <Empty text="还没有绑定来源目录。" />}
        </section>
        <section className="panel">
          <PanelHeading kicker="WORKBENCH HEALTH" title="同步健康" />
          {statuses.map((status) => (
            <div className="health" key={status.agent}>
              <i className={status.state === "ok" ? "good" : "warn"} />
              <b>{status.agent}</b>
              <span>{status.detail}</span>
            </div>
          ))}
        </section>
        <section className="panel">
          <PanelHeading
            kicker="RECENT ACTIVITY"
            title="近期活动"
            action="查看活动"
            onClick={() => setView("activity")}
          />
          {detail.events.slice(0, 4).map((event) => (
            <div className="signal" key={event.id}>
              <span>{event.agent}</span>
              <b>{event.skill}</b>
              <p>{event.summary}</p>
            </div>
          ))}
          {!detail.events.length && (
            <Empty text="同步后将显示当前工作区的 Skill 活动。" />
          )}
        </section>
      </section>
    </div>
  );
}

function KnowledgePane({
  mode,
  items,
  workspaces,
  tasks,
  promoteToTask,
  linkToTask,
  skillLibrary,
  reload,
}: {
  mode: "inbox" | "workspace";
  items: KnowledgeItem[];
  workspaces: Workspace[];
  tasks: Task[];
  promoteToTask: (item: KnowledgeItem) => Promise<void>;
  linkToTask: (itemId: number, taskId: number) => Promise<void>;
  skillLibrary: SkillLibrary;
  reload: () => Promise<void>;
}) {
  const [query, setQuery] = useState(""),
    [selected, setSelected] = useState<KnowledgeItem>();
  const list = items.filter(
    (item) =>
      (mode === "inbox"
        ? item.status === "inbox"
        : item.status !== "ignored") &&
      `${item.title} ${item.excerpt} ${item.tags.join(" ")}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  const update = async (
    item: KnowledgeItem,
    status: KnowledgeItem["status"],
    workspaceId = item.project_id,
    tags = item.tags,
    skillIds = item.skill_ids,
  ) => {
    await call("update_knowledge_item", {
      id: item.id,
      title: item.title,
      body: item.kind === "note" ? item.body : null,
      status,
      projectId: workspaceId,
      tags,
      skillIds,
    });
    await reload();
    setSelected({
      ...item,
      status,
      project_id: workspaceId,
      tags,
      skill_ids: skillIds,
    });
  };
  const promote = async (itemId: number) => {
    const item = items.find((candidate) => candidate.id === itemId);
    if (!item) return;
    await promoteToTask(item);
    await reload();
    setSelected(undefined);
  };
  const link = async (itemId: number, taskId: number) => {
    await linkToTask(itemId, taskId);
    await reload();
    setSelected(undefined);
  };
  return (
    <div className="view knowledge-workspace">
      <div className="workspace-toolbar">
        <label className="input-with-icon">
          <Search aria-hidden="true" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={
              mode === "inbox" ? "搜索待处理内容" : "搜索当前工作区的知识"
            }
          />
        </label>
        {mode === "inbox" && (
          <span className="toolbar-count">{list.length} 项待处理</span>
        )}
      </div>
      <div className="knowledge-grid">
        <section className="knowledge-list">
          {list.map((item) => (
            <button
              className={`knowledge-row ${selected?.id === item.id ? "selected" : ""}`}
              key={item.id}
              onClick={() => setSelected(item)}
            >
              <ItemIcon kind={item.kind} />
              <div>
                <b>{item.title}</b>
                <p>{item.excerpt || "无可显示摘录"}</p>
                <small>
                  {item.project_title || "未归入工作区"} ·{" "}
                  {formatDate(item.updated_at)}
                </small>
              </div>
              {!item.available && <em className="offline">来源失效</em>}
            </button>
          ))}
          {!list.length && (
            <Empty
              text={
                mode === "inbox"
                  ? "收件箱已清空。同步本地来源即可发现新内容。"
                  : "当前工作区还没有知识条目。"
              }
            />
          )}
        </section>
        <section className="detail-panel knowledge-detail">
          {selected ? (
            <ItemDetail
              item={selected}
              workspaces={workspaces}
              skillLibrary={skillLibrary}
              update={update}
              tasks={tasks}
              promoteToTask={promote}
              linkToTask={link}
            />
          ) : (
            <Empty text="从左侧选择一项内容查看详情。" />
          )}
        </section>
      </div>
    </div>
  );
}

function SourcesPane({
  roots,
  goSettings,
}: {
  roots: KnowledgeRoot[];
  goSettings: () => void;
}) {
  return (
    <div className="view source-pane">
      <div className="section-toolbar">
        <div>
          <span className="eyebrow">SOURCE DIRECTORIES</span>
          <h2>当前工作区的来源</h2>
          <p>来源目录中的文本文件会先进入收件箱，再由你决定是否归档。</p>
        </div>
        <button className="button button-primary" onClick={goSettings}>
          <Plus aria-hidden="true" />
          添加来源
        </button>
      </div>
      <section className="source-list panel">
        {roots.map((root) => (
          <article className="source-card" key={root.id}>
            <span className="item-icon">
              <FolderOpen aria-hidden="true" />
            </span>
            <div>
              <b>{root.name}</b>
              <p>{root.path}</p>
              <small>{root.detail}</small>
            </div>
            <em className={root.detail.includes("不存在") ? "offline" : "live"}>
              {root.detail.includes("不存在") ? "来源失效" : "已启用"}
            </em>
          </article>
        ))}
        {!roots.length && (
          <Empty text="还没有来源目录，添加一个本地目录开始收集知识。" />
        )}
      </section>
    </div>
  );
}
function TimelineEventRow({ event }: { event: Event }) {
  return (
    <article className="timeline-event" key={event.id}>
      <span>
        {event.agent} · {formatDate(event.at)}
        {event.timestamp_quality === "file" && <em className="timeline-time-fallback">文件时间</em>}
      </span>
      <b>{event.skill}</b>
      <p>{event.summary}</p>
      <small className="timeline-event-meta">
        会话 {shortSessionId(event.session_id)} · {event.occurrences} 次调用
      </small>
    </article>
  );
}

function ActivityPane({ events }: { events: Event[] }) {
  return (
    <div className="view activity-pane">
      <div className="section-toolbar">
        <div>
          <span className="eyebrow">ACTIVITY TRAIL</span>
          <h2>工作区活动</h2>
          <p>这里记录能从本地 Agent 历史中验证的 Skill 使用。</p>
        </div>
      </div>
      <section className="panel review-timeline">
        {events.map((event) => (
          <article className="timeline-event" key={event.id}>
            <span>
              {event.agent} · {formatDate(event.at)}
              {event.timestamp_quality === "file" && <em className="timeline-time-fallback">文件时间</em>}
            </span>
            <b>{event.skill}</b>
            <p>{event.summary}</p>
            <small className="timeline-event-meta">
              会话 {shortSessionId(event.session_id)} · {event.occurrences} 次调用
            </small>
          </article>
        ))}
        {!events.length && <Empty text="暂无可证实的 Skill 使用记录。" />}
      </section>
    </div>
  );
}

function ItemDetail({
  item,
  workspaces,
  skillLibrary: _skillLibrary,
  update,
  tasks,
  promoteToTask,
  linkToTask,
}: {
  item: KnowledgeItem;
  workspaces: Workspace[];
  skillLibrary: SkillLibrary;
  tasks: Task[];
  promoteToTask: (itemId: number) => Promise<void>;
  linkToTask: (itemId: number, taskId: number) => Promise<void>;
  update: (
    item: KnowledgeItem,
    status: KnowledgeItem["status"],
    workspaceId?: number | null,
    tags?: string[],
    skillIds?: number[],
  ) => Promise<void>;
}) {
  const [workspaceId, setWorkspaceId] = useState(
      item.project_id?.toString() ?? "",
    ),
    [tags, setTags] = useState(item.tags.join(", ")),
    [skillIds, setSkillIds] = useState(item.skill_ids.join(",")),
    [linkTaskId, setLinkTaskId] = useState(""),
    [actionError, setActionError] = useState("");
  const save = (status = item.status) =>
    void update(
      item,
      status,
      workspaceId ? Number(workspaceId) : null,
      tags.split(","),
      skillIds.split(",").filter(Boolean).map(Number),
    );
  const prepareAndPromote = async () => {
    setActionError("");
    try {
      await update(
        item,
        "inbox",
        workspaceId ? Number(workspaceId) : null,
        tags.split(","),
        skillIds.split(",").filter(Boolean).map(Number),
      );
      await promoteToTask(item.id);
    } catch (error) {
      setActionError(callErrorMessage(error));
    }
  };
  const prepareAndLink = async () => {
    if (!linkTaskId) return;
    setActionError("");
    try {
      await update(
        item,
        "inbox",
        workspaceId ? Number(workspaceId) : null,
        tags.split(","),
        skillIds.split(",").filter(Boolean).map(Number),
      );
      await linkToTask(item.id, Number(linkTaskId));
    } catch (error) {
      setActionError(callErrorMessage(error));
    }
  };
  return (
    <>
      <div className="detail-title">
        <div>
          <span className="eyebrow">
            {item.kind.replace("_", " ").toUpperCase()} · {item.status}
          </span>
          <h2>{item.title}</h2>
        </div>
        {item.source_path && (
          <button
            className="button button-ghost"
            onClick={() => void call("open_path", { path: item.source_path })}
          >
            <ExternalLink aria-hidden="true" />
            打开文件
          </button>
        )}
      </div>
      <p className="description">{item.excerpt || "此条目未保存文本摘录。"}</p>
      <div className="source-trace-row">
        <span className="agent-chip">{captureKindLabel(item.capture_kind)}</span>
        {item.source_uri && (
          <a href={item.source_uri} target="_blank" rel="noreferrer">
            {item.source_uri}
          </a>
        )}
      </div>
      {item.kind === "note" && (
        <textarea
          className="note-body"
          defaultValue={item.body}
          onBlur={(event) =>
            void call("update_knowledge_item", {
              id: item.id,
              title: item.title,
              body: event.target.value,
              status: item.status,
              projectId: item.project_id,
              tags: item.tags,
              skillIds: item.skill_ids,
            })
          }
        />
      )}
      <div className="meta-editor">
        <label>
          归入工作区
          <SelectControl
            value={workspaceId}
            values={[
              "",
              ...workspaces.map((workspace) => workspace.id.toString()),
            ]}
            labels={{
              "": "未归入工作区",
              ...Object.fromEntries(
                workspaces.map((workspace) => [
                  workspace.id.toString(),
                  workspace.title,
                ]),
              ),
            }}
            onChange={setWorkspaceId}
          />
        </label>
        <label>
          标签
          <input
            className="field-control workspace-input"
            value={tags}
            onChange={(event) => setTags(event.target.value)}
            placeholder="用逗号分隔"
          />
        </label>
        <label>
          关联 Skill ID
          <input
            className="field-control workspace-input"
            value={skillIds}
            onChange={(event) => setSkillIds(event.target.value)}
            placeholder="可选，逗号分隔"
          />
        </label>
      </div>
      <div className="detail-actions">
        {item.status === "inbox" ? (
          <>
            <button
              className="button button-primary"
              disabled={!workspaceId}
              onClick={() => void prepareAndPromote()}
            >
              生成任务
            </button>
            <SelectControl
              value={linkTaskId}
              values={["", ...tasks.map((task) => task.id.toString())]}
              labels={{
                "": "选择已有任务",
                ...Object.fromEntries(tasks.map((task) => [task.id.toString(), task.title])),
              }}
              onChange={setLinkTaskId}
            />
            <button
              className="button button-secondary"
              disabled={!workspaceId || !linkTaskId}
              onClick={() => void prepareAndLink()}
            >
              关联已有任务
            </button>
            <button
              className="button button-secondary"
              onClick={() => save("ignored")}
            >
              忽略
            </button>
            <button
              className="button button-primary"
              onClick={() => save("archived")}
            >
              <Archive aria-hidden="true" />
              归档到知识库
            </button>
          </>
        ) : (
          <button className="button button-primary" onClick={() => save()}>
            保存整理
          </button>
        )}
      </div>
      {actionError && <p className="run-error">{actionError}</p>}
    </>
  );
}

function SkillsPane({
  library,
  workspaceItems,
}: {
  library: SkillLibrary;
  workspaceItems?: KnowledgeItem[];
}) {
  const [query, setQuery] = useState(""),
    [selectedKey, setSelectedKey] = useState(""),
    [selectedVariantId, setSelectedVariantId] = useState<number>(),
    [card, setCard] = useState<Card>(),
    [dirty, setDirty] = useState(false),
    [state, setState] = useState("已保存");
  const timer = useRef<number | undefined>(undefined);
  const linked = useMemo(
    () => new Set((workspaceItems ?? []).flatMap((item) => item.skill_ids)),
    [workspaceItems],
  );
  const linkedGroups = useMemo(
    () =>
      new Set(
        library.groups
          .filter((group) =>
            group.variants.some((variant) => linked.has(variant.id)),
          )
          .map((group) => group.key),
      ),
    [library.groups, linked],
  );
  const roots = library.groups.filter((group) => group.root);
  const matches = (group: SkillGroup) =>
    `${group.name} ${group.description} ${group.variants.map((variant) => `${variant.agent} ${variant.path} ${variant.description}`).join(" ")}`
      .toLowerCase()
      .includes(query.toLowerCase());
  const list = roots.filter(matches);
  const selected = library.groups.find((group) => group.key === selectedKey);
  const selectedVariant =
    selected?.variants.find((variant) => variant.id === selectedVariantId) ??
    selected?.variants[0];
  const chooseVariant = async (variant: SkillVariant) => {
    setSelectedVariantId(variant.id);
    setCard(undefined);
    setDirty(false);
    setState("读取中");
    try {
      setCard(
        (await call<Card | null>("get_usage_card", { skillId: variant.id })) ??
          emptyCard(variant.id),
      );
      setState("已保存");
    } catch {
      setState("读取失败");
    }
  };
  const chooseGroup = (group: SkillGroup) => {
    setSelectedKey(group.key);
    const variant =
      group.variants.find((item) => item.available) ?? group.variants[0];
    if (variant) void chooseVariant(variant);
  };
  useEffect(() => {
    if (!selectedKey && list.length) chooseGroup(list[0]);
  }, [list.length]);
  useEffect(() => {
    document
      .querySelector<HTMLElement>(".skill-group-detail")
      ?.scrollTo({ top: 0, left: 0 });
  }, [selectedKey]);
  useEffect(() => {
    if (!card || !dirty) return;
    setState("正在保存");
    clearTimeout(timer.current);
    timer.current = window.setTimeout(async () => {
      try {
        await call("save_usage_card", { card });
        setDirty(false);
        setState("已保存");
      } catch {
        setState("保存失败");
      }
    }, 800);
    return () => clearTimeout(timer.current);
  }, [card, dirty]);
  const linkedCount = workspaceItems ? linkedGroups.size : undefined;
  return (
    <div className="view skills-view">
      <div className="workspace-toolbar">
        <label className="input-with-icon">
          <Search aria-hidden="true" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索 Skill、Agent 或路径"
          />
        </label>
        {linkedCount !== undefined && (
          <span className="toolbar-count">{linkedCount} 个已关联</span>
        )}
      </div>
      <div className="skills-library-grid">
        <section className="skills-grid skill-group-list">
          {list.map((group) => (
            <SkillGroupCard
              key={group.key}
              group={group}
              selected={selectedKey === group.key}
              linked={linkedGroups.has(group.key)}
              onClick={() => chooseGroup(group)}
            />
          ))}
          {!list.length && <Empty text="没有匹配的 Skill。" />}
        </section>
        <section className="detail-panel skill-group-detail">
          {selected && selectedVariant ? (
            <>
              <div className="detail-title">
                <div>
                  <span className="eyebrow">
                    SKILL GROUP · {selected.variants.length} 个版本
                  </span>
                  <h2>{selected.name}</h2>
                </div>
                <button
                  className="button button-ghost"
                  onClick={() =>
                    void call("open_path", { path: selectedVariant.path })
                  }
                >
                  <ExternalLink aria-hidden="true" />
                  打开当前来源
                </button>
              </div>
              <p className="description">
                {selected.description || "暂无描述"}
              </p>
              <SkillGraph
                group={selected}
                library={library}
                selectGroup={chooseGroup}
              />
              <VariantList
                group={selected}
                selectedId={selectedVariant.id}
                choose={chooseVariant}
              />
              <div className="skill-variant-detail">
                <div className="skill-variant-heading">
                  <div>
                    <span className="eyebrow">
                      {selectedVariant.agent} VERSION · {state}
                    </span>
                    <h3>{selectedVariant.name}</h3>
                  </div>
                  <span
                    className={selectedVariant.available ? "live" : "offline"}
                  >
                    {selectedVariant.available ? "可用" : "来源失效"}
                  </span>
                </div>
                <p className="variant-path">{selectedVariant.path}</p>
                {card && (
                  <UsageCard
                    card={card}
                    change={(next) => {
                      setCard(next);
                      setDirty(true);
                    }}
                  />
                )}
              </div>
            </>
          ) : (
            <Empty text="从左侧选择一个主 Skill，查看版本与关系图。" />
          )}
        </section>
      </div>
    </div>
  );
}
function SkillGroupCard({
  group,
  selected,
  linked,
  onClick,
}: {
  group: SkillGroup;
  selected: boolean;
  linked: boolean;
  onClick: () => void;
}) {
  const agents = [...new Set(group.variants.map((variant) => variant.agent))];
  return (
    <button
      className={`skill-card skill-group-card spotlight-glow ${selected ? "selected border-beam" : ""}`}
      onClick={onClick}
    >
      <span className="agent-mark">{group.name.slice(0, 1).toUpperCase()}</span>
      <div className="skill-group-card-copy">
        <div className="skill-card-title">
          <b>{group.name}</b>
          {linked && (
            <span className="linked-dot" aria-label="当前工作区已关联" />
          )}
        </div>
        <p>{group.description || "暂无描述"}</p>
        <div className="skill-group-meta">
          {agents.slice(0, 3).map((agent) => (
            <span
              className={`agent-chip agent-${agent.toLowerCase()}`}
              key={agent}
            >
              {agent}
            </span>
          ))}
          {agents.length > 3 && (
            <span className="agent-chip">+{agents.length - 3}</span>
          )}
          <small>
            {group.variants.length} 个版本
            {group.children.length
              ? ` · ${group.children.length} 个从属 Skill`
              : ""}
          </small>
        </div>
        {group.unresolved_relations.length > 0 && (
          <small className="relation-warning">有未解析关系</small>
        )}
      </div>
    </button>
  );
}
function VariantList({
  group,
  selectedId,
  choose,
}: {
  group: SkillGroup;
  selectedId: number;
  choose: (variant: SkillVariant) => Promise<void>;
}) {
  return (
    <section className="skill-variants">
      <div className="skill-section-heading">
        <div>
          <span className="card-kicker">SOURCE VARIANTS</span>
          <b>Agent 版本</b>
        </div>
        <small>{group.variants.length} 个来源</small>
      </div>
      {group.variants.map((variant) => (
        <button
          className={`skill-variant-row ${selectedId === variant.id ? "selected" : ""}`}
          key={variant.id}
          onClick={() => void choose(variant)}
        >
          <span className="agent-mark small">{variant.agent.slice(0, 1)}</span>
          <div>
            <b>{variant.agent}</b>
            <p>{variant.path}</p>
          </div>
          <span className={variant.available ? "live" : "offline"}>
            {variant.available ? "可用" : "失效"}
          </span>
          {variant.has_card && (
            <Check className="variant-check" aria-label="已有使用卡片" />
          )}
        </button>
      ))}
    </section>
  );
}
function svgLabelLines(value: string, maxLength: number) {
  const text = value.trim();
  if (!text) return ["未命名"];
  if (text.length <= maxLength) return [text];
  const first = text.slice(0, maxLength);
  const remaining = text.slice(maxLength, maxLength * 2 - 1);
  return [first, remaining ? `${remaining}…` : "…"];
}

function SvgWrappedLabel({
  value,
  x,
  y,
  maxLength,
  className,
}: {
  value: string;
  x: number;
  y: number;
  maxLength: number;
  className: string;
}) {
  const lines = svgLabelLines(value, maxLength);
  const firstLineY = y - ((lines.length - 1) * 16) / 2;
  return (
    <text
      className={className}
      x={x}
      y={firstLineY}
      textAnchor="middle"
      dominantBaseline="middle"
    >
      {lines.map((line, index) => (
        <tspan x={x} dy={index === 0 ? 0 : 16} key={`${line}-${index}`}>
          {line}
        </tspan>
      ))}
    </text>
  );
}

function FunctionTopologyGraphVertical({
  library,
  query,
  selectedKey,
  selectedFunction,
  selectGroup,
  setFunction,
}: {
  library: SkillLibrary;
  query: string;
  selectedKey: string;
  selectedFunction: string;
  selectGroup: (group: SkillGroup) => void;
  setFunction: (key: string) => void;
}) {
  const topologyRef = useRef<HTMLDivElement>(null);
  const [columns, setColumns] = useState(3);
  const groups = library.function_groups;
  const normalizedQuery = query.trim().toLowerCase();
  const skills = library.groups.filter(
    (group) =>
      !normalizedQuery ||
      `${group.name} ${group.description} ${group.variants
        .map((variant) => `${variant.agent} ${variant.path}`)
        .join(" ")}`
        .toLowerCase()
        .includes(normalizedQuery),
  );

  useEffect(() => {
    const element = topologyRef.current;
    if (!element) return;
    const updateColumns = (width: number) => {
      setColumns(width < 560 ? 1 : width < 820 ? 2 : 3);
    };
    updateColumns(element.getBoundingClientRect().width);
    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width;
      if (width) updateColumns(width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const groupWidth = 180;
  const skillWidth = 150;
  const rowStep = 112;
  const top = 82;
  const width = columns === 1 ? 500 : columns === 2 ? 700 : 900;
  const groupX = columns === 1 ? 100 : 125;
  const skillStartX = columns === 1 ? 325 : columns === 2 ? 385 : 420;
  const skillStep = columns === 1 ? 0 : columns === 2 ? 185 : 185;
  const skillRows = Math.max(1, Math.ceil(skills.length / columns));
  const height = Math.max(420, top + Math.max(groups.length, skillRows) * rowStep + 56);
  const groupPositions = new Map(
    groups.map((group, index) => [group.key, { x: groupX, y: top + index * rowStep }]),
  );
  const skillPositions = new Map(
    skills.map((group, index) => [
      group.key,
      {
        x: skillStartX + (index % columns) * skillStep,
        y: top + Math.floor(index / columns) * rowStep,
      },
    ]),
  );

  return (
    <section className="function-topology-panel">
      <div className="skill-section-heading">
        <div>
          <span className="card-kicker">FUNCTION TOPOLOGY</span>
          <b>按功能分组的 Skill 网络</b>
        </div>
        <small>{groups.length} 个功能组 · {skills.length} 个 Skill</small>
      </div>
      <div className="function-topology-scroll" ref={topologyRef}>
        <svg
          className="function-topology"
          viewBox={`0 0 ${width} ${height}`}
          role="img"
          aria-label="按功能分组的 Skill 网络拓扑图"
        >
          <defs>
            <marker
              id="function-arrow-vertical"
              markerWidth="8"
              markerHeight="8"
              refX="7"
              refY="4"
              orient="auto"
            >
              <path d="M0,0 L8,4 L0,8 Z" fill="currentColor" />
            </marker>
          </defs>
          {library.function_relations.map((relation) => {
            const group = groupPositions.get(relation.source);
            const skill = skillPositions.get(relation.target);
            if (!group || !skill) return null;
            return (
              <line
                className="function-edge graph-flow-pulse"
                key={`${relation.source}-${relation.target}`}
                x1={group.x + groupWidth / 2}
                y1={group.y}
                x2={skill.x - skillWidth / 2}
                y2={skill.y}
                markerEnd="url(#function-arrow-vertical)"
              />
            );
          })}
          {groups.map((group) => {
            const position = groupPositions.get(group.key);
            if (!position) return null;
            const active = selectedFunction === group.key;
            return (
              <g
                className={`function-node-group ${active ? "active" : ""}`}
                key={group.key}
                role="button"
                tabIndex={0}
                onClick={() => setFunction(group.key)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setFunction(group.key);
                  }
                }}
              >
                <rect
                  className={`function-node ${active ? "active" : ""}`}
                  x={position.x - groupWidth / 2}
                  y={position.y - 30}
                  width={groupWidth}
                  height="60"
                  rx="13"
                />
                <SvgWrappedLabel
                  value={group.name}
                  x={position.x}
                  y={position.y}
                  maxLength={13}
                  className="function-node-label"
                />
              </g>
            );
          })}
          {skills.map((group) => {
            const position = skillPositions.get(group.key);
            if (!position) return null;
            const active = group.key === selectedKey;
            const muted = Boolean(
              selectedFunction && !group.function_keys.includes(selectedFunction),
            );
            return (
              <g
                className={`function-skill-node-group ${muted ? "muted" : ""}`}
                key={group.key}
                role="button"
                tabIndex={0}
                onClick={() => selectGroup(group)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    selectGroup(group);
                  }
                }}
              >
                <rect
                  className={`function-skill-node ${active ? "current" : ""}`}
                  x={position.x - skillWidth / 2}
                  y={position.y - 33}
                  width={skillWidth}
                  height="66"
                  rx="12"
                />
                <SvgWrappedLabel
                  value={group.name}
                  x={position.x}
                  y={position.y - 9}
                  maxLength={14}
                  className="function-skill-label"
                />
                <text
                  className="function-skill-meta"
                  x={position.x}
                  y={position.y + 23}
                  textAnchor="middle"
                >
                  {group.variants.length} 个版本{group.root ? "" : " · 从属"}
                </text>
              </g>
            );
          })}
        </svg>
      </div>
      <p className="function-topology-note">
        连线表示 Skill 属于对应功能组；一个 Skill 可以同时出现在多个功能组。节点按可用宽度自动换行，分类依据本地描述、正文和已声明的 GitHub 来源。
      </p>
    </section>
  );
}

function FunctionTopologyGraph({
  library,
  query,
  selectedKey,
  selectedFunction,
  selectGroup,
  setFunction,
}: {
  library: SkillLibrary;
  query: string;
  selectedKey: string;
  selectedFunction: string;
  selectGroup: (group: SkillGroup) => void;
  setFunction: (key: string) => void;
}) {
  const groups = library.function_groups;
  const normalizedQuery = query.trim().toLowerCase();
  const skills = library.groups.filter((group) => !normalizedQuery || `${group.name} ${group.description} ${group.variants.map((variant) => `${variant.agent} ${variant.path}`).join(" ")}`.toLowerCase().includes(normalizedQuery));
  const width = Math.max(1120, skills.length * 170 + 120);
  const positions = new Map(skills.map((group, index) => [group.key, 85 + index * 170]));
  const groupPositions = new Map(groups.map((group, index) => [group.key, 130 + index * 180]));
  return (
    <section className="function-topology-panel">
      <div className="skill-section-heading">
        <div><span className="card-kicker">FUNCTION TOPOLOGY</span><b>按功能分组的 Skill 网络</b></div>
        <small>{groups.length} 个功能组 · {skills.length} 个主 Skill</small>
      </div>
      <div className="function-topology-scroll">
        <svg className="function-topology" viewBox={`0 0 ${width} 330`} role="img" aria-label="按功能分组的 Skill 网络拓扑图">
          <defs><marker id="function-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 Z" fill="currentColor" /></marker></defs>
          {library.function_relations.map((relation) => {
            const groupX = groupPositions.get(relation.source);
            const skillX = positions.get(relation.target);
            if (groupX === undefined || skillX === undefined) return null;
            return <line className="function-edge graph-flow-pulse" key={`${relation.source}-${relation.target}`} x1={groupX} y1="86" x2={skillX} y2="212" markerEnd="url(#function-arrow)" />;
          })}
          {groups.map((group) => {
            const x = groupPositions.get(group.key) ?? 0;
            const active = selectedFunction === group.key;
            return <g className={`function-node-group ${active ? "active" : ""}`} key={group.key} role="button" tabIndex={0} onClick={() => setFunction(group.key)} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); setFunction(group.key); } }}>
              <rect className={`function-node ${active ? "active" : ""}`} x={x - 76} y="36" width="152" height="50" rx="13" />
              <text className="function-node-label" x={x} y="66" textAnchor="middle">{truncate(group.name, 16)}</text>
            </g>;
          })}
          {skills.map((group) => {
            const x = positions.get(group.key) ?? 0;
            const active = group.key === selectedKey;
            const muted = Boolean(selectedFunction && !group.function_keys.includes(selectedFunction));
            return <g className={`function-skill-node-group ${muted ? "muted" : ""}`} key={group.key} role="button" tabIndex={0} onClick={() => selectGroup(group)} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); selectGroup(group); } }}>
              <rect className={`function-skill-node ${active ? "current" : ""}`} x={x - 72} y="210" width="144" height="58" rx="12" />
              <text className="function-skill-label" x={x} y="235" textAnchor="middle">{truncate(group.name, 18)}</text>
              <text className="function-skill-meta" x={x} y="176" textAnchor="middle">{group.variants.length} 版本</text>
            </g>;
          })}
        </svg>
      </div>
      <p className="function-topology-note">连线表示 Skill 属于对应功能组；一个 Skill 可以同时出现在多个功能组。分类依据本地描述、正文和已声明的 GitHub 来源。</p>
    </section>
  );
}

function SkillGraph({
  group,
  library,
  selectGroup,
}: {
  group: SkillGroup;
  library: SkillLibrary;
  selectGroup: (group: SkillGroup) => void;
}) {
  const parents = group.parents
    .map((key) => library.groups.find((item) => item.key === key))
    .filter((item): item is SkillGroup => Boolean(item));
  const children = group.children
    .map((key) => library.groups.find((item) => item.key === key))
    .filter((item): item is SkillGroup => Boolean(item));
  const hasRelations =
    parents.length + children.length > 0 ||
    group.unresolved_relations.length > 0;
  if (!hasRelations)
    return (
      <section className="skill-graph-panel">
        <div className="skill-section-heading">
          <div>
            <span className="card-kicker">RELATION GRAPH</span>
            <b>主从关系</b>
          </div>
        </div>
        <div className="skill-graph-empty">
          <CircleDashed aria-hidden="true" />
          <span>暂无已声明关系</span>
        </div>
      </section>
    );
  const height = Math.max(
      230,
      Math.max(parents.length, children.length) * 68 + 54,
    ),
    centerX = 380,
    centerY = height / 2,
    parentX = 18,
    childX = 560;
  const positions = (items: SkillGroup[], x: number) =>
    items.map((item, index) => ({
      item,
      x,
      y: (height - Math.max(items.length, 1) * 54) / 2 + index * 68 + 27,
    }));
  const parentNodes = positions(parents, parentX),
    childNodes = positions(children, childX);
  return (
    <section className="skill-graph-panel">
      <div className="skill-section-heading">
        <div>
          <span className="card-kicker">RELATION GRAPH</span>
          <b>主从关系</b>
        </div>
        {group.cycle && (
          <small className="relation-warning">检测到循环关系</small>
        )}
      </div>
      <div className="skill-graph-scroll">
        <svg
          className="skill-graph"
          viewBox={`0 0 760 ${height}`}
          role="img"
          aria-label={`${group.name} 的 Skill 关系图`}
        >
          <defs>
            <marker
              id="skill-arrow"
              markerWidth="8"
              markerHeight="8"
              refX="7"
              refY="4"
              orient="auto"
            >
              <path d="M0,0 L8,4 L0,8 Z" fill="currentColor" />
            </marker>
          </defs>
          {parentNodes.map((node) => (
            <line
              className="graph-edge graph-flow-pulse"
              key={`parent-${node.item.key}`}
              x1={node.x + 166}
              y1={node.y}
              x2={centerX - 8}
              y2={centerY}
              markerEnd="url(#skill-arrow)"
            />
          ))}
          {childNodes.map((node) => (
            <line
              className="graph-edge graph-flow-pulse"
              key={`child-${node.item.key}`}
              x1={centerX + 166}
              y1={centerY}
              x2={node.x - 8}
              y2={node.y}
              markerEnd="url(#skill-arrow)"
            />
          ))}
          {parentNodes.map((node) => (
            <GraphNode
              node={node.item}
              x={node.x}
              y={node.y}
              onClick={() => selectGroup(node.item)}
              key={node.item.key}
            />
          ))}
          <GraphNode node={group} x={centerX} y={centerY} current />
          {childNodes.map((node) => (
            <GraphNode
              node={node.item}
              x={node.x}
              y={node.y}
              onClick={() => selectGroup(node.item)}
              key={node.item.key}
            />
          ))}
        </svg>
      </div>
      {group.unresolved_relations.length > 0 && (
        <p className="relation-warning graph-warning">
          未解析：{group.unresolved_relations.join("、")}
        </p>
      )}
    </section>
  );
}
function GraphNode({
  node,
  x,
  y,
  current,
  onClick,
}: {
  node: SkillGroup;
  x: number;
  y: number;
  current?: boolean;
  onClick?: () => void;
}) {
  const content = (
    <>
      <rect
        className={`graph-node ${current ? "current" : ""}`}
        x={x}
        y={y - 25}
        width="166"
        height="50"
        rx="10"
      />
      <text className="graph-node-label" x={x + 12} y={y - 2}>
        {truncate(node.name, 21)}
      </text>
      <text className="graph-node-meta" x={x + 12} y={y + 15}>
        {node.variants.length} 个版本 · {node.children.length} 个从属
      </text>
    </>
  );
  return onClick ? (
    <g
      className="graph-node-group"
      role="button"
      tabIndex={0}
      aria-label={`查看 ${node.name}`}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
    >
      {content}
    </g>
  ) : (
    <g>{content}</g>
  );
}
function truncate(value: string, length: number) {
  return value.length > length ? `${value.slice(0, length - 1)}…` : value;
}
function UsageCard({
  card,
  change,
}: {
  card: Card;
  change: (card: Card) => void;
}) {
  const fields: [keyof Omit<Card, "skill_id">, string][] = [
    ["scenarios", "适用场景"],
    ["triggers", "触发提示 / 关键词"],
    ["steps", "推荐步骤"],
    ["notes", "个人备注"],
    ["pitfalls", "踩坑与边界"],
    ["links", "参考链接"],
    ["tags", "标签（逗号分隔）"],
  ];
  return (
    <div className="use-card">
      <div className="card-kicker">MY OPERATING NOTES · 自动保存</div>
      {fields.map(([key, label]) => (
        <label key={key}>
          {label}
          <textarea
            value={card[key]}
            onChange={(event) => change({ ...card, [key]: event.target.value })}
          />
        </label>
      ))}
    </div>
  );
}

function ThemeSwitcher({
  theme,
  selectTheme,
}: {
  theme: ThemeId;
  selectTheme: (theme: ThemeId) => void;
}) {
  const [open, setOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number; width: number } | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const activeTheme = THEMES.find((candidate) => candidate.id === theme) ?? THEMES[0];

  useEffect(() => {
    if (!open) return;
    const updateMenuPosition = () => {
      const trigger = triggerRef.current;
      if (!trigger) return;
      const rect = trigger.getBoundingClientRect();
      const width = Math.min(360, window.innerWidth - 32);
      const left = Math.min(
        Math.max(16, rect.right - width),
        Math.max(16, window.innerWidth - width - 16),
      );
      setMenuPosition({ top: rect.bottom + 10, left, width });
    };
    updateMenuPosition();
    window.addEventListener("resize", updateMenuPosition);
    window.addEventListener("scroll", updateMenuPosition, true);
    const closeOnOutside = (event: PointerEvent) => {
      if (!(event.target instanceof Node)) return;
      if (rootRef.current?.contains(event.target)) return;
      if (event.target instanceof Element && event.target.closest(".theme-switcher-portal")) return;
      setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener("pointerdown", closeOnOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("resize", updateMenuPosition);
      window.removeEventListener("scroll", updateMenuPosition, true);
      document.removeEventListener("pointerdown", closeOnOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  const chooseTheme = (nextTheme: ThemeId) => {
    selectTheme(nextTheme);
    setOpen(false);
    requestAnimationFrame(() => triggerRef.current?.focus());
  };

  return (
    <div className="theme-switcher" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        className="button button-ghost theme-switcher-trigger"
        aria-label={`切换主题，当前为${activeTheme.name}`}
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen((current) => !current)}
      >
        <Palette aria-hidden="true" />
        <span className="theme-trigger-copy">
          <small>主题</small>
          <strong>{activeTheme.name}</strong>
        </span>
        <span className={`theme-dot theme-dot-${activeTheme.id}`} aria-hidden="true" />
        <ChevronDown aria-hidden="true" />
      </button>
      {open && menuPosition && createPortal(
        <div className="theme-switcher-portal" style={{ top: menuPosition.top, left: menuPosition.left, width: menuPosition.width }}>
        <div className="theme-switcher-menu" role="dialog" aria-label="选择界面主题">
          <div className="theme-switcher-heading">
            <div>
              <span className="card-kicker">THEME SYSTEM</span>
              <strong>选择界面主题</strong>
            </div>
            <span>即时应用</span>
          </div>
          <div className="theme-switcher-options" role="radiogroup" aria-label="界面主题">
            {THEMES.map((candidate) => (
              <ThemeCard
                key={candidate.id}
                theme={candidate}
                selected={candidate.id === theme}
                compact
                onSelect={chooseTheme}
              />
            ))}
          </div>
        </div>
        </div>,
        document.body,
      )}
    </div>
  );
}

function ThemeCard({
  theme,
  selected,
  compact = false,
  onSelect,
}: {
  theme: ThemeDefinition;
  selected: boolean;
  compact?: boolean;
  onSelect: (theme: ThemeId) => void;
}) {
  return (
    <button
      type="button"
      className={`theme-card${compact ? " compact" : ""}${selected ? " selected" : ""}`}
      role="radio"
      aria-checked={selected}
      aria-label={`${theme.name} · ${theme.description}`}
      onClick={() => onSelect(theme.id)}
    >
      <span className="theme-card-image">
        <img src={theme.image} alt={`${theme.name}主题背景`} />
        <span className="theme-card-image-shade" aria-hidden="true" />
        {selected && <Check className="theme-card-check" aria-hidden="true" />}
      </span>
      <span className="theme-card-copy">
        <strong>{theme.name}</strong>
        <small>{theme.description}</small>
      </span>
      <span className="theme-card-swatches" aria-hidden="true">
        <i className={`theme-swatch theme-swatch-${theme.id}`} />
        <i className={`theme-swatch theme-swatch-${theme.id}-highlight`} />
        <i className={`theme-swatch theme-swatch-${theme.id}-support`} />
      </span>
    </button>
  );
}

function ThemeSettings({
  theme,
  selectTheme,
}: {
  theme: ThemeId;
  selectTheme: (theme: ThemeId) => void;
}) {
  const activeTheme = THEMES.find((candidate) => candidate.id === theme) ?? THEMES[0];
  return (
    <section className="setting-card theme-settings-card">
      <div className="panel-heading">
        <div>
          <span className="eyebrow">INTERFACE THEME</span>
          <h2>界面主题</h2>
        </div>
        <span className="theme-current-label">当前：{activeTheme.name}</span>
      </div>
      <p>选择一套星空背景与配色，主题会立即应用到所有页面、面板、文本和交互状态。</p>
      <div className="theme-grid" role="radiogroup" aria-label="界面主题">
        {THEMES.map((candidate) => (
          <ThemeCard
            key={candidate.id}
            theme={candidate}
            selected={candidate.id === theme}
            onSelect={selectTheme}
          />
        ))}
      </div>
      <small className="theme-persistence-note" aria-live="polite">
        已选择 {activeTheme.name} · 下次启动自动恢复
      </small>
    </section>
  );
}

function WorkspaceSettings({
  roots,
  workspaces,
  agentProbes,
  syncStatus,
  theme,
  selectTheme,
  reload,
  confirm,
}: {
  roots: KnowledgeRoot[];
  workspaces: Workspace[];
  agentProbes: AgentProbe[];
  syncStatus: SyncStatus;
  theme: ThemeId;
  selectTheme: (theme: ThemeId) => void;
  reload: () => Promise<void>;
  confirm: (config: Confirmation) => void;
}) {
  const [name, setName] = useState(""),
    [kind, setKind] = useState("project"),
    [path, setPath] = useState(""),
    [workspaceId, setWorkspaceId] = useState(""),
    [workspaceError, setWorkspaceError] = useState("");
  const browse = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") setPath(picked);
  };
  const add = async () => {
    await call("add_knowledge_root", {
      name: name || (kind === "project" ? "项目文件" : "Agent 产物"),
      kind,
      path,
      projectId: workspaceId ? Number(workspaceId) : null,
    });
    setName("");
    setPath("");
    await reload();
  };
  const bindWorkspaceDirectory = async (workspace: Workspace) => {
    setWorkspaceError("");
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked !== "string") return;
      await call("save_workspace", {
        workspace: {
          id: workspace.id,
          title: workspace.title,
          description: workspace.description,
          path: picked,
          color: workspace.color,
        },
      });
      await reload();
    } catch (error) {
      setWorkspaceError(`绑定目录失败：${String(error)}`);
    }
  };
  const removeWorkspace = (workspace: Workspace) =>
    confirm({
      title: `删除“${workspace.title}”？`,
      description:
        "工作区内的知识和来源目录会变为未分类，但不会删除任何本地文件。",
      confirmLabel: "删除工作区",
      danger: true,
      action: async () => {
        await call("delete_workspace", { id: workspace.id });
        await reload();
      },
    });
  return (
    <div className="view settings">
      <ThemeSettings theme={theme} selectTheme={selectTheme} />
      <section className="setting-card agent-probe-card">
        <div className="panel-heading">
          <div>
            <span className="eyebrow">AGENT ADAPTER PROBE</span>
            <h2>Agent 启动探针</h2>
          </div>
          <span className="offline">Prompt 填入，不自动发送</span>
        </div>
        <p>检查本机是否能找到 Codex、Claude、Cursor 命令。Cursor 会按关联工作区复用或新开窗口，使用 Ctrl+N 创建 Agent 会话并填入 Prompt；不会自动发送。</p>
        <div className="agent-probe-list">
          {agentProbes.map((probe) => (
            <div className="agent-probe-row" key={probe.agent}>
              <i className={probe.state === "available" ? "good" : "warn"} />
              <div>
                <b>{probe.agent}</b>
                <span>{probe.detail}</span>
                <small>{probe.executable || `命令：${probe.command}`}</small>
              </div>
              <em className={probe.state === "available" ? "live" : "offline"}>
                {probe.state === "available" ? "可用" : "需配置"}
              </em>
            </div>
          ))}
          {!agentProbes.length && <Empty text="正在读取本机 Agent 能力" />}
        </div>
      </section>
      <section className={`setting-card sync-summary-card ${syncStatus.state}`}>
        <div className="panel-heading">
          <div>
            <span className="eyebrow">SYNC STATUS</span>
            <h2>同步状态</h2>
          </div>
          <em className={syncStatus.state === "failed" ? "offline" : "live"}>
            {syncStatus.state === "running"
              ? "同步中"
              : syncStatus.state === "failed"
                ? "失败"
                : "已完成"}
          </em>
        </div>
        <div className="sync-summary-body">
          <strong>
            {syncStatus.stage === "skills"
              ? "Skill 来源扫描"
              : syncStatus.stage === "timeline"
                ? "Agent 历史同步"
                : syncStatus.stage === "knowledge"
                  ? "知识来源同步"
                  : "等待下一次同步"}
          </strong>
          <p>{syncStatus.detail || "尚未执行过完整同步。"}</p>
          {syncStatus.finished_at && (
            <small>最近完成：{formatDate(syncStatus.finished_at)}</small>
          )}
        </div>
      </section>
      <section className="setting-card">
        <span className="eyebrow">WORKSPACES</span>
        <h2>工作区管理</h2>
        <p>工作区只组织本地索引和 Agent 产物，不会移动或删除你的原始文件。要启动 Cursor，必须先绑定项目目录。</p>
        {workspaceError && <p className="form-error">{workspaceError}</p>}
        {workspaces.map((workspace) => (
          <div className="setting-row" key={workspace.id}>
            <b>{workspace.title}</b>
            <span>
              {workspace.path || "未绑定项目目录"}
              <small>
                {workspace.knowledge_count} 条知识 · {workspace.source_count}{" "}
                个来源
              </small>
            </span>
            <button
              className="button button-secondary"
              onClick={() => void bindWorkspaceDirectory(workspace)}
            >
              <FolderOpen aria-hidden="true" />
              {workspace.path ? "更换目录" : "绑定目录"}
            </button>
            <button
              className="button button-danger"
              onClick={() => removeWorkspace(workspace)}
            >
              <Trash2 aria-hidden="true" />
              删除
            </button>
          </div>
        ))}
      </section>
      <section className="setting-card">
        <span className="eyebrow">FILE COLLECTION SOURCES</span>
        <h2>文件归集目录</h2>
        <p>
          支持 Markdown、TXT、JSON、JSONL、CSV、YAML/YML；单文件最大 1
          MB。移除索引不会删除本地文件。
        </p>
        {roots.map((root) => (
          <div className="setting-row" key={root.id}>
            <b>{root.name}</b>
            <span>
              {root.path}
              <small>{root.detail}</small>
            </span>
            <em className={root.kind === "agent_artifact" ? "live" : "offline"}>
              {root.kind === "agent_artifact" ? "Agent 产物" : "项目文件"}
            </em>
            <button
              className="button button-danger"
              onClick={() =>
                confirm({
                  title: "移除来源并清除索引？",
                  description:
                    "将清除该来源产生的本地索引，但不会删除目录中的任何文件。",
                  confirmLabel: "清除索引",
                  danger: true,
                  action: async () => {
                    await call("purge_knowledge_root", { id: root.id });
                    await reload();
                  },
                })
              }
            >
              清除索引
            </button>
          </div>
        ))}
        <div className="knowledge-root-form">
          <input
            className="field-control knowledge-root-input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="来源名称（可选）"
          />
          <SelectControl
            value={kind}
            values={["project", "agent_artifact"]}
            labels={{ project: "项目文件", agent_artifact: "Agent 产物" }}
            onChange={setKind}
          />
          <SelectControl
            value={workspaceId}
            values={[
              "",
              ...workspaces.map((workspace) => workspace.id.toString()),
            ]}
            labels={{
              "": "不关联工作区",
              ...Object.fromEntries(
                workspaces.map((workspace) => [
                  workspace.id.toString(),
                  workspace.title,
                ]),
              ),
            }}
            onChange={setWorkspaceId}
          />
          <input
            className="field-control knowledge-root-input"
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="选择要归集文件的目录"
          />
          <button
            className="button button-secondary"
            onClick={() => void browse()}
          >
            <FolderOpen aria-hidden="true" />
            选择目录
          </button>
          <button
            className="button button-primary"
            disabled={!path}
            onClick={() => void add()}
          >
            添加文件来源
          </button>
        </div>
      </section>
    </div>
  );
}

function SkillHistory({
  events,
  refresh,
  refreshing,
}: {
  events: Event[];
  refresh: () => Promise<void>;
  refreshing: boolean;
}) {
  const [query, setQuery] = useState("");
  const [agent, setAgent] = useState("全部");
  const list = events.filter(
    (event) =>
      (agent === "全部" || event.agent === agent) &&
      `${event.skill} ${event.summary} ${event.project_path ?? ""}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  return (
    <div className="view skill-history-view">
      <div className="skill-history-heading">
        <div>
          <h2>Skill 调用历史</h2>
          <p>从本机 Agent 历史中识别出的 Skill 使用记录。</p>
        </div>
        <button className="button button-secondary" disabled={refreshing} aria-busy={refreshing} onClick={() => void refresh()}>
          <RefreshCw className={refreshing ? "spin" : ""} aria-hidden="true" />
          {refreshing ? "同步中…" : "重新同步历史"}
        </button>
      </div>
      <section className="panel skill-history-panel">
        <div className="skill-history-filters">
          <label className="input-with-icon">
            <Search aria-hidden="true" />
            <input aria-label="搜索调用历史" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索 Skill、摘要或项目路径" />
          </label>
          <SelectControl
            value={agent}
            values={["全部", ...new Set(events.map((event) => event.agent))]}
            onChange={setAgent}
          />
          <span>{list.length} 条记录</span>
        </div>
        <p className="timeline-rule">统计口径：Agent × 会话 × Skill；同一会话中的同一 Skill 只计一次，重复调用计入次数。</p>
        {list.map((event) => (
          <article className="timeline-event" key={event.id}>
            <div className="skill-history-event-topline">
              <span>{event.agent} · {formatDate(event.at)}</span>
              {event.timestamp_quality === "file" && <em className="timeline-time-fallback">文件时间</em>}
            </div>
            <b>{event.skill}</b>
            <p>{event.summary}</p>
            <small className="timeline-event-meta">
              会话 {shortSessionId(event.session_id)} · {event.occurrences} 次调用{event.project_path ? ` · ${event.project_path}` : ""}
            </small>
          </article>
        ))}
        {!list.length && <Empty text={events.length ? "没有匹配的调用记录。" : "暂无可证实的 Skill 使用记录，请先同步本机历史。"} />}
      </section>
    </div>
  );
}

function Metric({
  value,
  label,
  note,
}: {
  value: number;
  label: string;
  note: string;
}) {
  return (
    <article className="metric">
      <b data-motion-counter={value}>{value}</b>
      <span>{label}</span>
      <small>{note}</small>
    </article>
  );
}
function PanelHeading({
  kicker,
  title,
  action,
  onClick,
}: {
  kicker: string;
  title: string;
  action?: string;
  onClick?: () => void;
}) {
  return (
    <div className="panel-heading">
      <div>
        <span>{kicker}</span>
        <b>{title}</b>
      </div>
      {action && (
        <button className="text-button" onClick={onClick}>
          {action} →
        </button>
      )}
    </div>
  );
}
function ItemIcon({ kind }: { kind: KnowledgeItem["kind"] }) {
  const Icon =
    kind === "note" ? PenLine : kind === "agent_artifact" ? Sparkles : FileText;
  return (
    <span className="item-icon">
      <Icon aria-hidden="true" />
    </span>
  );
}
function KnowledgeCompact({ item }: { item: KnowledgeItem }) {
  return (
    <div className="knowledge-compact">
      <ItemIcon kind={item.kind} />
      <div>
        <b>{item.title}</b>
        <p>{item.excerpt}</p>
      </div>
      <small>{formatDate(item.updated_at)}</small>
    </div>
  );
}
function Empty({ text }: { text: string }) {
  return (
    <div className="empty">
      <CircleDashed aria-hidden="true" />
      <p>{text}</p>
    </div>
  );
}

function SelectControl({
  value,
  values,
  labels,
  onChange,
}: {
  value: string;
  values: Iterable<string>;
  labels?: Record<string, string>;
  onChange: (value: string) => void;
}) {
  return (
    <Select.Root value={value} onValueChange={onChange}>
      <Select.Trigger className="select-trigger" aria-label="选择条件">
        <Select.Value>{labels?.[value] ?? value}</Select.Value>
        <Select.Icon className="select-chevron">
          <ChevronDown aria-hidden="true" />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content
          className="select-content"
          position="popper"
          sideOffset={6}
        >
          <Select.Viewport>
            {[...values].map((item) => (
              <Select.Item className="select-item" value={item} key={item}>
                <Select.ItemText>{labels?.[item] ?? item}</Select.ItemText>
                <Select.ItemIndicator>
                  <Check aria-hidden="true" />
                </Select.ItemIndicator>
              </Select.Item>
            ))}
          </Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}

function CommandPalette({
  open: isOpen,
  close,
  skillLibrary,
  setTab,
}: {
  open: boolean;
  close: () => void;
  skillLibrary: SkillLibrary;
  setTab: (tab: Tab) => void;
}) {
  const [query, setQuery] = useState("");
  useEffect(() => {
    if (isOpen) setQuery("");
  }, [isOpen]);
  useEffect(
    () => animatePortalState(isOpen, ".command-palette"),
    [isOpen],
  );
  const results = [
    ...skillLibrary.groups.map((group) => ({
      label: group.name,
      detail: `Skill · ${group.variants.length} 个版本`,
      kind: "skill" as const,
      id: group.key,
    })),
  ]
    .filter((result) =>
      result.label.toLowerCase().includes(query.toLowerCase()),
    )
    .slice(0, 8);
  return (
    <AlertDialog.Root open={isOpen} onOpenChange={(next) => !next && close()}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay forceMount className="dialog-overlay" />
        <AlertDialog.Content forceMount className="command-palette">
          <div className="input-with-icon">
            <Search aria-hidden="true" />
            <input
              autoFocus
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              aria-label="搜索 Skill"
              placeholder="搜索 Skill"
            />
          </div>
          <div className="palette-actions">
            <button
              className="palette-action"
              onClick={() => {
                setTab("skills");
                close();
              }}
            >
              <Sparkles aria-hidden="true" />
              <span>本机 Skill</span>
              <small>管理与优化</small>
            </button>
            <button
              className="palette-action"
              onClick={() => {
                setTab("review");
                close();
              }}
            >
              <History aria-hidden="true" />
              <span>调用历史</span>
              <small>查看本机使用记录</small>
            </button>
            <button className="palette-action" onClick={() => { setTab("settings"); close(); }}>
              <Settings aria-hidden="true" />
              <span>设置</span>
              <small>Agent 与更新命令</small>
            </button>
            {results.map((result) => (
              <button
                className="palette-action"
                key={`${result.kind}-${result.id}`}
                onClick={() => {
                  setTab("skills");
                  close();
                }}
              >
                <BookOpen aria-hidden="true" />
                <span>{result.label}</span>
                <small>{result.detail}</small>
              </button>
            ))}
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
function ConfirmDialog({
  confirmation,
  close,
}: {
  confirmation?: Confirmation;
  close: () => void;
}) {
  useEffect(
    () => animatePortalState(Boolean(confirmation), ".confirm-dialog"),
    [confirmation],
  );
  return (
    <AlertDialog.Root
      open={Boolean(confirmation)}
      onOpenChange={(open) => !open && close()}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay forceMount className="dialog-overlay" />
        <AlertDialog.Content forceMount className="confirm-dialog">
          <div className="dialog-icon">
            <CircleDashed aria-hidden="true" />
          </div>
          <AlertDialog.Title>{confirmation?.title}</AlertDialog.Title>
          <AlertDialog.Description className="confirm-dialog-description">
            {confirmation?.description}
          </AlertDialog.Description>
          <div className="dialog-actions">
            <AlertDialog.Cancel className="button button-secondary">
              取消
            </AlertDialog.Cancel>
            <AlertDialog.Action
              className={`button ${confirmation?.danger ? "button-danger" : "button-primary"}`}
              onClick={() => {
                const action = confirmation?.action;
                close();
                if (action) void action();
              }}
            >
              {confirmation?.confirmLabel}
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
function callErrorMessage(error: unknown) {
  if (typeof error === "string" && error.trim()) return error;
  if (error instanceof Error && error.message.trim()) return error.message;
  return "请检查本地来源目录和权限";
}

function shortSessionId(value: string) {
  const normalized = value.trim();
  return normalized ? normalized.slice(0, 12) : "未知";
}

async function copyTextToClipboard(value: string) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("clipboard unavailable");
}

function buildTaskPrompt(task: Task) {
  const sections = [
    "# 工作台任务上下文",
    "",
    "## 任务",
    `- 标题：${task.title}`,
    `- 目标：${task.objective || "请先澄清目标"}`,
    `- 状态：${task.status}`,
    `- 关联项目：${task.projects.map((project) => project.title).join("、") || "未关联项目"}`,
    task.recommended_skill ? `- 推荐 Skill：${task.recommended_skill}` : "",
    "",
    "## 执行步骤",
    task.steps || "请先分析任务并提出可执行步骤。",
    "",
    "## 来源或背景",
    task.source?.content || task.source?.uri || "暂无补充来源。",
    "",
    "## 执行要求",
    "1. 先复述你对目标和约束的理解。",
    "2. 开始修改前先给出简短计划，并指出需要确认的风险。",
    "3. 完成后记录修改内容、验证命令、结果和遗留问题。",
  ];
  return sections.join("\n");
}

function buildSkillOptimizationPrompt(variant: SkillVariant, goal: string) {
  const content = (variant.content || variant.body || "暂无 SKILL.md 内容").slice(0, 14000);
  return [
    "# 本机 Skill 优化任务",
    "",
    `- Skill：${variant.name}`,
    `- Agent：${variant.agent}`,
    `- 文件：${variant.path}`,
    "",
    "## 优化目标",
    goal.trim() || "检查触发条件、执行步骤和边界情况，提出可直接落地的优化建议。",
    "",
    "## 当前 SKILL.md",
    content,
    "",
    "## 执行要求",
    "1. 先指出当前 Skill 最影响可用性的具体问题。",
    "2. 给出保持原意的最小修改方案，优先改善触发条件、步骤、错误处理和示例。",
    "3. 在修改前说明计划；完成后列出修改内容、验证方式和仍需人工确认的风险。",
    "4. 不要自动发送或执行高风险操作，等待用户确认。",
  ].join("\n");
}

function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (part: number) => String(part).padStart(2, "0");
  return date.getFullYear() + "-" + pad(date.getMonth() + 1) + "-" + pad(date.getDate()) + " " + pad(date.getHours()) + ":" + pad(date.getMinutes()) + ":" + pad(date.getSeconds());
}

createRoot(document.getElementById("root")!).render(<App />);
