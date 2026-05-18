use rdev::{listen, EventType, Key};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::state::{AppState, VakhState};
use crate::perform_state_transition;
use crate::db::Database;
pub use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, GetWindowTextW, GetClassNameW, GetWindow, GW_HWNDNEXT, IsWindowVisible
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, PROCESS_NAME_WIN32};
use windows_sys::Win32::Foundation::{CloseHandle, FALSE};

pub fn get_foreground_window_ignoring_vakh() -> isize {
    unsafe {
        let mut hwnd = GetForegroundWindow();
        
        // Loop to find the first visible window that isn't VAKH or a system tray
        while hwnd != 0 {
            let (title, class) = get_window_details(hwnd as isize);
            let is_vakh = title.contains("VAKH") || class.contains("Tauri");
            let is_system = class == "Shell_TrayWnd" || class == "WorkerW" || class == "Progman";
            
            if !is_vakh && !is_system && IsWindowVisible(hwnd) != 0 {
                return hwnd as isize;
            }
            hwnd = GetWindow(hwnd, GW_HWNDNEXT);
        }
        0
    }
}

pub fn get_window_details(hwnd: isize) -> (String, String) {
    unsafe {
        let mut text_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd as _, text_buf.as_mut_ptr(), 512);
        let title = String::from_utf16_lossy(&text_buf[..len as usize]);

        let mut class_buf = [0u16; 512];
        let len = GetClassNameW(hwnd as _, class_buf.as_mut_ptr(), 512);
        let class = String::from_utf16_lossy(&class_buf[..len as usize]);
        
        (title, class)
    }
}

pub fn get_process_name(hwnd: isize) -> Option<String> {
    unsafe {
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd as _, &mut process_id);
        if process_id == 0 { return None; }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process_id);
        if handle == 0 { return None; }

        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let success = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if success != 0 {
            let path = String::from_utf16_lossy(&buffer[..size as usize]);
            std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    }
}
use crate::config::AppConfig;
use tauri::AppHandle;

use whisper_rs::WhisperContext;

pub fn start_hotkey_listener(handle: AppHandle, state: Arc<Mutex<AppState>>, db: Arc<Database>, ctx: Arc<WhisperContext>, config: AppConfig) {
    let mut last_ctrl_press = Instant::now() - Duration::from_secs(10);
    
    std::thread::spawn(move || {
        if let Err(error) = listen(move |event| {
            if let EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) = event.event_type {
                let now = Instant::now();
                if now.duration_since(last_ctrl_press) < Duration::from_millis(400) {
                    println!("Double Ctrl detected!");
                    
                    // Capture active window HWND (ignoring VAKH itself)
                    let hwnd = get_foreground_window_ignoring_vakh();
                    let proc_name = get_process_name(hwnd as isize).unwrap_or_else(|| "Unknown".to_string());
                    println!("[Hooks] Captured HWND: {} ({})", hwnd, proc_name);
                    
                    {
                        let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
                        if app_state.current_state == VakhState::Idle {
                            app_state.target_hwnd = Some(hwnd as isize);
                        }
                    }

                    // Perform transition
                    perform_state_transition(handle.clone(), Arc::clone(&state), Arc::clone(&db), Arc::clone(&ctx), config.clone());
                    
                    // Reset
                    last_ctrl_press = now - Duration::from_secs(10);
                } else {
                    last_ctrl_press = now;
                }
            }
        }) {
            eprintln!("Error listening to events: {:?}", error);
        }
    });
}
