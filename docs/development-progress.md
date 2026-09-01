# 开发进度记录

> 产品：个人 AI 工作操作系统 MVP
> 关联文档：[MVP 需求文档](./mvp-prd.md) · [MVP 开发计划](./mvp-development-plan.md) · [UI 规范](./design/ui-spec.md)

## 当前状态

- 当前阶段：阶段 6 工作台沉浸式视觉与特效升级（Obsidian Nebula & Cyber Glow）
- 状态：已完成
- 最近更新：2026-08-25
- 当前目标：全面落地黑曜石深邃背景、双霓虹流光、玻璃拟态高光、鼠标聚光灯（Spotlight）与 SVG 拓扑能量流行动画。

## 阶段清单

- [x] 产品方向与 MVP 范围确认
- [x] PRD 与开发计划建立
- [x] 建立产品上下文源文件
- [x] 阶段 0：Codex / Claude / Cursor 启动方式技术探针（探针 UI 已接入）
- [x] 阶段 1：Task / Project / Source 数据模型（Agent Run 待接入）
- [x] 阶段 2：今日行动首页与任务详情（手动任务闭环）
- [ ] 阶段 3：AI 任务草稿与推荐
- [x] 阶段 4：Agent 启动与上下文注入（Cursor 工作区判定、Ctrl+N 新会话、Ctrl+V 填入、终端降级）
- [x] 阶段 5：结果回填与复盘（自动历史解析、结果草稿、任务复盘）
- [x] 阶段 6：全套沉浸式黑曜石与极光流光视觉与动效重构（已落地）

## 本轮工作

### 已完成

- 完成设计系统 Tokens 升级（`src/design-system.css`）：深渊黑曜石基底（`#09080e`）、双霓虹强调（星云紫 `--accent` + 赛博青 `--cyan`）、玻璃拟态高光与外发光漫射（Bloom）。
- 完成全局沉浸式样式重构（`src/styles.css`）：18s 动态星云弥散背景、HUD 科技点阵微网格、黑曜石玻璃拟态卡片（`backdrop-filter: blur(24px)`）、1px 镜面顶光反射（Specular Border）。
- 实现鼠标跟随聚光灯（Spotlight Glow）：在 `src/main.tsx` 中以 `requestAnimationFrame` 节流，并按悬停容器计算局部坐标，卡片/面板只在 hover 或 focus-within 时显示柔和光晕。
- 实现 SVG 拓扑图与关系图动态能量脉冲流动连线（`graph-flow-pulse` + `energyFlow` 动画），统一紫青数据流视觉。
- 升级流光边框（Border Beam）、发光状态徽章（`live` 极光绿 / 呼吸点脉冲）、HUD 发光小标题，并为激活导航和选中 Skill 接入边框流光。
- 增加 `prefers-reduced-motion` 降级：关闭星云、网格、Beam、拓扑脉冲和 Spotlight 动效，保留信息与键盘焦点反馈。
- 修复移动端视觉壳体的 flex 压缩：`760px` 以下改为侧栏与主内容上下堆叠，390px 视口下保持完整宽度与页面级滚动。

### 2026-08-25 · 截图回归修复

