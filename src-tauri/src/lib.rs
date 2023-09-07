use std::{thread, time::Duration, sync::Mutex, collections::HashMap, process};

use tauri::{Manager, AppHandle, SystemTrayEvent, WindowUrl, WindowBuilder};
use sysinfo::{ProcessExt, System, SystemExt, PidExt};
use winapi::um::winuser::*;

use persistent::save_projects;

pub mod handlers;
pub mod persistent;

const UPDATE_INTERVAL: Duration = Duration::from_secs(1);
const PRODUCT_NAME: &str = "UnityDevTimer";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    display_name: String,
    time: Duration,
    open: bool,
}

#[derive(Default)]
pub struct AppState {
    projects: Mutex<HashMap<String, Project>>,
}

impl AppState {
    pub fn new(projects: HashMap<String, Project>) -> Self {
        Self {
            projects: Mutex::new(projects),
        }
    }
}

pub fn update_loop(app: tauri::AppHandle) {
    let mut system = System::new_all();
    thread::spawn(move || loop {
        thread::sleep(UPDATE_INTERVAL);

        system.refresh_all();

        let state = app.state::<AppState>();
        let mut projects = state.projects.lock().unwrap();

        for project in projects.values_mut() {
            project.open = false;
        }

        for process in system.processes_by_exact_name("Unity.exe") {
            let open_project_names = get_process_windows(process.pid().as_u32()).into_iter()
                .filter(|title| title.contains("- Unity"))
                .map(|title| {
                    let segments: Vec<&str> = title.split("-").collect();

                    if segments.len() == 4 {
                        return segments[0].trim().to_string();
                    }

                    segments[..segments.len() - 3].join("-").trim().to_string()
                });

            for project_name in open_project_names {
                let project = projects
                    .entry(project_name.clone())
                    .or_insert_with(|| Project { 
                        display_name: format_project_name(&project_name), 
                        time: Duration::from_secs(0),
                        open: true,
                    });

                project.time += UPDATE_INTERVAL;
                project.open = true;
            }
        }

        if let Err(err) = send_update(&projects, &app) {
            eprintln!("Error sending update: {}", err);
        }
    });
}

#[derive(serde::Serialize, Clone, Default)]
struct UpdatePayload {
    project_names: Vec<String>,
    projects: Vec<Project>
}

fn send_update(projects: &HashMap<String, Project>, app: &AppHandle) -> tauri::Result<()> {
    let payload = UpdatePayload {
        project_names: projects.keys().cloned().collect(),
        projects: projects.values().cloned().collect(),
    };

    app.emit_all("update", payload)?;
    save_projects(projects);

    Ok(())
}

pub fn handle_system_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::MenuItemClick { id, .. } => {
            let item = app.tray_handle().get_item(&id);
            match id.as_str() {
                "hide" => {
                    if let Ok(open) = toggle_window(app) {
                        item.set_title(if open { "Hide" } else { "Show" }).ok();
                    }
                },
                "quit" => process::exit(0),
                _ => { }
            }
        },
        SystemTrayEvent::LeftClick { .. } => {
            toggle_window(app).ok();
        },
        SystemTrayEvent::RightClick { .. } => { },
        SystemTrayEvent::DoubleClick { .. } => { },
        _ => { },
    }
}

fn toggle_window(app: &AppHandle) -> tauri::Result<bool> {
    let window = app.get_window("main").unwrap_or_else(|| {
        let url = WindowUrl::App("index.html".into());
        WindowBuilder::new(app, "main", url).build().expect("Failed to open window")
    });

    if window.is_visible()? {
        window.hide()?;
        return Ok(false);
    } else {
        window.show()?;
        return Ok(true)
    }
}

fn get_process_windows(process_id: u32) -> Vec<String> {
    let mut windows = Vec::new();

    unsafe {
        let mut window = GetTopWindow(std::ptr::null_mut());
        while window != std::ptr::null_mut() {
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == process_id {
                let mut title = [0u16; 1024];
                GetWindowTextW(window, title.as_mut_ptr(), 1024);
                windows.push(String::from_utf16_lossy(&title));
            }

            window = GetWindow(window, GW_HWNDNEXT);
        }
    }

    windows
}

fn format_project_name(name: &str) -> String {
    let mut formatted = String::new();
    let mut chars = name.chars();
    let mut last = ' ';

    while let Some(mut c) = chars.next() {
        if c.is_ascii_uppercase() && last.is_ascii_lowercase() {
            formatted.push(' ');
        }

        if c == '-' || c == '_' {
            c = ' ';
        }

        formatted.push(match last {
            ' ' => c.to_ascii_uppercase(),
            _ => c,
        });

        last = c;
    }

    formatted
}