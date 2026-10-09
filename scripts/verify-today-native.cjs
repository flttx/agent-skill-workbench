// Real release WebView2 + Tauri IPC acceptance. Always uses isolated application/user data.
// PLAYWRIGHT_MODULE_PATH may point to the bundled Playwright runtime; no IPC is mocked.
// TAURI_QA_EXE may point to an acceptance-only build with additionalBrowserArgs enabling port 9223.
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const { spawn, execFileSync } = require("node:child_process");
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || "playwright");

async function main() {
  const executable = path.resolve(process.env.TAURI_QA_EXE || path.join(__dirname, "../src-tauri/target/release/agent-skill-workbench.exe"));
  const temporaryRoot = await fs.mkdtemp(path.join(os.tmpdir(), "workbench-native-"));
  const output = process.env.TAURI_QA_OUTPUT || path.join(temporaryRoot, "evidence");
  const projectPath = path.join(temporaryRoot, "project");
  const unavailablePath = path.join(temporaryRoot, "not-a-repository");
  const profile = path.join(temporaryRoot, "user");
  const environment = { ...process.env, APPDATA: path.join(temporaryRoot, "appdata"),
    LOCALAPPDATA: path.join(temporaryRoot, "local"), USERPROFILE: profile,
    CODEX_HOME: path.join(profile, ".codex"), WEBVIEW2_USER_DATA_FOLDER: path.join(temporaryRoot, "webview"),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--remote-debugging-port=9223" };
  for (const directory of [output, projectPath, unavailablePath, profile, environment.APPDATA, environment.LOCALAPPDATA]) {
    await fs.mkdir(directory, { recursive: true });
  }
  execFileSync("git", ["-C", projectPath, "init", "--quiet"], { windowsHide: true });
  const documentPath = path.join(projectPath, "PROGRESS.md");
  await fs.writeFile(documentPath, "# 验收项目\n- 当前阶段：真实阶段一\n- 已完成：准备环境\n## 下一步\n1. 核对原生保存\n", "utf8");
  const checks = [];
  const errors = [];
  let child;
  let browser;
  let page;
  let processLog = "";
  const invoke = (command, args = {}) => page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), { command, args });
  async function launch(env = environment) {
    child = spawn(executable, [], { env, cwd: path.dirname(executable), windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    process.stdout.write(`Native test process ${child.pid}; isolated root ${temporaryRoot}\n`);
    child.stdout.on("data", (data) => { processLog += String(data); });
    child.stderr.on("data", (data) => { processLog += String(data); });
    const deadline = Date.now() + 30_000;
    let connectionError;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Tauri exited (${child.exitCode}): ${processLog.slice(-2000)}`);
      try {
        browser = await chromium.connectOverCDP("http://127.0.0.1:9223", { timeout: 1500 });
        break;
      } catch (error) {
        connectionError = error;
        await new Promise((resolve) => setTimeout(resolve, 300));
      }
    }
    if (!browser) {
      const ownedChildren = execFileSync("powershell.exe", ["-NoProfile", "-Command", `Get-CimInstance Win32_Process | Where-Object ParentProcessId -eq ${child.pid} | Select-Object ProcessId,Name,CommandLine | ConvertTo-Json -Compress`], { windowsHide: true, encoding: "utf8" });
      await fs.writeFile(path.join(output, "launch-diagnostics.json"), ownedChildren || "[]");
      throw new Error(`WebView2 connection failed: ${connectionError}; diagnostics saved in ${output}`);
    }
    page = browser.contexts().flatMap((context) => context.pages())[0];
    assert.ok(page, "No native WebView2 page");
    page.on("pageerror", (error) => errors.push(error.message));
    await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === "function");
    await page.getByRole("heading", { name: "今日", exact: true }).waitFor();
    assert.match(page.url(), /tauri\.localhost|^tauri:\/\//);
  }
  async function stop() {
    if (browser) {
      await browser.close();
      browser = undefined;
    }
    if (child && child.exitCode === null) {
      execFileSync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], { windowsHide: true, stdio: "pipe" });
      await new Promise((resolve) => { if (child.exitCode !== null) resolve(); else child.once("exit", resolve); });
    }
    child = undefined;
  }
  try {
    await launch();
    const existing = await invoke("list_today_projects");
    assert.deepEqual(existing, [], "Isolation must start with an empty database");
    checks.push("release原生WebView启动、真实IPC与空数据库隔离");
    process.stdout.write("Native WebView connected; isolated database confirmed.\n");
    await page.getByRole("button", { name: "添加项目", exact: true }).click();
    await page.getByRole("textbox", { name: "项目名称", exact: true }).fill("原生验收项目");
    await page.getByRole("textbox", { name: "项目目录", exact: true }).fill(projectPath);
    await page.getByRole("button", { name: "添加并查看项目", exact: true }).click();
    await page.getByRole("heading", { name: "原生验收项目", exact: true }).waitFor();
    const project = (await invoke("list_today_projects")).find((item) => item.path === projectPath.replaceAll("\\", "/") || item.title === "原生验收项目");
    assert.ok(project, "UI project creation must persist through real IPC");
    await invoke("set_workspace_pinned", { id: project.id, pinned: true });
    checks.push("首次空首页通过添加表单创建真实项目");
    const unavailable = await invoke("save_workspace", { workspace: { title: "非Git验收项目", description: "隔离测试", path: unavailablePath, color: "violet" } });
    await page.reload();
    await page.getByRole("heading", { name: "原生验收项目", exact: true }).waitFor();
    const projects = await invoke("list_today_projects");
    assert.equal(projects.find((item) => item.id === project.id).git.state, "available");
    assert.equal(projects.find((item) => item.id === project.id).codex.state, "available");
    assert.equal(projects.find((item) => item.id === unavailable.id).git.state, "unavailable");
    await page.screenshot({ path: path.join(output, "native-home.png") });
    checks.push("真实项目/Git/Codex可用性及默认置顶项目");

    await page.getByRole("combobox", { name: "切换当前项目" }).click();
    await page.getByRole("option", { name: "非Git验收项目", exact: true }).click();
    assert.equal(await page.getByRole("button", { name: "生成进度草稿", exact: true }).isDisabled(), true);
    await page.getByRole("combobox", { name: "切换当前项目" }).click();
    await page.getByRole("option", { name: "原生验收项目", exact: true }).click();
    checks.push("原生项目切换及Git不可用只读");

    await page.locator(".today-document-settings > summary").click();
    await page.getByRole("textbox", { name: "文档相对路径" }).fill("PROGRESS.md");
    await page.getByRole("button", { name: "保存文档配置", exact: true }).click();
    await page.getByText("配置已保存，下次生成将读取这些文档。").waitFor();
    await assert.rejects(() => invoke("configure_progress_documents", { projectId: project.id, paths: ["../outside.md"] }));
    assert.deepEqual((await invoke("list_today_projects")).find((item) => item.id === project.id).progress_document_paths, ["PROGRESS.md"]);
    checks.push("文档配置真实持久化及越界拒绝不覆盖配置");

    await page.getByRole("button", { name: "生成进度草稿", exact: true }).click();
    const stage = page.getByRole("textbox", { name: "当前阶段", exact: true });
    await stage.waitFor();
    assert.equal(await stage.inputValue(), "真实阶段一");
    assert.equal(await page.getByRole("textbox", { name: "下一步", exact: true }).inputValue(), "核对原生保存");
    await stage.fill("用户确认的阶段");
    await page.getByRole("button", { name: "保存为新版本", exact: true }).click();
    await page.getByText("已保存进度版本 v1。").waitFor();
    let versions = await invoke("list_progress_versions", { projectId: project.id });
    assert.equal(versions.length, 1);
    assert.ok(versions[0].user_confirmed_fields.includes("current_stage"));
    checks.push("真实文档中文字段解析、编辑保存v1与确认字段保护");

    await fs.writeFile(documentPath, "# 验收项目\n- 当前阶段：真实阶段二\n- 已完成：准备环境\n## 下一步\n1. 核对原生保存\n", "utf8");
    await page.getByRole("button", { name: "生成进度草稿", exact: true }).click();
    await page.getByRole("button", { name: "当前阶段：采用新证据", exact: true }).waitFor();
    assert.equal(await stage.inputValue(), "用户确认的阶段");
    assert.equal(await page.getByRole("button", { name: "保存为新版本", exact: true }).isDisabled(), true);
    await page.getByRole("button", { name: "当前阶段：采用新证据", exact: true }).click();
    assert.equal(await stage.inputValue(), "真实阶段二");
    await page.screenshot({ path: path.join(output, "native-conflict.png") });
    await page.getByRole("button", { name: "保存为新版本", exact: true }).click();
    await page.getByText("已保存进度版本 v2。").waitFor();
    versions = await invoke("list_progress_versions", { projectId: project.id });
    assert.equal(versions.length, 2);
    assert.equal(versions.find((item) => item.version === 1).current_stage, "用户确认的阶段");
    assert.equal(versions[0].current_stage, "真实阶段二");
    checks.push("真实新证据冲突、选择新值、追加v2且v1不变");

    await page.getByRole("button", { name: "生成进度草稿", exact: true }).click();
    await stage.waitFor();
    const gitDirectory = path.resolve(projectPath, ".git");
    const disabledGit = path.resolve(projectPath, ".git-unavailable");
    assert.ok(gitDirectory.startsWith(temporaryRoot + path.sep) && disabledGit.startsWith(temporaryRoot + path.sep));
    await fs.rename(gitDirectory, disabledGit);
    await page.getByRole("button", { name: "保存为新版本", exact: true }).click();
    await page.getByText("保存失败，请刷新数据源状态后重试；草稿和已有版本已保留。").waitFor();
    assert.equal((await invoke("list_progress_versions", { projectId: project.id })).length, 2);
    await page.getByRole("button", { name: "刷新项目状态", exact: true }).click();
    await page.getByText("数据源不可用，草稿仅供查看。刷新状态后可继续编辑和保存。").waitFor();
    assert.equal(await stage.isDisabled(), true);
    await fs.rename(disabledGit, gitDirectory);
    await page.getByRole("button", { name: "刷新项目状态", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector("#progress-field-current_stage").disabled);
    await page.getByRole("button", { name: "保存为新版本", exact: true }).click();
    await page.getByText("已保存进度版本 v3。").waitFor();
    checks.push("生成后Git失效阻断保存、保留草稿与版本、恢复后保存v3");
    await stop();
    await launch();
    await page.getByRole("heading", { name: "原生验收项目", exact: true }).waitFor();
    versions = await invoke("list_progress_versions", { projectId: project.id });
    assert.equal(versions.length, 3);
    assert.equal(versions[0].version, 3);
    assert.deepEqual((await invoke("list_today_projects")).find((item) => item.id === project.id).progress_document_paths, ["PROGRESS.md"]);
    await page.screenshot({ path: path.join(output, "native-restarted.png") });
    checks.push("真实进程重启后项目、置顶、配置与三版本保留");
    assert.deepEqual(errors, []);
    checks.push("原生WebView未捕获异常为零");
    const report = { passed: checks.length, checks, executable, temporaryRoot, database: path.join(environment.APPDATA, "AgentSkillWorkbench/workbench.sqlite"), screenshots: output };
    await fs.writeFile(path.join(output, "native-evidence.json"), JSON.stringify(report, null, 2));
    process.stdout.write(JSON.stringify(report, null, 2) + "\n");
  } finally {
    await stop();
    await fs.writeFile(path.join(output, "process.log"), processLog);
  }
}

main().catch((error) => { process.stderr.write(String(error.stack || error) + "\n"); process.exitCode = 1; });
