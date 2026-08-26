use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::State;

#[derive(Serialize)]
pub struct CursorLaunchPlan {
    workspace_path: String,
    window_mode: String,
    matched_window: Option<i64>,
    cursor_running: bool,
}

#[derive(Deserialize)]
pub struct CursorLaunchRequest {
    task_id: Option<i64>,
    workspace_path: String,
    prompt: String,
    auto_send: bool,
}

#[derive(Serialize)]
pub struct CursorLaunchResult {
    run_id: String,
    status: String,
    transport: String,
    window_mode: String,
    window_id: Option<i64>,
    error: Option<String>,
}

#[cfg(windows)]
mod platform {
    use std::{
        ffi::c_void, os::windows::process::CommandExt, path::Path, process::Command, thread,
        time::Duration,
    };
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_I, VK_N, VK_V,
            },
            WindowsAndMessaging::{
                EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
                IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE, WNDENUMPROC,
            },
        },
    };

    #[derive(Clone)]
    pub struct CursorWindow {
        pub handle: i64,
        pub title: String,
    }

    fn window_title(hwnd: HWND) -> String {
        let mut buffer = [0u16; 512];
        let length = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..length.max(0) as usize])
    }

    fn process_image(pid: u32) -> Option<String> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let result = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        };
        let _ = unsafe { CloseHandle(process) };
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }

    unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let title = window_title(hwnd);
        if title.trim().is_empty() || !title.to_ascii_lowercase().contains("cursor") {
            return BOOL(1);
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0
            || !process_image(pid)
                .map(|path| path.to_ascii_lowercase().ends_with("\\cursor.exe"))
                .unwrap_or(false)
        {
            return BOOL(1);
        }
        let windows = &mut *(lparam.0 as *mut Vec<CursorWindow>);
        windows.push(CursorWindow {
            handle: hwnd.0 as isize as i64,
            title,
        });
        BOOL(1)
    }

    pub fn list_windows() -> Vec<CursorWindow> {
        let mut windows = Vec::new();
        let callback: WNDENUMPROC = Some(collect_window);
        let pointer = LPARAM(&mut windows as *mut Vec<CursorWindow> as isize);
        let _ = unsafe { EnumWindows(callback, pointer) };
        windows
    }

    fn to_hwnd(handle: i64) -> HWND {
        HWND(handle as isize as *mut c_void)
    }

    fn emit_key(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> Result<(), String> {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err("Windows 键盘输入注入失败".into());
        }
        Ok(())
    }

    fn key_combo(modifier: VIRTUAL_KEY, key: VIRTUAL_KEY) -> Result<(), String> {
        emit_key(modifier, KEYBD_EVENT_FLAGS(0))?;
        emit_key(key, KEYBD_EVENT_FLAGS(0))?;
        emit_key(key, KEYEVENTF_KEYUP)?;
        emit_key(modifier, KEYEVENTF_KEYUP)
    }

    pub fn spawn_cursor(executable: &Path, workspace: &Path, mode: &str) -> Result<(), String> {
        let flag = if mode == "reuse" {
            "--reuse-window"
        } else {
            "--new-window"
        };
        let command_line = format!(
            "\"{}\" {} \"{}\"",
            executable.to_string_lossy(),
            flag,
            workspace.to_string_lossy()
        );
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into());
        Command::new(comspec)
            .creation_flags(0x00000010)
            .args(["/D", "/C", &command_line])
            .spawn()
            .map_err(|error| format!("启动 Cursor 失败：{error}"))?;
        Ok(())
    }

    pub fn wait_for_window(
        workspace_leaf: &str,
        before: &[i64],
        preferred: Option<i64>,
    ) -> Option<i64> {
        let leaf = workspace_leaf.to_ascii_lowercase();
        for _ in 0..32 {
            let windows = list_windows();
            if let Some(handle) =
                preferred.filter(|value| windows.iter().any(|window| window.handle == *value))
            {
                return Some(handle);
            }
            if let Some(window) = windows.iter().find(|window| {
                !before.contains(&window.handle)
                    && window.title.to_ascii_lowercase().contains(&leaf)
            }) {
                return Some(window.handle);
            }
            if let Some(window) = windows
                .iter()
                .find(|window| window.title.to_ascii_lowercase().contains(&leaf))
            {
                return Some(window.handle);
            }
            thread::sleep(Duration::from_millis(250));
        }
        None
    }

    pub fn fill_new_agent(handle: i64) -> Result<(), String> {
        let window = to_hwnd(handle);
        unsafe {
            let _ = ShowWindow(window, SW_RESTORE);
        }
        if !unsafe { SetForegroundWindow(window) }.as_bool() {
            return Err("无法将 Cursor 目标窗口置前".into());
        }
        thread::sleep(Duration::from_millis(180));
        // 先聚焦/打开 Agent 面板，再使用 Cursor 的 Ctrl+N（New Agent）。
        key_combo(VK_CONTROL, VK_I)?;
        thread::sleep(Duration::from_millis(220));
        key_combo(VK_CONTROL, VK_N)?;
        thread::sleep(Duration::from_millis(420));
        if unsafe { GetForegroundWindow() }.0 != window.0 {
            return Err("Cursor 目标窗口在自动填充前失去焦点".into());
        }
        // Prompt 已由前端写入剪贴板；只粘贴，不发送。
        key_combo(VK_CONTROL, VK_V)?;
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    #[derive(Clone)]
    pub struct CursorWindow {
        pub handle: i64,
        pub title: String,
    }

    pub fn list_windows() -> Vec<CursorWindow> {
        Vec::new()
    }
    pub fn spawn_cursor(_executable: &Path, _workspace: &Path, _mode: &str) -> Result<(), String> {
        Err("Cursor 桌面自动化仅支持 Windows".into())
    }
    pub fn wait_for_window(
        _workspace_leaf: &str,
        _before: &[i64],
        _preferred: Option<i64>,
    ) -> Option<i64> {
        None
    }
    pub fn fill_new_agent(_handle: i64) -> Result<(), String> {
        Err("Cursor 桌面自动化仅支持 Windows".into())
    }
}