- 根据设置页和 Skill 列表截图定位并修复三项结构性样式缺失：补齐 `skills-library-grid` 双栏布局、`function-filter-strip` 胶囊筛选样式、`skill-command-row` 设置表单网格布局。
- 将 Skill 卡片描述改为左对齐、限 3 行，并补齐选中态；将启用复选框替换为可见的暗色主题控件。
- 页面切换时重置 `.page-scroll` 到顶部，避免设置页标题和首个面板被旧滚动位置裁切。
- 影响文件：`src/main.tsx`、`src/styles.css`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check`、`pnpm build` 通过；桌面浏览器回归确认 Skill 双栏、设置页首卡片位置和筛选条样式；390px 移动布局保持无横向溢出。
- 已知限制：项目无 Git 元数据，`git diff --check` 无法执行；构建保留既有 JS chunk 超过 500 kB 的提示。

### 2026-08-25 · 全页面布局审查

- 使用 `product-design` 路由到现有页面 audit；由于环境没有独立 `design-audit` 子技能，按同等审查标准直接检查截图标注区域、代码结构和运行态视口。
- 修复顶部操作区网格、Skill 详情标题与动作分组、来源元数据字段网格，并移除 Skill 详情内部纵向滚动造成的嵌套滚动。
- 验证：`pnpm check`、`pnpm build` 通过；运行态检查确认桌面双栏、顶部操作区和 390px 移动视口无横向溢出。
- 同步更新视觉规范 `docs/design/ui-spec.md`。
- 通过 `pnpm check` 与 `pnpm build` 验证。

### 待验证

- Windows 上 Codex、Claude、Cursor 的“打开应用并注入 Prompt”能力。
- 任务跨多个项目时的默认上下文选择策略。
- Agent Run 的日志、修改文件和测试结果自动回填方式。

## 验证记录

| 日期 | 命令/场景 | 结果 | 备注 |
|---|---|---|---|
| 2026-08-25 | pnpm check | 通过 | TypeScript 类型检查通过；本轮修改后复验通过 |
| 2026-08-25 | pnpm build | 通过 | Vite 生产构建通过；保留既有 chunk 体积提示 |
| 2026-08-25 | 人工视觉验收 | 通过 | 1280×720 桌面与 390×844 移动视口；确认背景层、空态、无横向溢出和 Spotlight 局部坐标 |
| 2026-08-06 | cargo check | 通过 | Rust 后端编译通过 |
| 2026-08-06 | cargo test | 通过 | 18 个测试全部通过 |

## 下一步

1. 在运行中的桌面端窗口里查看全套黑曜石星云流光与动效表现。
2. 新增 Agent Run 持久化模型。
3. 增加结果回填和执行历史。

## 变更日志

### 2026-08-06

- 建立开发进度文档。
- 完成 Task / Project / Source 数据模型。
- 完成今日行动首页和手动任务闭环。
- 建立项目 UI 规范，统一普通输入框、文本域和复选框的控件契约。
- 修复任务创建/更新调用中前端 camelCase 与 Rust snake_case 参数不一致的问题。
- 接入 `probe_agents` 只读命令，在设置页展示 Codex、Claude、Cursor 的本机可用状态。
- 建立 Agent 适配器探针结论与降级策略文档。
- 完成本机命令版本基线：Codex CLI 0.142.5、Claude Code 2.1.207；Cursor 已发现但版本参数无稳定输出。
- 任务卡接入启动确认框、Context Prompt 生成、剪贴板复制和白名单 Agent 启动命令。
- 修复 Windows 启动路径选择：优先使用 `codex.cmd`、`claude.cmd`、`cursor.cmd`，不再启动无扩展名 Unix shim。
- 修复 Windows `cmd /C start` 引号重解析：改用 PowerShell `Start-Process` 分离传递执行文件、参数和工作目录。
- 完成 Cursor 工作区启动链路：按已记录窗口句柄/标题决定 `--reuse-window` 或 `--new-window`。
- 完成 Cursor Agent 会话注入：定向激活目标窗口，发送 `Ctrl+I`、`Ctrl+N`、`Ctrl+V`，不自动发送 Prompt。
- 新增 `agent_runs` 持久化记录和 `cursor-agent` 终端降级状态。
- 验证 `cargo test` 19 项、`pnpm check`、`pnpm build` 全部通过；当前机器未发现 `cursor-agent` 命令。
- 本阶段新增：Agent Run 结果回填与历史闭环。
- 后端增加 Codex、Claude、Cursor 会话历史解析、任务匹配和结果保存接口。
- 前端增加任务详情执行记录抽屉、结果草稿编辑和手动刷新。
- 运行结果只更新 Agent Run，不会自动把任务标记为已完成。
- 后续调整：历史刷新只读取工作台明确发起且带 task_id、Prompt 的运行记录，不再创建全局孤立会话或待确认执行卡片。

### 2026-08-06 阶段 5 验证

- cargo test：22 项测试通过。
- cargo check：通过，保留既有 legacy 未使用代码警告。
- pnpm check：通过。
- 待完成：pnpm build 和三类 Agent 本地历史的手工验收。
- Baseline diff closure (2026-08-07): agent runs now capture a workspace fingerprint before launch. Refresh computes final `+` / `~` / `-` changes against that baseline, while intermediate touched paths are kept separately.
- Progress sync standard (2026-08-07): added a project-level rule requiring every completed task to update the progress log with scope, files, verification, known issues, and next steps. See `docs/development-progress-spec.md`.
### 2026-08-07 · 开发进度自动同步规范

- 目标：规定每次任务完成后自动更新项目进度文档，确保阶段状态、验证结果和下一步工作可追溯。
- 完成：新增 `docs/development-progress-spec.md`，定义任务开始前检查、完成判定、进度记录模板、专项文档同步规则、验证格式和最终回复自检；在项目 `AGENTS.md` 中加入强制执行入口。
- 影响文件：`AGENTS.md`、`docs/development-progress-spec.md`、`docs/development-progress.md`。
- 验证：检查规范文件和进度记录已写入；本次仅修改文档，没有代码变更，因此未运行构建测试。
- 已知问题：仓库当前未提供 Git 元数据，无法使用 `git diff --check` 检查变更；不影响文档内容。
- 下一步：后续每个任务结束前按该规范更新进度文档，并在最终回复中提供进度链接。
### 2026-08-07 · 阶段 6 验收计划启动
- 目标：从核心开发转入真实项目场景验收，验证 MVP 的完整使用闭环。
- 完成：新增 `docs/phase-6-acceptance-plan.md`，覆盖基础链路、Codex/Claude/Cursor 适配、结果回填、最终改动文件基线差异、连续使用、通过标准和退出条件。
- 影响文件：`docs/phase-6-acceptance-plan.md`、`docs/mvp-development-plan.md`、`docs/development-progress.md`。
- 验证：已检查计划与现有阶段 6 目标一致；本次仅建立验收计划，尚未执行真实项目手工验收。
- 已知问题：阶段状态为“计划中”，阶段 6 尚未完成；旧文档中的部分历史验证数量仍需在验收过程中校正。
- 下一步：执行第一轮基础链路和三类 Agent 启动验收，逐项填写场景结果和运行记录 ID。
### 2026-08-07 · 阶段 6 计划调整为工作台全模块闭环

- 目标：修正“阶段 6 只验收 Agent”的范围，补齐工作区、文件收集、收件箱、Skill 库和今日行动的产品闭环。
- 完成：盘点现有页面和后端能力，新增 `docs/workbench-module-completion-plan.md`；Agent 启动与结果验收调整为全模块计划中的子项。
- 影响文件：`docs/workbench-module-completion-plan.md`、`docs/mvp-development-plan.md`、`docs/development-progress.md`。
- 验证：已通过代码和文档检索确认当前任务模块最完整，其他模块存在基础页面/底层能力但缺少完整采集、整理和关联闭环；本次未修改业务代码。
- 已知问题：文件收集目前以本地目录扫描为主；网页、GitHub 和 AI 对话来源尚未形成统一快速捕获入口；Skill 推荐与草稿审核仍需补齐。
- 下一步：先实现 P0 工作区、来源、收件箱与任务之间的闭环，再进行 Agent 子计划验收。
### 2026-08-07 · 阶段 6 重新校准为工作台全面改版

- 目标：恢复最初“个人 AI 工作台全面改版”的产品方向，不把当前侧边栏菜单当作最终信息架构。
- 完成：新增 `docs/design/workbench-redesign.md`，定义工作台首页、统一捕获、项目上下文、任务执行控制台、资源层和目标导航；将现有菜单标记为过渡实现。
- 影响文件：`docs/design/workbench-redesign.md`、`docs/design/context.md`、`docs/mvp-development-plan.md`、`docs/workbench-module-completion-plan.md`、`docs/development-progress.md`。
- 验证：已对照原始产品上下文和当前 `src/main.tsx` 导航实现，确认当前菜单与全面改版目标存在偏差；本次只调整规划和设计文档，未修改业务代码。
- 已知问题：目标架构尚未进入 UI 原型和代码重构阶段；当前任务页面仍是主要可用模块。
- 下一步：执行阶段 A，先产出全面改版的工作台外壳、统一捕获和项目上下文交互原型，再开始阶段 B 的 UI 重构。
### 2026-08-07 · 阶段 A 第一版工作台外壳
- 目标：将现有任务页推进为工作台驾驶舱雏形，开始落实全面改版的主导航、统一捕获和上下文状态展示。
- 完成：主导航收敛为工作台、项目上下文、执行记录和设置；新增全局“快速捕获”入口；首页新增待整理来源和执行脉冲区域；捕获内容支持选择项目并进入 inbox；旧笔记创建默认行为保持不变。
- 影响文件：`src/main.tsx`、`src/styles.css`、`src-tauri/src/lib.rs`、`docs/design/workbench-redesign.md`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；`cargo fmt --check` 通过；`cargo check` 通过；`cargo test` 33 项通过。保留既有 Rust unused/dead_code 警告和 Vite chunk 体积提示。
- 已知问题：统一捕获当前先复用 note 数据结构，网页/GitHub/AI 对话的专用元数据尚未拆分；首页执行脉冲目前为展示入口，尚未实现独立执行视图跳转。
- 下一步：继续完成阶段 A 的交互原型，补齐捕获草稿数据结构和项目上下文入口，再进入工作台外壳细节重构。
### 2026-08-07 · 阶段 A Source Draft 数据结构与转任务闭环

- 目标：让统一捕获的来源草稿可追踪、可归档，并能进入任务执行流。
- 完成：`knowledge_items` 增加 `capture_kind` 与 `source_uri`，旧数据库启动时兼容迁移；首页来源队列显示来源类型；来源详情显示原始链接；待整理来源可按关联项目生成任务，任务保存来源标题、原文和链接，成功后自动归档来源。
- 影响文件：`src-tauri/src/lib.rs`、`src/main.tsx`、`src/styles.css`。
- 验证：`cargo fmt --check`、`cargo check`、`cargo test`（33 项通过）、`pnpm check`、`pnpm build` 均通过。
- 已知问题：当前生成任务仍需关联项目；网页/GitHub 来源尚未接入浏览器扩展或剪贴板自动采集；保留既有 Rust unused/dead_code 与 Vite chunk 体积警告。
- 下一步：继续完成项目上下文的统一入口，补充来源草稿的编辑/忽略操作，再进入阶段 B 的页面组件和滚动条规范收敛。
### 2026-08-07 · 阶段 A 来源处理入口补齐

- 目标：让工作台首页的来源队列能够进入完整处理流程，而不是只能生成任务。
- 完成：首页增加“处理待整理来源”入口，复用现有收件箱详情的编辑、忽略、归档和项目归属能力；来源队列继续保留“生成任务”快捷动作。
- 影响文件：`src/main.tsx`。
- 验证：`pnpm check` 通过；此前 `cargo check`、`cargo test`（33 项）、`pnpm build` 已通过。
- 已知问题：来源详情仍在收件箱工作区中打开，尚未抽成新的统一侧滑面板；项目上下文页面仍处于过渡实现。
- 下一步：把来源详情、项目上下文和任务创建统一到工作台侧滑面板，再进入阶段 B 的组件和滚动条规范收敛。
### 2026-08-07 · 本机 Skill 资源视图保留

- 目标：在全面改版后仍能快速查找本机 Skill、查看用法和复用命令。
- 完成：侧边栏新增资源层，保留“本机 Skill”和“收件箱”入口；Skill 页面继续复用现有本机 Skill 库与详情能力；顶部标题在 Skill/收件箱视图中同步显示当前资源上下文；设计文档明确 Skill 资源视图不可被隐藏的任务上下文替代。
- 影响文件：`src/main.tsx`、`src/styles.css`、`docs/design/workbench-redesign.md`。
- 验证：待完成本轮 UI 变更后执行 `pnpm check`、`pnpm build` 和 Impeccable detector。
- 已知问题：Skill 页面仍是过渡实现，后续需要补充资源层搜索、用法分组和与项目上下文的关联入口。
- 下一步：统一 Skill 资源详情与任务上下文的关联动作，再收敛阶段 B 的组件规范。
### 2026-08-07 · Skill 资源层验证

- 验证：`pnpm check`、`pnpm build` 通过；Impeccable detector 未发现本轮新增问题。
- 保留告警：扫描仍报告旧样式中的两处 side-tab（Markdown 引用块、任务步骤块）左侧边框，本轮未修改其既有语义和范围。
### 2026-08-07 · 修复 Tauri WebView 中文乱码

- 原因：`index.html` 未声明 UTF-8 字符集；Tauri 本地 WebView 可能按 Windows 系统编码解析页面，导致 UTF-8 中文显示为乱码。
- 修复：补充 `<!doctype html>`、`lang="zh-CN"`、`<meta charset="UTF-8">`、viewport 和页面标题。
- 验证：`pnpm check`、`pnpm build` 通过；构建后的 `dist/index.html` 已包含 UTF-8 声明。
- 使用提示：需要完全退出旧 Tauri 进程并重新启动，让 WebView 加载新的构建产物。
### 2026-08-07 · 修复资源层新增中文文案乱码

- 原因：本轮新增资源层入口的两个 label 在写入时就使用了乱码文本，和 charset 问题无关。
- 修复：将“本机 Skill”“收件箱”以及对应顶部标题替换为真实 UTF-8 中文；同时扫描前端、Rust 和样式文件，未发现同类新增标记。
- 验证：源码标记扫描通过，待构建完成后确认 `pnpm check` 与 `pnpm build`。
### 2026-08-07 · 乱码修复验证完成

- `pnpm check`、`pnpm build` 通过。
- 构建前源码检查确认“本机 Skill”“收件箱”存在，`鏈満 Skill`、`鏀朵欢绠` 等错误入口文案已不存在。
- 重新启动应用时必须加载最新构建产物；若仍显示旧文案，应完全退出旧 Tauri 进程后再启动。
### 2026-08-10 · 工作台徽标排版修复
- 目标：修复工作台“待整理来源”和“执行记录”卡片右上角数字徽标中的文字未垂直/水平居中问题。
- 完成：为工作台表面卡片的 `.surface-count` 增加局部覆盖，清除通用标题 `span` 规则带来的底部间距和字距，并固定 flex 居中与行高。
- 影响文件：`src/styles.css`、`docs/development-progress.md`。
- 验证：本地桌面渲染检查通过；两个徽标均为 26×26、`display:flex`、水平/垂直居中；窄屏检查无横向溢出；`pnpm check`、`pnpm build` 均通过；Impeccable detector 仅报告项目原有两处 side-tab 规则。
- 已知问题：无新增；本地 WebView 因未连接 Tauri 后端会显示“读取失败”提示，不影响本次静态排版检查。
- 下一步：继续按阶段 6 计划进行 Tauri 真实场景验收。
### 2026-08-10 · 全局页面样式规范化修复

- 目标：在保留深色紫色 Work OS 视觉风格的前提下，统一共享样式、修复徽标与控件排版、改善移动端首屏布局，并清理可修复的厚侧边强调线。
- 完成：补充语义设计 Token 和统一焦点环；收窄面板标题辅助文字规则；规范 `.surface-count`、按钮、图标按钮、文本按钮、选择器和原生表单控件；为触控设备扩大图标按钮点击区域；压缩移动端侧栏和工作区列表高度；为长文本补充换行策略；将任务步骤、Markdown 引用、执行错误和变更警告改为轻量边界容器；精确化 reduced-motion 规则。
- 影响文件：`src/design-system.css`、`src/styles.css`、`src/main.tsx`、`docs/development-progress.md`。
- 验证：`pnpm check`、`pnpm build` 通过；Impeccable detector 返回空数组；六个导航页在 1440/1024/768/390 下无横向溢出；移动端主内容起点约 270px；快速捕获弹窗桌面/移动均无溢出；徽标为 26×26 且水平垂直居中。
- 已知问题：本地 Vite 浏览器未连接 Tauri runtime 时，`listen` 会抛出 `transformCallback` 错误并显示读取失败提示；生产 build 仍有已有的超过 500 kB chunk 警告；二者不由本次 CSS 改动引入。
- 下一步：在真实 Tauri 窗口完成 Agent/执行场景验收。
### 2026-08-13 · 本机 Skill 页面双重滚动条修复
- 目标：移除 Skill 详情打开时重复显示的内部纵向滚动条，保留页面最外侧滚动条。
- 完成：隐藏详情抽屉自身的可见滚动条，同时保留滚轮、触控板和键盘滚动能力；未改动其他页面或横向关系图滚动行为。
- 影响文件：`src/styles.css`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；Impeccable layout detector 返回空数组。
- 已知问题：生产构建保留既有的 JavaScript chunk 超过 500 kB 提示；本次未在真实 Tauri WebView 中进行截图复核。
- 下一步：在本机 Tauri 窗口打开长内容 Skill，确认页面仅显示最外侧纵向滚动条，并复核滚轮与键盘滚动。

### 2026-08-13 · 本机 Skill 详情卡片改为页面级滚动
- 目标：详情卡片不能继续作为隐藏滚动条的内部滚动容器，长内容应完整展开并由页面外层滚动。
- 完成：将打开状态的 Skill 详情定位到页面滚动容器的绝对层，保持浮层覆盖关系；移除 `overflow:auto` 的内部滚动，遮罩不拦截页面滚动。
- 影响文件：`src/styles.css`、`docs/development-progress.md`。
- 验证：`pnpm check`、`pnpm build` 通过；Impeccable layout detector 返回空数组。
- 已知问题：尚未在真实 Tauri WebView 中复核长内容滚动；生产构建保留既有的 JavaScript chunk 超过 500 kB 提示。
- 下一步：在本机窗口确认卡片内容全部展开、仅页面外层滚动。

### 2026-08-13 · Skill 详情浮层层级修复
- 目标：详情卡片应覆盖本机 Skill 页面，页面内容作为被遮罩的背景。
- 完成：详情卡片改为页面滚动容器内的绝对定位浮层并置于遮罩之上；仍由页面外层承载纵向滚动，不新增卡片内部滚动条。
- 影响文件：`src/styles.css`、`docs/development-progress.md`。
- 验证：`pnpm check`、`pnpm build` 通过；Impeccable layout detector 返回空数组。
- 已知问题：尚未在真实 Tauri WebView 中复核浮层层级与长内容滚动；生产构建保留既有的 JavaScript chunk 超过 500 kB 提示。
- 下一步：在本机窗口确认详情浮层覆盖背景且卡片内容可通过外层滚动查看。
### 2026-08-13 · 阶段 6 P0 工作区—来源—收件箱—任务闭环

- 完成 `task_sources.knowledge_item_id` 兼容迁移与索引；旧快照来源仍可读取。
- 新增来源转任务、关联已有任务的后端事务命令，支持重复操作幂等、单任务单来源约束与失败回滚。
- 删除工作区时清理 `task_projects` 关联，保留任务、来源和本地文件。
- 工作区详情返回最近任务摘要；收件箱详情提供“生成任务/关联已有任务”；任务详情展示来源标题、类型、URI、摘要和原条目状态。
- 验证：`cargo fmt --check`、`cargo check`、`cargo test`（37 项）、`pnpm check`、`pnpm build`、Impeccable layout detector 均通过；构建保留既有 chunk 体积提示。
- 后续：在真实 Tauri WebView 按验收清单验证重启、重复点击、删除工作区和 Agent Run 回看。

### 2026-08-21 · Skill 工作台简化
- 目标：将可见工作台从通用个人工作台收敛为本机 Skill 管理、调用历史、外部更新和客户端优化入口。
- 完成：默认入口改为本机 Skill；侧栏收敛为本机 Skill、调用历史、设置；移除 Skill 首屏功能拓扑图；保留自动功能分类与搜索；Skill 详情突出更新外部 Skill、打开目录和 Cursor/Codex 优化；完整正文、版本关系、使用笔记和复制能力移入折叠详情；新增 Skill 调用历史搜索与 Agent 筛选；命令面板改为只搜索 Skill 及 Skill 相关空间。
- 影响文件：`src/main.tsx`、`src/styles.css`、`docs/design/context.md`、`docs/design/workbench-redesign.md`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；Impeccable detector（`src/main.tsx`、`src/styles.css`）返回空数组；`git diff --check` 未执行成功，项目目录没有 Git 元数据。
- 已知问题：生产构建仍有既有 JavaScript chunk 超过 500 kB 的提示；尚未在真实 Tauri WebView 中完成 Cursor/Codex 启动和多尺寸截图验收。
- 下一步：在真实 Tauri 窗口验证 Skill 同步、更新命令实时输出、调用历史刷新，以及 Cursor/Codex Prompt 填入链路。

### 2026-08-25 · 全局星空主题切换系统
- 目标：为工作台接入四套深色星空主题，默认使用“创生之柱·秘境苍金”，并让所有页面、文本、控件和背景氛围跟随主题。
- 完成：新增主题模型、启动前主题恢复、localStorage 持久化、设置页主题卡片和顶栏快捷入口；建立四套 CSS Token；接入四张 `public` 背景图；将导航、面板、按钮、表单、Markdown、代码、SVG 图表、弹窗和 Agent 探针中的主题色迁移到语义 Token；状态颜色保持稳定。
- 影响文件：`src/theme.ts`、`src/main.tsx`、`src/design-system.css`、`src/styles.css`、`src/agent-probe.css`、`index.html`、`docs/design/ui-spec.md`、`docs/design/context.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；Impeccable detector 仅报告项目既有的 side-tab 警告和 HUD 网格建议；四张背景图已复制到 `dist` 根目录；旧主题紫色硬编码扫描无匹配。
- 已知问题：当前浏览器后端不可用，无法完成截图级桌面/窄屏视觉验收；真实 Tauri WebView 中的主题切换和键盘交互仍待复核；生产构建保留既有 JavaScript chunk 超过 500 kB 的提示。
- 下一步：在真实 Tauri 窗口验证四套主题的背景可读性、设置页/顶栏切换、刷新恢复、键盘焦点和窄屏布局。

