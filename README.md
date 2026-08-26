# Agent Skill 工作台

本地 Windows 桌面应用：扫描 Codex、Claude、Cursor、Gemini 和 Agents 的 `SKILL.md`，保存个人使用卡片，并以只含元数据的方式记录可证实的 skill 使用事件。

```powershell
pnpm install
pnpm tauri dev
```

数据存放在 `%APPDATA%\AgentSkillWorkbench\workbench.sqlite`。应用只读 agent 目录；不会保存完整会话原文或修改 agent 配置。