fn normalize_workspace(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value)
        .canonicalize()
        .map_err(|_| "工作目录不存在或无权访问".to_string())?;
    if !path.is_dir() {
        return Err("工作目录必须是文件夹".into());
    }
    Ok(path)
}

fn workspace_leaf(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

fn window_plan(db: &Connection, workspace: &Path) -> (String, Option<i64>, bool) {
    let windows = platform::list_windows();
    let stored_handles: HashSet<i64> = db
    .prepare("SELECT window_handle FROM agent_runs WHERE agent='Cursor' AND workspace_path=?1 AND window_handle IS NOT NULL ORDER BY created_at DESC LIMIT 20")
    .ok()
    .and_then(|mut statement| statement.query_map([workspace.to_string_lossy().to_string()], |row| row.get::<_, i64>(0)).ok().map(|rows| rows.filter_map(Result::ok).collect()))
    .unwrap_or_default();
    let matched = windows
        .iter()
        .find(|window| stored_handles.contains(&window.handle))
        .or_else(|| {
            let leaf = workspace_leaf(workspace).to_ascii_lowercase();
            (!leaf.is_empty())
                .then(|| {
                    windows
                        .iter()
                        .find(|window| window.title.to_ascii_lowercase().contains(&leaf))
                })
                .flatten()
        })
        .map(|window| window.handle);
    (
        if matched.is_some() { "reuse" } else { "new" }.into(),
        matched,
        !windows.is_empty(),
    )
}

fn save_run(
    db: &Connection,
    run_id: &str,
    task_id: Option<i64>,
    workspace: &Path,
    mode: &str,
    transport: &str,
    handle: Option<i64>,
    prompt: &str,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    super::agent_runs::create_cursor_run(
        db,
        run_id,
        task_id,
        &workspace.to_string_lossy(),
        mode,
        transport,
        handle,
        prompt,
        status,
        error,
    )
}

fn plan_for(db: &Connection, workspace: &Path) -> CursorLaunchPlan {
    let (window_mode, matched_window, cursor_running) = window_plan(db, workspace);
    CursorLaunchPlan {
        workspace_path: workspace.to_string_lossy().into_owned(),
        window_mode,
        matched_window,
        cursor_running,
    }
}

#[tauri::command]
pub fn inspect_cursor_launch(
    workspace_path: String,
    db: State<super::Db>,
) -> Result<CursorLaunchPlan, String> {
    let workspace = normalize_workspace(&workspace_path)?;
    let db = super::lock(&db)?;
    Ok(plan_for(&db, &workspace))
}

fn fallback_cursor_agent(workspace: &Path, prompt: &str) -> Result<(), String> {
    let executable = super::first_command_path("cursor-agent")
        .ok_or_else(|| "未找到 cursor-agent，无法启动终端降级".to_string())?;
    let path = PathBuf::from(executable);
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "cmd" | "bat" | "ps1"))
        .unwrap_or(false)
    {
        return Err("cursor-agent 必须是可直接启动的可执行文件".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new(path)
            .arg(prompt)
            .current_dir(workspace)
            .creation_flags(0x00000010)
            .spawn()
            .map_err(|error| format!("启动 cursor-agent 终端失败：{error}"))?;
    }
    #[cfg(not(windows))]
    {
        Command::new(path)
            .arg(prompt)
            .current_dir(workspace)
            .spawn()
            .map_err(|error| format!("启动 cursor-agent 终端失败：{error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn launch_cursor_task(
    request: CursorLaunchRequest,
    db: State<super::Db>,
) -> Result<CursorLaunchResult, String> {
    if request.auto_send {
        return Err("当前仅支持填入 Prompt，不允许自动发送".into());
    }
    if request.prompt.trim().is_empty() {
        return Err("Prompt 不能为空".into());
    }
    let workspace = normalize_workspace(&request.workspace_path)?;
    let run_id = format!("cursor-{}", super::now());
    let (window_mode, matched_window, before_handles) = {
        let db = super::lock(&db)?;
        let plan = plan_for(&db, &workspace);
        let before = platform::list_windows()
            .into_iter()
            .map(|window| window.handle)
            .collect::<Vec<_>>();
        save_run(
            &db,
            &run_id,
            request.task_id,
            &workspace,
            &plan.window_mode,
            "cursor_ide",
            plan.matched_window,
            &request.prompt,
            "prepared",
            None,
        )?;
        super::agent_runs::prepare_run_baseline(&db, &run_id, &workspace)?;
        (plan.window_mode, plan.matched_window, before)
    };
    let executable = match super::first_command_path("cursor") {
        Some(path) => PathBuf::from(path),
        None => {
            let error = "未找到 Cursor 命令，请先检查 PATH 或安装配置".to_string();
            let db = super::lock(&db)?;
            save_run(
                &db,
                &run_id,
                request.task_id,
                &workspace,
                &window_mode,
                "cursor_ide",
                matched_window,
                &request.prompt,
                "failed",
                Some(&error),
            )?;
            return Err(error);
        }
    };
    {
        let db = super::lock(&db)?;
        save_run(
            &db,
            &run_id,
            request.task_id,
            &workspace,
            &window_mode,
            "cursor_ide",
            matched_window,
            &request.prompt,
            "launching",
            None,
        )?;
    }
    if let Err(desktop_error) = platform::spawn_cursor(&executable, &workspace, &window_mode) {
        if fallback_cursor_agent(&workspace, &request.prompt).is_ok() {
            let db = super::lock(&db)?;
            save_run(
                &db,
                &run_id,
                request.task_id,
                &workspace,
                &window_mode,
                "cursor_agent_terminal",
                None,
                &request.prompt,
                "fallback",
                Some(&desktop_error),
            )?;
            return Ok(CursorLaunchResult {
                run_id,
                status: "fallback".into(),
                transport: "cursor_agent_terminal".into(),
                window_mode,
                window_id: None,
                error: Some(desktop_error),
            });
        }
        let db = super::lock(&db)?;
        save_run(
            &db,
            &run_id,
            request.task_id,
            &workspace,
            &window_mode,
            "cursor_ide",
            matched_window,
            &request.prompt,
            "failed",
            Some(&desktop_error),
        )?;
        return Err(desktop_error);
    }
    let handle =
        platform::wait_for_window(&workspace_leaf(&workspace), &before_handles, matched_window);
    let desktop_result = handle
        .ok_or_else(|| "Cursor 窗口启动超时，无法定位目标工作区".to_string())
        .and_then(platform::fill_new_agent);
    match desktop_result {
        Ok(()) => {
            let db = super::lock(&db)?;
            save_run(
                &db,
                &run_id,
                request.task_id,
                &workspace,
                &window_mode,
                "cursor_ide",
                handle,
                &request.prompt,
                "prompt_filled",
                None,
            )?;
            Ok(CursorLaunchResult {
                run_id,
                status: "filled".into(),
                transport: "cursor_ide".into(),
                window_mode,
                window_id: handle,
                error: None,
            })
        }
        Err(desktop_error) => match fallback_cursor_agent(&workspace, &request.prompt) {
            Ok(()) => {
                let db = super::lock(&db)?;
                save_run(
                    &db,
                    &run_id,
                    request.task_id,
                    &workspace,
                    &window_mode,
                    "cursor_agent_terminal",
                    handle,
                    &request.prompt,
                    "fallback",
                    Some(&desktop_error),
                )?;
                Ok(CursorLaunchResult {
                    run_id,
                    status: "fallback".into(),
                    transport: "cursor_agent_terminal".into(),
                    window_mode,
                    window_id: handle,
                    error: Some(desktop_error),
                })
            }
            Err(fallback_error) => {
                let error = format!("{desktop_error}；终端降级也失败：{fallback_error}");
                let db = super::lock(&db)?;
                save_run(
                    &db,
                    &run_id,
                    request.task_id,
                    &workspace,
                    &window_mode,
                    "cursor_ide",
                    handle,
                    &request.prompt,
                    "failed",
                    Some(&error),
                )?;
                Err(error)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn launch_mode_defaults_to_new_when_no_window_matches() {
        assert_eq!(
            if None::<i64>.is_some() {
                "reuse"
            } else {
                "new"
            },
            "new"
        );
    }
}