### 2026-08-25 · 侧边栏透明度与主题浮层层级修复
- 目标：让侧边栏可透出当前主题背景，并修复顶栏主题浮层被 Skill 详情面板遮挡的问题。
- 完成：新增按主题变化的 `--sidebar-bg` 半透明 Token；降低侧边栏遮罩和模糊强度；将主题浮层挂载到 `document.body` 的独立 fixed 层，并按触发按钮位置实时定位，脱离页面滚动容器绘制。
- 影响文件：`src/main.tsx`、`src/design-system.css`、`src/styles.css`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；Impeccable detector 仅报告项目既有的 side-tab 警告和 HUD 网格建议。
- 已知问题：生产构建保留既有 JavaScript chunk 超过 500 kB 的提示；当前浏览器后端不可用，无法在本地浏览器自动截图确认最终层级。
- 下一步：在真实 Tauri 窗口确认四套主题下侧边栏背景可见，且主题浮层完整覆盖详情面板并支持滚动和键盘操作。

### 2026-08-26 · 背景氛围动效静态化
- 目标：移除背景上不协调的移动光效，保留星空背景和深色氛围层的静态视觉效果。
- 完成：停止雾状光晕呼吸、彩色光球漂移和装饰网格移动，背景图片继续作为固定沉浸层显示。
- 影响文件：`src/styles.css`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；背景动画关键帧引用扫描无残留。
- 已知问题：真实 Tauri WebView 截图级验收仍需复核；生产构建保留既有 JavaScript chunk 超过 500 kB 的提示。
- 下一步：在真实 Tauri 窗口确认四套主题下背景光效稳定、文字可读且无额外运动干扰。

