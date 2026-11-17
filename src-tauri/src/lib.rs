mod hub;
mod model;
mod process;
mod storage;
#[cfg(windows)]
mod windows_process;

use hub::{Hub, Shared, Snapshot};
use model::{Config, Preview, Task};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type Result<T> = std::result::Result<T, String>;
static EXIT_ALLOWED: AtomicBool = AtomicBool::new(false);
static EXIT_PENDING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn snapshot(state: tauri::State<Shared>) -> Snapshot {
    state.0.lock().unwrap().snapshot()
}

fn apply_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<()> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|e| format!("应用开机启动设置失败：{e}"))
}

#[tauri::command]
fn save_config(
    app: tauri::AppHandle,
    state: tauri::State<Shared>,
    config: Config,
    overwrite: bool,
) -> Result<()> {
    let mut hub = state.0.lock().unwrap();
    let old = hub.config.settings.launch_at_login;
    let next = config.settings.launch_at_login;
    hub.save(config, overwrite)?;
    if (old != next || hub.app_error.is_some())
        && let Err(error) = apply_autostart(&app, next)
    {
        let mut rollback = hub.config.clone();
        rollback.settings.launch_at_login = old;
        hub.save(rollback, false)?;
        return Err(error);
    }
    hub.app_error = None;
    Ok(())
}

#[tauri::command]
fn reload_config(app: tauri::AppHandle, state: tauri::State<Shared>) -> Result<()> {
    let mut hub = state.0.lock().unwrap();
    hub.reload()?;
    apply_autostart(&app, hub.config.settings.launch_at_login)?;
    hub.app_error = None;
    Ok(())
}

#[tauri::command]
fn recover_config(app: tauri::AppHandle, state: tauri::State<Shared>, backup: bool) -> Result<()> {
    let mut hub = state.0.lock().unwrap();
    hub.recover(backup)?;
    apply_autostart(&app, hub.config.settings.launch_at_login)?;
    hub.app_error = None;
    Ok(())
}

#[tauri::command]
fn preview_command(state: tauri::State<Shared>, task: Task) -> Result<Preview> {
    let hub = state.0.lock().unwrap();
    model::render(&task, &hub.config, &hub.store.root, false)
}

#[tauri::command]
async fn run_task(state: tauri::State<'_, Shared>, task_id: String, action: String) -> Result<()> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || match action.as_str() {
        "start" => shared.start(&task_id),
        "stop" => shared.stop(&task_id),
        "restart" => shared.restart(&task_id),
        _ => Err("未知运行操作".into()),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn choose_path(app: tauri::AppHandle, directory: bool) -> Result<Option<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = app.dialog().file();
        let file = if directory {
            dialog.blocking_pick_folder()
        } else {
            dialog.blocking_pick_file()
        };
        file.map(|p| {
            p.into_path()
                .map(|p| p.to_string_lossy().to_string())
                .map_err(|e| e.to_string())
        })
        .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn import_file(
    state: tauri::State<'_, Shared>,
    source: String,
    replace: Option<String>,
) -> Result<()> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        shared
            .0
            .lock()
            .unwrap()
            .import(PathBuf::from(source), replace)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn delete_file(state: tauri::State<Shared>, file_id: String) -> Result<()> {
    state.0.lock().unwrap().delete_file(&file_id)
}

#[tauri::command]
fn open_directory(
    app: tauri::AppHandle,
    state: tauri::State<Shared>,
    file_id: Option<String>,
) -> Result<()> {
    let hub = state.0.lock().unwrap();
    let path = if let Some(file_id) = file_id {
        let f = hub
            .config
            .files
            .iter()
            .find(|f| f.id == file_id)
            .ok_or("文件不存在")?;
        hub.store.root.join("files").join(&f.id)
    } else {
        hub.store.root.clone()
    };
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn read_output(state: tauri::State<Shared>, run_id: String) -> Result<String> {
    state.0.lock().unwrap().output(&run_id)
}
#[tauri::command]
fn clear_output(state: tauri::State<Shared>, run_id: String) -> Result<()> {
    state.0.lock().unwrap().clear_output(&run_id)
}
#[tauri::command]
fn clear_history(state: tauri::State<Shared>, task_id: String) -> Result<()> {
    state.0.lock().unwrap().clear_history(&task_id)
}
#[tauri::command]
async fn cleanup_run(
    state: tauri::State<'_, Shared>,
    run_id: String,
    terminate: bool,
) -> Result<()> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        shared.0.lock().unwrap().cleanup(&run_id, terminate)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn request_exit(app: tauri::AppHandle) {
    if EXIT_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let shared = app.state::<Shared>().inner().clone();
    std::thread::spawn(move || match shared.shutdown() {
        Ok(()) => {
            EXIT_ALLOWED.store(true, Ordering::SeqCst);
            app.exit(0);
        }
        Err(e) => {
            shared.0.lock().unwrap().shutting_down = false;
            show(&app);
            let _ = app.emit("hub-error", e);
            EXIT_PENDING.store(false, Ordering::SeqCst);
        }
    });
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    request_exit(app);
}

#[tauri::command]
fn window_minimize(window: tauri::WebviewWindow) -> Result<()> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::WebviewWindow) -> Result<()> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn window_close(window: tauri::WebviewWindow) -> Result<()> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
fn window_is_maximized(window: tauri::WebviewWindow) -> Result<bool> {
    window.is_maximized().map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    if process::handle_console_helper() {
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show(app)
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = dirs::home_dir()
                .ok_or("无法找到用户主目录")?
                .join(".script-hub");
            let shared = Shared(Arc::new(Mutex::new(
                Hub::open(root).map_err(std::io::Error::other)?,
            )));
            let (auto_ids, launch_at_login, valid) = {
                let hub = shared.0.lock().unwrap();
                (
                    hub.config
                        .tasks
                        .iter()
                        .filter(|t| t.auto_start)
                        .map(|t| t.id.clone())
                        .collect::<Vec<_>>(),
                    hub.config.settings.launch_at_login,
                    hub.config_error.is_none(),
                )
            };
            app.manage(shared.clone());
            use tauri::{
                menu::{Menu, MenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };
            let open = MenuItem::with_id(app, "show", "打开 Script Hub", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "停止任务并退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().ok_or("缺少托盘图标")?.clone())
                .tooltip("Script Hub")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show(app),
                    "quit" => request_exit(app.clone()),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show(tray.app_handle());
                    }
                })
                .build(app)?;
            if valid && let Err(e) = apply_autostart(app.handle(), launch_at_login) {
                shared.0.lock().unwrap().app_error = Some(e);
            }
            if !std::env::args().any(|a| a == "--autostart") {
                show(app.handle());
            }
            if valid {
                for id in auto_ids {
                    let shared = shared.clone();
                    std::thread::spawn(move || {
                        let _ = shared.start(&id);
                    });
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_config,
            reload_config,
            recover_config,
            preview_command,
            run_task,
            choose_path,
            import_file,
            delete_file,
            open_directory,
            read_output,
            clear_output,
            clear_history,
            cleanup_run,
            quit_app,
            window_minimize,
            window_toggle_maximize,
            window_close,
            window_is_maximized
        ])
        .build(tauri::generate_context!())
        .expect("无法初始化 Script Hub")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !EXIT_ALLOWED.load(Ordering::SeqCst)
            {
                api.prevent_exit();
                request_exit(app.clone());
            }
        });
}
