# Agent Skill Workbench 项目指令

## 项目范围

这是一个 Tauri 2 + React + TypeScript + Rust 的个人 AI 工作台。
前端位于 `src/`，Tauri/Rust 后端位于 `src-tauri/`，产品与设计文档位于 `docs/`。

## 通用开发要求

- 修改前先阅读相关代码和文档，不覆盖用户已有改动。
- 保持改动范围与当前任务一致，优先使用现有组件、Token 和工具链。
- 前端修改后运行 `pnpm check` 和 `pnpm build`。
- Rust 修改后运行 `cargo check`；涉及逻辑时运行 `cargo test`。
- 不要为了隐藏问题删除警告、测试或历史数据。
- 使用 `apply_patch` 编辑项目文件。

## UI 开发规范（强制）

当任务涉及页面、组件、布局、CSS、交互、弹窗、表单、滚动条或视觉调整时：

1. 修改前必须阅读 `docs/design/ui-spec.md`。
2. 同时参考 `docs/design/context.md`，确保实现符合产品定位和交互原则。
3. 颜色、间距、圆角、控件高度、字体和状态样式优先使用 `src/design-system.css` 中的 Token 和共享类。
4. 普通 `input`、`textarea`、`select`、checkbox 不得使用浏览器默认样式；应接入共享控件样式。
5. 弹窗、表单和任务执行结果必须保持信息层级、可读性、键盘访问和明确反馈。
6. UI 修改完成后检查桌面宽度、窄屏布局、滚动容器和焦点状态。

## 文档同步

- UI 规范变更同步更新 `docs/design/ui-spec.md`。
- 产品方向或交互原则变更同步更新 `docs/design/context.md`。
- 开发阶段、验收结果和已知限制同步更新 `docs/development-progress.md`。
- Progress synchronization is mandatory: before the final response, read `docs/development-progress-spec.md` and update `docs/development-progress.md` with the task goal, completed work, affected files, verification results, known issues, and next step. Work that is unverified or blocked must not be marked complete. Sync the relevant UI, product, requirements, or Agent adapter document when the task changes that area.