### 2026-08-26 · 按钮顶部边框统一
- 目标：修复不同按钮类型顶部边框线亮度和来源不一致的问题。
- 完成：统一 `.button`、功能筛选按钮和来源视图切换按钮的顶部边框 Token；选中态保留主题强调顶边；移除主按钮独立的 inset 白色高光。
- 影响文件：`src/design-system.css`、`src/styles.css`、`docs/design/ui-spec.md`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；Impeccable detector 仅报告项目既有的 side-tab 警告和 HUD 网格建议。
- 已知问题：真实 Tauri WebView 截图级验收仍需复核；生产构建保留既有 JavaScript chunk 超过 500 kB 的提示。
- 下一步：在真实 Tauri 窗口检查主按钮、次按钮、筛选按钮和窄屏布局的顶部边框一致性。

### 2026-08-26 · 初始化 Git 忽略规则
- 目标：为当前 Git 仓库补充项目级 `.gitignore`，避免依赖、构建产物、Tauri 生成目录和本地日志进入版本控制。
- 完成：新增 `.gitignore`，覆盖 pnpm 依赖、Vite/Tauri 构建目录、Tauri 生成 schema、TypeScript 增量文件、环境变量、日志、操作系统和 IDE 临时文件；保留项目锁文件和源代码。
- 影响文件：`.gitignore`、`docs/development-progress.md`。
- 验证：`git status --short --branch` 确认忽略规则生效；未执行代码构建，因本次仅修改仓库配置。
- 已知问题：仓库当前仍没有初始提交，现有项目文件会继续显示为未跟踪，需后续人工确认后再提交。
- 下一步：检查未跟踪文件清单，确认需要纳入版本控制的项目文件后创建初始提交。

