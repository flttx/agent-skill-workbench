// Run against the local Vite server. PLAYWRIGHT_MODULE_PATH can point to a bundled runtime.
// Synthetic IPC fixtures verify the UI contract; this does not replace Tauri desktop acceptance.
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
const os = require("node:os");
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || "playwright");

async function main() {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_BROWSER_PATH });
  const output = process.env.TODAY_QA_OUTPUT || path.join(os.tmpdir(), "agent-workbench-today-qa");
  const checks = [];
  const errors = [];
  try {
    await fs.mkdir(output, { recursive: true });
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, reducedMotion: "reduce" });
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => {
      const now = "2026-10-08T08:00:00Z";
      const freshRun = new URLSearchParams(location.search).has("first_run");
      const projects = [1, 2].map((id) => ({
        id, title: id === 1 ? "甲项目" : "乙项目", path: id === 1 ? "E:/fixtures/alpha" : "E:/fixtures/beta",
        pinned: id === 1, updated_at: now, last_activity_at: now, current_stage: "测试阶段", next_action: "核对进度",
        last_completed: "已有任务", blocker_count: 0, codex: { state: "available", detail: "测试 CLI" },
        git: { state: "available", detail: "工作区干净" }, latest_progress_version: 1, progress_document_paths: [],
      }));
      const draftFor = (id) => ({
        project_id: id, project_title: projects.find((project) => project.id === id).title,
        current_stage: "用户确认阶段", last_completed: "已完成任务", next_action: "验证保存", blockers: "无",
        validation: "测试通过", workspace_changes: "工作区干净", generated_at: now,
        source_refs: [{ kind: "document", label: "docs/progress.md", detail: "E:/fixtures/docs/progress.md", at: now }],
        preserved_fields: freshRun ? [] : ["current_stage"],
        conflicts: freshRun ? [] : [{ field: "current_stage", confirmed_value: "用户确认阶段", suggested_value: "新的证据阶段" }],
      });
      if (freshRun) projects.length = 0;
      const store = { projects, failSave: false, failCreate: false, delayedCreate: false, pendingCreate: null,
        nextDirectory: "E:/fixtures/fresh", directoryFails: false,
        failRefresh: false, delayedDraft: false, pendingDraft: null, calls: [], versions: [] };
      store.versions = projects.map((project) => ({ ...draftFor(project.id), id: project.id, version: 1, created_at: now, user_confirmed_fields: ["current_stage"] }));
      window.__TODAY_QA__ = store;
      window.__TAURI_INTERNALS__ = {
        transformCallback: () => 1,
        unregisterCallback: () => {},
        invoke: async (command, args = {}) => {
          store.calls.push({ command, args });
          if (command === "list_today_projects") {
            if (store.failRefresh) throw new Error("synthetic refresh failure");
            return structuredClone(store.projects);
          }
          if (command === "list_skill_groups") return { groups: [], relations: [], function_groups: [], function_relations: [] };
          if (command === "get_sync_status") return { stage: "idle", state: "idle", detail: "", started_at: null, finished_at: null };
          if (command === "plugin:dialog|open") {
            if (store.directoryFails) throw new Error("synthetic native picker failure");
            return store.nextDirectory;
          }
          if (command === "save_workspace") {
            if (store.failCreate) throw new Error("synthetic raw database error must never be displayed");
            if (store.delayedCreate) await new Promise((resolve) => { store.pendingCreate = resolve; });
            const input = args.workspace;
            if (projects.some((project) => project.path === input.path)) throw new Error("synthetic duplicate path");
            const id = Math.max(0, ...projects.map((project) => project.id)) + 1;
            const project = { id, title: input.title, path: input.path, pinned: false, updated_at: now, last_activity_at: now,
              current_stage: "未记录", next_action: "记录下一步", last_completed: "暂无", blocker_count: 0,
              codex: { state: "available", detail: "测试 CLI" }, git: { state: "available", detail: "工作区干净" },
              latest_progress_version: null, progress_document_paths: [] };
            projects.push(project);
            return { ...project, description: input.description, color: input.color, last_opened_at: now, inbox_count: 0, knowledge_count: 0, source_count: 0 };
          }
          if (command === "list_progress_versions") return store.versions.filter((version) => version.project_id === args.projectId);
          if (command === "generate_progress_draft") {
            if (store.delayedDraft) return new Promise((resolve) => { store.pendingDraft = () => resolve(draftFor(args.projectId)); });
            return draftFor(args.projectId);
          }
          if (command === "configure_progress_documents") {
            store.projects.find((project) => project.id === args.projectId).progress_document_paths = args.paths;
            return args.paths;
          }
          if (command === "save_progress_version") {
            if (store.failSave) throw new Error("synthetic raw SQL error must never be displayed");
            const version = Math.max(0, ...store.versions.filter((item) => item.project_id === args.input.project_id).map((item) => item.version)) + 1;
            const saved = { ...args.input, id: 99, version, created_at: now };
            store.versions.push(saved);
            return saved;
          }
          if (command === "plugin:event|listen") return 1;
          return [];
        },
      };
    });
    await page.goto(process.env.TODAY_QA_URL || "http://127.0.0.1:1420/");
    await page.getByRole("heading", { name: "甲项目", exact: true }).waitFor();
    checks.push("今日缓存/项目首屏");

    const themeChecks = [];
    const themes = [["pillars", "创生之柱"], ["black-hole", "黑洞引力"], ["starweaver", "恒星编织者"], ["cosmic-cliffs", "宇宙山脉"]];
    for (const [theme, label] of themes) {
      await page.getByRole("button", { name: /^切换主题，当前为/ }).click();
      await page.getByRole("radio", { name: new RegExp(label) }).click();
      await page.waitForFunction((id) => document.documentElement.dataset.theme === id, theme);
      const contrast = await page.locator(".today-primary-action .button-primary").evaluate((button) => {
        const context = document.createElement("canvas").getContext("2d");
        const luminance = (color) => {
          context.clearRect(0, 0, 1, 1);
          context.fillStyle = color;
          context.fillRect(0, 0, 1, 1);
          const rgb = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map((value) => {
            const channel = value / 255;
            return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
          });
          return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        };
        const style = getComputedStyle(button);
        const fg = luminance(style.color), bg = luminance(style.backgroundColor);
        return (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);
      });
      assert.ok(contrast >= 4.5, `${theme} primary contrast ${contrast.toFixed(2)}`);
      for (const [width, height] of [[1180, 760], [900, 600], [760, 900], [390, 844], [320, 844]]) {
        await page.setViewportSize({ width, height });
        await page.evaluate(() => {
          document.querySelector(".page-scroll").scrollTop = 0;
          document.querySelector(".app-shell").scrollTop = 0;
        });
        await page.waitForTimeout(150);
        const layout = await page.locator(".today-primary-action button").evaluate((button) => {
          const rect = button.getBoundingClientRect();
          const project = document.querySelector(".today-card-heading").getBoundingClientRect();
          const ready = document.querySelector(".today-readiness-row").getBoundingClientRect();
          return { top: rect.top, bottom: rect.bottom, visible: rect.top >= 0 && rect.bottom <= innerHeight,
            projectVisible: project.top >= 0 && project.bottom <= innerHeight,
            statusVisible: ready.top >= 0 && ready.bottom <= innerHeight,
            overflow: document.documentElement.scrollWidth > innerWidth };
        });
        assert.equal(layout.overflow, false, `${theme}/${width} horizontal overflow`);
        assert.equal(layout.visible && layout.projectVisible && layout.statusVisible, true, `${theme}/${width} primary context below fold: ${JSON.stringify(layout)}`);
        await page.screenshot({ path: path.join(output, `home-${theme}-${width}.png`) });
        themeChecks.push({ theme, width, contrast: Number(contrast.toFixed(2)), ...layout });
      }
    }
    await page.setViewportSize({ width: 1280, height: 900 });
    checks.push("四主题1180/900/760/390/320px首屏项目、状态、主动作可见及按钮对比度达标");
    await page.getByRole("combobox", { name: "切换当前项目" }).click();
    await page.getByRole("option", { name: "乙项目", exact: true }).click();
    await page.getByRole("heading", { name: "乙项目", exact: true }).waitFor();
    await page.getByRole("combobox", { name: "切换当前项目" }).click();
    await page.getByRole("option", { name: "甲项目", exact: true }).click();
    checks.push("项目名旁切换入口可用");
    assert.equal(await page.getByRole("button", { name: "生成进度草稿", exact: true }).count(), 1);
    await page.getByRole("button", { name: /查看 Git 状态/ }).click();
    await page.locator("#today-git-status").waitFor();
    assert.match(await page.locator("#today-git-status").textContent(), /这是已读取的状态/);
    await page.getByRole("button", { name: /查看 Git 状态/ }).click();
    checks.push("唯一生成入口与Git就地反馈");

    const search = page.getByRole("button", { name: "搜索项目与 Skill" });
    await search.click();
    await page.getByRole("textbox", { name: "搜索项目与 Skill" }).fill("/beta");
    await page.getByRole("button", { name: /^乙项目 项目/ }).click();
    await page.getByRole("heading", { name: "乙项目", exact: true }).waitFor();
    assert.equal(await search.evaluate((element) => document.activeElement === element), true);
    checks.push("按项目路径搜索、切换上下文与焦点返回");
    await search.click();
    await page.getByRole("textbox", { name: "搜索项目与 Skill" }).fill("不存在的项目");
    await page.getByText("没有匹配的项目或 Skill，请尝试名称或目录。").waitFor();
    await page.keyboard.press("Escape");
    checks.push("搜索空状态与 Escape 关闭");
    await page.keyboard.press("Control+k");
    await page.getByRole("textbox", { name: "搜索项目与 Skill" }).fill("甲项目");
    await page.getByRole("button", { name: /^甲项目 项目/ }).click();
    checks.push("Ctrl+K 名称搜索");

    await page.locator(".today-document-settings > summary").click();
    await page.getByRole("textbox", { name: "文档相对路径" }).focus();
    await page.keyboard.press("Shift+Tab");
    assert.ok(await page.locator(".today-document-settings > summary").evaluate((element) => {
      const style = getComputedStyle(element);
      return parseFloat(style.outlineWidth) >= 2 && style.outlineStyle !== "none" && style.outlineColor !== "rgb(16, 16, 16)";
    }));
    checks.push("折叠入口主题键盘焦点可见");
    await page.getByRole("textbox", { name: "文档相对路径" }).fill("docs/progress.md");
    await page.getByRole("button", { name: "保存文档配置" }).click();
    await page.getByText("配置已保存，下次生成将读取这些文档。").waitFor();
    assert.deepEqual(await page.evaluate(() => window.__TODAY_QA__.projects[0].progress_document_paths), ["docs/progress.md"]);
    await page.getByRole("textbox", { name: "文档相对路径" }).fill("");
    await page.getByRole("button", { name: "保存文档配置" }).click();
    await page.waitForFunction(() => window.__TODAY_QA__.projects[0].progress_document_paths.length === 0);
    checks.push("文档配置保存与空配置恢复默认");

    await page.getByRole("button", { name: "生成进度草稿", exact: true }).click();
    const save = page.getByRole("button", { name: "保存为新版本", exact: true });
    await save.waitFor();
    assert.equal(await save.isDisabled(), true);
    await page.getByRole("button", { name: "当前阶段：保留确认值", exact: true }).click();
    assert.equal(await page.getByRole("textbox", { name: "当前阶段", exact: true }).inputValue(), "用户确认阶段");
    assert.equal(await save.isEnabled(), true);
    await page.getByRole("button", { name: "当前阶段：采用新证据", exact: true }).click();
    assert.equal(await page.getByRole("textbox", { name: "当前阶段", exact: true }).inputValue(), "新的证据阶段");
    checks.push("未处理冲突阻断保存、保留旧值、采用新证据");
    await page.evaluate(() => { window.__TODAY_QA__.failSave = true; });
    await save.click();
    await page.getByText("保存失败，请刷新数据源状态后重试；草稿和已有版本已保留。").waitFor();
    assert.equal(await page.getByRole("textbox", { name: "当前阶段", exact: true }).inputValue(), "新的证据阶段");
    assert.equal(await page.getByText("synthetic raw SQL error must never be displayed").count(), 0);
    checks.push("保存失败保留草稿、不暴露原始错误");

    await page.evaluate(() => { window.__TODAY_QA__.projects[0].git.state = "unavailable"; });
    await page.getByRole("button", { name: "刷新项目状态", exact: true }).click();
    await page.getByText("数据源不可用，草稿仅供查看。刷新状态后可继续编辑和保存。").waitFor();
    assert.equal(await save.isDisabled(), true);
    assert.equal(await page.getByRole("textbox", { name: "当前阶段", exact: true }).isDisabled(), true);
    await page.evaluate(() => { window.__TODAY_QA__.projects[0].git.state = "available"; });
    await page.getByRole("button", { name: "刷新项目状态", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector("#progress-field-current_stage").disabled);
    await page.evaluate(() => { window.__TODAY_QA__.failRefresh = true; });
    await page.getByRole("button", { name: "刷新项目状态", exact: true }).click();
    await page.getByText("刷新失败，已保留缓存和当前草稿；可稍后重试。").waitFor();
    assert.equal(await page.getByRole("textbox", { name: "当前阶段", exact: true }).inputValue(), "新的证据阶段");
    await page.evaluate(() => { window.__TODAY_QA__.failRefresh = false; });
    checks.push("数据源降级禁用编辑保存、恢复及刷新失败保留草稿");

    await page.locator(".today-draft-sources summary").click();
    await page.locator(".today-document-settings").screenshot({ path: path.join(output, "today-document-settings.png") });
    await page.locator(".today-draft-editor").screenshot({ path: path.join(output, "today-draft-desktop.png") });
    await page.screenshot({ path: path.join(output, "today-desktop.png"), fullPage: true });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    const horizontalOverflow = await page.locator(".today-view").evaluate((element) => element.scrollWidth > element.clientWidth);
    assert.equal(horizontalOverflow, false);
    await page.screenshot({ path: path.join(output, "today-mobile.png"), fullPage: true });
    await page.locator(".today-draft-editor").screenshot({ path: path.join(output, "today-draft-mobile.png") });
    checks.push("1280px/390px 布局与可展开来源，无横向溢出");
    await page.evaluate(() => { window.__TODAY_QA__.failSave = false; });
    await save.click();
    await page.getByText("已保存进度版本 v2。").waitFor();
    assert.equal(await page.evaluate(() => window.__TODAY_QA__.versions.filter((version) => version.project_id === 1).length), 2);
    assert.equal(await page.evaluate(() => window.__TODAY_QA__.versions.at(-1).user_confirmed_fields.includes("current_stage")), true);
    checks.push("新版本追加、旧版本保留、确认字段继续受保护");

    await page.evaluate(() => { window.__TODAY_QA__.delayedDraft = true; });
    await page.getByRole("button", { name: "生成进度草稿", exact: true }).click();
    await search.click();
    await page.getByRole("textbox", { name: "搜索项目与 Skill" }).fill("乙项目");
    await page.getByRole("button", { name: /^乙项目 项目/ }).click();
    await page.evaluate(() => window.__TODAY_QA__.pendingDraft());
    await page.waitForFunction(() => !document.querySelector(".today-primary-action button").disabled);
    assert.equal(await page.locator(".today-draft-editor").count(), 0);
    checks.push("切换项目后丢弃旧项目迟到草稿");

    await page.setViewportSize({ width: 1180, height: 760 });
    const firstRunUrl = new URL(process.env.TODAY_QA_URL || "http://127.0.0.1:1420/");
    firstRunUrl.searchParams.set("first_run", "1");
    await page.goto(firstRunUrl.toString());
    await page.getByRole("heading", { name: "还没有可用项目", exact: true }).waitFor();
    const addProject = page.getByRole("button", { name: "添加项目", exact: true });
    await addProject.click();
    const nameField = page.getByRole("textbox", { name: "项目名称", exact: true });
    const pathField = page.getByRole("textbox", { name: "项目目录", exact: true });
    const addSubmit = page.getByRole("button", { name: "添加并查看项目", exact: true });
    await nameField.waitFor();
    assert.equal(await nameField.evaluate((element) => document.activeElement === element), true);
    await addSubmit.click();
    assert.equal(await page.evaluate(() => window.__TODAY_QA__.calls.filter((call) => call.command === "save_workspace").length), 0);
    checks.push("空首页添加入口、弹窗首字段聚焦、必填阻断");
    await page.getByRole("button", { name: "选择目录", exact: true }).click();
    assert.equal(await pathField.inputValue(), "E:/fixtures/fresh");
    assert.equal(await nameField.inputValue(), "fresh");
    await page.evaluate(() => { window.__TODAY_QA__.nextDirectory = null; });
    await page.getByRole("button", { name: "选择目录", exact: true }).click();
    assert.equal(await pathField.inputValue(), "E:/fixtures/fresh");
    await page.evaluate(() => { window.__TODAY_QA__.directoryFails = true; });
    await page.getByRole("button", { name: "选择目录", exact: true }).click();
    await page.getByText("无法打开目录选择器，可以直接粘贴项目目录路径。").waitFor();
    assert.equal(await pathField.inputValue(), "E:/fixtures/fresh");
    checks.push("目录选择自动填名、取消保留路径、选择器失败允许粘贴");
    await nameField.fill("首次项目");
    await page.evaluate(() => { window.__TODAY_QA__.failCreate = true; });
    await addSubmit.click();
    await page.getByRole("alert").waitFor();
    assert.equal(await nameField.inputValue(), "首次项目");
    assert.equal(await pathField.inputValue(), "E:/fixtures/fresh");
    assert.equal(await page.getByText("synthetic raw database error must never be displayed").count(), 0);
    await page.screenshot({ path: path.join(output, "create-project-desktop.png") });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(output, "create-project-mobile.png") });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    checks.push("添加失败保留字段、错误友好、弹窗桌面与窄屏无溢出");
    await page.keyboard.press("Escape");
    await page.getByRole("alertdialog").waitFor({ state: "detached" });
    assert.equal(await addProject.evaluate((element) => document.activeElement === element), true);
    checks.push("取消创建返回空态触发按钮焦点");

    await addProject.click();
    await nameField.fill("首次项目");
    await pathField.fill("E:/fixtures/fresh");
    await page.evaluate(() => { window.__TODAY_QA__.failCreate = false; window.__TODAY_QA__.delayedCreate = true; });
    await page.locator(".workspace-composer-dialog form").evaluate((form) => { form.requestSubmit(); form.requestSubmit(); });
    await page.waitForFunction(() => typeof window.__TODAY_QA__.pendingCreate === "function");
    assert.equal(await page.evaluate(() => window.__TODAY_QA__.calls.filter((call) => call.command === "save_workspace").length), 2);
    assert.equal(await page.getByRole("button", { name: "添加中…", exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole("button", { name: "取消", exact: true }).isDisabled(), true);
    await page.keyboard.press("Escape");
    assert.equal(await page.getByRole("alertdialog").count(), 1);
    await page.evaluate(() => window.__TODAY_QA__.pendingCreate());
    await page.getByRole("heading", { name: "首次项目", exact: true }).waitFor();
    await page.getByRole("alertdialog").waitFor({ state: "detached" });
    assert.equal(await page.getByRole("combobox", { name: "切换当前项目" }).evaluate((element) => document.activeElement === element), true);
    checks.push("重复提交仅一次请求、保存中防取消、成功进入新项目并恢复焦点");
    await page.evaluate(() => { window.__TODAY_QA__.delayedCreate = false; });
    await page.getByRole("button", { name: "生成进度草稿", exact: true }).click();
    await page.getByRole("textbox", { name: "当前阶段", exact: true }).fill("首次创建后确认阶段");
    await page.getByRole("button", { name: "保存为新版本", exact: true }).click();
    await page.getByText("已保存进度版本 v1。").waitFor();
    assert.equal(await page.evaluate(() => window.__TODAY_QA__.versions[0].project_id), 1);
    checks.push("空首页到创建、生成、编辑、保存首版本闭环");

    await addProject.click();
    await nameField.fill("刷新待恢复项目");
    await pathField.fill("E:/fixtures/second");
    await page.evaluate(() => { window.__TODAY_QA__.failRefresh = true; });
    await addSubmit.click();
    await page.getByRole("alertdialog").waitFor({ state: "detached" });
    await page.getByText("项目已添加，状态暂未更新；请刷新项目状态，无需重复添加。").waitFor();
    assert.equal(await page.evaluate(() => window.__TODAY_QA__.projects.length), 2);
    await page.evaluate(() => { window.__TODAY_QA__.failRefresh = false; });
    await page.getByRole("button", { name: "刷新项目状态", exact: true }).click();
    await page.getByRole("heading", { name: "刷新待恢复项目", exact: true }).waitFor();
    checks.push("已有项目继续添加、保存成功但刷新失败不重复创建、刷新后切换新项目");
    assert.deepEqual(errors, []);
    checks.push("运行态无未捕获异常");
    await fs.writeFile(path.join(output, "layout-evidence.json"), JSON.stringify(themeChecks, null, 2));
    process.stdout.write(JSON.stringify({ passed: checks.length, checks, layouts: themeChecks.length,
      themes: [...new Map(themeChecks.map(({ theme, contrast }) => [theme, { theme, contrast }])).values()], screenshots: output }, null, 2) + "\n");
  } finally {
    await browser.close();
  }
}

main().catch((error) => { process.stderr.write(String(error.stack || error) + "\n"); process.exitCode = 1; });
