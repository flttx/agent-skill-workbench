# 个人 AI 工作操作系统 MVP 开发计划

> 目标：交付并验证“想法 → 任务 → Agent → 结果”闭环。  
> 建议节奏：6 个阶段；每个阶段完成后可独立验收。  
> 计划基于当前 Tauri + React + Rust + SQLite 项目。

## 开发原则

- 先打通一个真实闭环，再扩展入口和集成数量。
- 不推翻已有 Skill 扫描、项目来源和本地 SQLite 能力，先重组信息架构。
- 外部 Agent 通过适配器隔离，不把 Codex/Claude/Cursor 逻辑写进任务核心。
- 所有 AI 自动化都保留人工确认点。
- 每阶段都要有可操作的验收场景。

## 阶段 0：技术探针与基线

### 目标

确认三个 Agent 在 Windows 上能否被打开、定位工作目录并接收 Prompt。

### 工作项

- 调查 Codex、Claude、Cursor 的可用 CLI、URL Scheme、命令行或应用启动方式。
- 统一定义 `AgentAdapter` 接口：`canLaunch`、`launch`、`buildPrompt`、`diagnose`。
- 明确“自动填入但不自动发送”在各 Agent 中的可行程度。
- 为适配器建立模拟实现，避免 UI 被外部工具阻塞。

### 产出

- 适配器技术结论。
- Agent 配置格式。
- 失败和降级策略。

### 验收

- 三种 Agent 至少都能给出明确的“可启动 / 不可启动 / 需要用户配置”状态。
- 在无法自动填入时，仍能生成一键复制的 Prompt。

## 阶段 1：数据模型与后端命令

### 目标

建立任务、上下文包和 Agent Run 的持久化基础。

### 工作项

- 新增 `tasks` 表：标题、目标、状态、优先级、项目关联、来源、推荐 Agent、推荐 Skill、创建/更新时间。
- 新增 `task_projects` 多对多关系表。
- 新增 `task_sources` 表，支持 URL、文本、文件路径和外部引用。
- 新增 `context_packs` 或任务快照结构。
- 新增 `agent_runs` 表：任务快照、Agent、状态、日志、结果、产物和验证信息。
- 新增 Rust command：任务 CRUD、项目关联、Run 创建/更新、结果保存。
- 保证已有 `projects`、`knowledge_items`、`skills` 数据可继续使用。

### 验收

- 可以通过命令创建任务、关联多个项目、创建 Run 并保存结果。
- 应用重启后数据不丢失。
- 删除项目不会删除任务正文和执行记录，只解除关联。

## 阶段 2：今日行动与任务详情

### 目标

把首页从知识库概览改成行动面板。

### 工作项

- 新增“今日行动”首页。
- 任务卡显示项目、来源、Agent、Skill、状态和最近结果。
- 新增任务快速输入入口。
- 新增任务详情页：目标、步骤、来源、项目、Agent、Skill、Context Pack。
- 支持用户确认/拒绝 AI 推荐。
- 支持任务状态、排序和项目筛选。

### 验收

- 用户可以在首页输入想法并保存为任务。
- 用户可以编辑任务，不需要进入设置页或知识库页。
- 用户能看到跨多个项目的任务列表。

## 阶段 3：AI 草稿与推荐

### 目标

让自然语言想法变成可执行任务。

### 工作项

- 定义任务草稿 JSON Schema。
- 实现标题、目标、步骤、项目、Agent、Skill 推荐。
- 实现推荐理由和置信度展示。
- 实现草稿预览、修改、确认和取消。
- 实现 Skill 草稿生成，但默认只进入审核状态。
- 设计无 AI 或调用失败时的手动编辑路径。

### 验收

- 输入一段模糊想法后，系统能生成结构化草稿。
- AI 结果不会自动覆盖已有任务或启动 Agent。
- 用户可以只采用部分建议。

## 阶段 4：Agent 启动与上下文注入

### 目标

从任务详情启动 Codex、Claude 或 Cursor。

### 工作项

- 实现三种 Agent Adapter。
- 实现 Context Pack 预览和编辑。
- 根据 Agent 生成对应 Prompt/启动参数。
- 支持打开应用、设置工作目录、写入 Prompt 或复制 Prompt。
- 创建 `AgentRun` 并实时更新启动状态。
- 增加配置页：Agent 路径、命令、工作目录和测试按钮。

### 验收

- 用户选择 Agent 后，应用能打开对应工具。
- 任务上下文完整进入 Prompt，且启动前可检查。
- 任意适配器失败时，用户能获得明确原因和可复制 Prompt。