### 2026-09-01 · 复制到其他 Agent 交互布局修复
- 目标：修复“复制到其他 Agent”触发按钮位于详情底部、点击后复制表单跳到详情头部且样式散乱的问题。
- 完成：将复制入口移动到 Skill 详情动作区，使入口与优化、更新、打开目录保持同一操作层级；复制面板紧随动作区渲染；补齐目标 Agent 标签、确认按钮、关闭动作、成功结果和错误状态的布局与响应式样式；增加 `aria-expanded`、`aria-controls` 和状态播报。
- 影响文件：`src/main.tsx`、`src/styles.css`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；Impeccable detector 返回 2 条项目既有提示（折叠箭头侧边强调线、背景网格），未发现本轮新增问题；已检查桌面双栏与 760px 以下单列规则的 DOM/CSS 顺序。
- 已知问题：生产构建仍有既有 JavaScript chunk 超过 500 kB 提示；尚未在真实 Tauri WebView 中完成截图级点击验收。
- 下一步：在真实 Tauri 窗口打开 Skill 详情，确认复制入口、表单展开位置、目标选择和复制成功反馈的最终视觉表现。

### 2026-09-01 · 复制面板控件样式细化
- 目标：根据最新截图修复复制面板关闭按钮的浏览器默认灰底，以及目标 Agent 选择器的长文本折行和垂直对齐问题。
- 完成：关闭按钮改为透明主题文本按钮并补齐 hover/focus 状态；选择器保留目标路径信息但将已选值限制为单行省略；下拉项同步处理长路径截断；目标字段使用明确的可见标签和无障碍名称。
- 影响文件：`src/main.tsx`、`src/styles.css`、`src/design-system.css`、`docs/development-progress.md`。
- 验证：`pnpm check` 通过；`pnpm build` 通过；`git diff --check` 通过；Impeccable detector 返回 2 条项目既有提示，未发现本轮新增问题；已针对截图中的窄面板宽度检查关闭按钮、标签、选择器和确认按钮的布局规则。
- 已知问题：生产构建仍有既有 JavaScript chunk 超过 500 kB 提示；最终点击验收仍需在真实 Tauri WebView 中完成。
- 下一步：在真实 Tauri 窗口确认关闭按钮 hover/focus、选择器展开和复制成功状态的最终视觉表现。
