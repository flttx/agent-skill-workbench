# Agent 适配器技术探针

> 阶段：0 · 更新日期：2026-08-06

## 探针目标

确认工作台在 Windows 本机能否发现 Codex、Claude、Cursor，并定义后续启动与 Prompt 传递的安全边界。

## 本机基线结果

| Agent | PATH 命令 | 当前结果 | 结论 |
|---|---|---|---|
| Codex | `codex` | 使用 `codex.cmd`；`codex-cli 0.142.5` | 可进入 CLI 启动参数验证 |
| Claude | `claude` | 使用 `claude.cmd`；`2.1.207` | 可进入 CLI 启动参数验证 |
| Cursor | `cursor` | 使用 `cursor.cmd`；桌面进程存在；版本参数无稳定输出 | 先按 CLI/桌面启动参数待验证处理 |

应用内通过 Windows `where.exe` 做只读探针，不执行 Agent 命令、不读取会话内容，也不自动发送 Prompt。

当前已实现 `launch_agent`：只接受 Codex、Claude、Cursor 白名单，按探针找到的路径打开新终端/应用，并可传入已验证的项目工作目录。Prompt 由前端在确认框中预览后复制到剪贴板，启动命令本身不接收 Prompt 参数。

Windows 的 `where.exe` 会同时返回 Node/Unix shim 和 Windows 入口；探针优先选择 `.cmd`、`.exe`、`.bat`、`.ps1`，避免把无扩展名 shim 交给 `start` 导致系统尝试打开 `\\`。

启动实现使用 PowerShell `Start-Process` 传递结构化参数，不再把整段命令交给 `cmd.exe /C start` 做二次引号解析；含空格的 Cursor 安装路径也会单独保留参数引号。

Cursor 专用链路已接入 `inspect_cursor_launch` 和 `launch_cursor_task`：工作目录先规范化，随后根据 `agent_runs` 中仍存活的窗口句柄或 Cursor 窗口标题决定 `--reuse-window` / `--new-window`。窗口就绪后定向激活目标窗口，发送 `Ctrl+I` 聚焦 Agent 面板、`Ctrl+N` 新建会话和 `Ctrl+V` 粘贴 Prompt，不发送 Enter。桌面自动化失败时尝试启动 `cursor-agent` 可见终端；本机当前未发现该命令，因此降级需要先安装 Cursor Agent CLI。

## 当前适配器契约

后续每个 Agent 适配器都应提供以下能力：

```text
canLaunch()       → 是否存在可用命令或应用路径
diagnose()        → 版本、路径、工作目录和失败原因
buildPrompt()     → 根据 Task/Project/Source/Skill 生成上下文 Prompt
launch()          → 打开工具并设置工作目录；发送前保留人工确认
fallback()        → 复制完整 Prompt，允许用户手动粘贴执行
```

## MVP 降级策略

1. 找到命令：显示“可用”，允许进入启动参数验证。
2. 找不到命令：显示“需配置”，不自动猜测安装目录。
3. 无法自动注入 Prompt：复制完整 Prompt，并保留任务上下文。
4. 自动发送 Prompt：暂不启用，必须在后续阶段逐个 Agent 验证并增加确认步骤。

## 下一步探针

- 记录三个命令的版本输出。
- 验证工作目录参数是否生效。
- 验证 Prompt 作为参数、标准输入或剪贴板传递的行为。
- 将探针结果映射为 `AgentAdapter` 配置，并创建 Agent Run。
- 在运行中的 Tauri 窗口里分别点击三个 Agent，确认应用打开、工作目录生效且剪贴板内容完整。
- Agent Run 结果闭环已接入：Codex、Claude、Cursor 使用统一的启动记录和本地历史解析接口。解析结果只作为可编辑草稿保存；工作区、时间或 Prompt 无法形成唯一匹配时保留在原任务运行记录中，不创建孤立会话。
- 新增接口：start_agent_run、list_agent_runs、refresh_agent_runs、save_agent_run_result、resolve_agent_run。Prompt 仍由前端复制或由 Cursor 桌面链路填入，默认不自动发送。
- 历史同步只针对工作台创建的任务运行记录，并要求 Agent、工作区、时间窗口和 Prompt 达到唯一匹配；不会为未关联的本地会话创建 Agent Run。
- Result reconciliation: each task run captures a workspace baseline before launch. Refresh compares current fingerprints against that baseline, reports final `+` / `~` / `-` paths, and keeps touched-but-reverted paths as intermediate files.