## 阶段 5：结果回填与复盘

### 目标

让执行结果回到任务，而不是停留在外部 Agent 中。

### 工作项

- 新增 Run 详情：状态、时间、日志摘要、结果、文件、测试和产物。
- 支持手动粘贴 Agent 结果。
- 支持打开修改文件和产物路径。
- 支持任务完成、继续、阻塞和重试决策。
- 生成下一步建议，但需要用户确认。
- 将现有 Review/Activity 迁移为 Agent Run 视图。

### 验收

- 用户可以完整记录一次 Agent 执行结果。
- 失败不会丢失上下文和日志。
- 用户可以从历史 Run 重新生成任务或继续执行。

## 阶段 6：真实场景验证与收敛

### 目标

用真实项目验证 MVP 是否解决“工具太多、行动分散”的问题。

### 验证脚本

1. 输入一个与 Jupiter 相关的模糊想法。
2. 生成任务并关联项目。
3. 确认推荐 Agent 和 Skill。
4. 启动 Agent 并完成一次执行。
5. 保存结果、修改文件和测试信息。
6. 第二天从今日行动中继续该任务。

### 观测指标

- 从想法到可执行任务的时间 ≤ 3 分钟。
- 从任务到 Agent 启动的时间 ≤ 1 分钟。
- 任务上下文无需重复复制超过一次。
- 失败任务能够在 1 分钟内恢复上下文。
- 用户能在首页明确看到下一步，而不是先进入多个分类页面。

## 建议排期

| 阶段 | 建议时间 | 依赖 |
|---|---:|---|
| 阶段 0 技术探针 | 0.5–1 天 | 无 |
| 阶段 1 数据模型 | 1–2 天 | 阶段 0 结论可并行 |
| 阶段 2 今日行动 | 1–2 天 | 阶段 1 |
| 阶段 3 AI 草稿 | 1–2 天 | 阶段 1、2 |
| 阶段 4 Agent 启动 | 2–3 天 | 阶段 0、2、3 |
| 阶段 5 结果回填 | 1–2 天 | 阶段 4 |
| 阶段 6 场景验证 | 1 天 | 阶段 5 |

阶段 6 的 Agent 具体验收矩阵见 [`docs/phase-6-acceptance-plan.md`](./phase-6-acceptance-plan.md)；现状功能盘点见 [`docs/workbench-module-completion-plan.md`](./workbench-module-completion-plan.md)；全面改版路线见 [`docs/design/workbench-redesign.md`](./design/workbench-redesign.md)。

## 当前代码的迁移策略

### 保留

- Tauri 桌面壳。
- React 前端和现有动效系统。
- Rust + SQLite 本地持久化。
- Skill 扫描、Skill 库和来源目录扫描。
- 项目/工作区数据作为 Project 的基础。

### 重构

- `WorkspacePage` 的默认视图改为 Today/Tasks。
- `KnowledgePane` 从主流程改为 Source/Inbox 辅助页。
- `Review` 重构为 Agent Runs。
- 新增 Task、Context Pack、Agent Adapter 页面和命令。
- 将 Skill ID 手工输入改为可搜索选择器。

### 暂不动

- 浏览器扩展。
- GitHub 深度同步。
- 自动发送 Prompt。
- 全自动日志抓取和无人值守执行。

## 开发完成定义

当以下链路在 Windows 本地运行并通过真实项目验证时，MVP 完成：

```text
输入想法
→ 生成并确认任务
→ 关联多个项目
→ 预览 Context Pack
→ 打开 Codex/Claude/Cursor 并注入 Prompt
→ 创建 Agent Run
→ 保存结果、文件和验证信息
→ 在今日行动中继续或关闭任务
```
- 阶段 5 已实现 Agent Run 结果闭环：三类 Agent 启动记录统一落库，应用启动/手动刷新时解析本地历史，按 Agent、工作区、时间和 Prompt 进行保守匹配。
- 任务详情支持查看运行记录、编辑自动提取草稿、保存摘要/改动/验证/未解决问题；保存结果不会自动完成任务。
- 历史同步只处理工作台明确发起、带任务 ID 和 Prompt 的运行记录；无法唯一匹配时保留该任务运行记录，不创建孤立会话。
- Stage 5 refinement: final changed files are computed from a launch-time workspace baseline. Git projects and non-Git projects use ignored-file-aware fingerprints; history tool events are kept separately as intermediate evidence.
