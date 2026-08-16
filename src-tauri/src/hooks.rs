use rdev::{listen, EventType, Key};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::state::{AppState, VakhState};
use crate::perform_state_transition;
use crate::db::Database;
pub use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, GetWindowTextW, GetClassNameW, GetWindow, GW_HWNDNEXT, IsWindowVisible,
    GetGUIThreadInfo, GUITHREADINFO
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, PROCESS_NAME_WIN32};
use windows_sys::Win32::Foundation::{CloseHandle, FALSE};

// Added in 0.2.8: Captures the specifically focused child-control window (e.g. text area, input box)
// using GetGUIThreadInfo instead of just the top-level parent window.
// Previously, we only used get_foreground_window_ignoring_vakh() which fetched the main application window,
// causing typing injection to fail or mismatch if a complex app had multiple sub-controls/embedded text panes.
pub fn get_focused_control_hwnd() -> isize {
    unsafe {
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let result = GetGUIThreadInfo(0, &mut info);
        println!("[Hooks] GetGUIThreadInfo result: {}, hwndFocus: {}", result, info.hwndFocus);
        if result != 0 && info.hwndFocus != 0 {
            // Verify the focused control does not belong to VAKH itself
            let (title, class) = get_window_details(info.hwndFocus as isize);
            let title_lower = title.to_lowercase();
            let class_lower = class.to_lowercase();
            let proc_name = get_process_name(info.hwndFocus as isize)
                .unwrap_or_else(|| "".to_string())
                .to_lowercase();
            let is_vakh = title_lower.contains("vakh") 
                || class_lower.contains("tauri") 
                || proc_name.contains("vakh")
                || proc_name.contains("tauri");
            if is_vakh {
                let fg = get_foreground_window_ignoring_vakh();
                println!("[Hooks] Focused control is VAKH. Falling back to foreground ignoring VAKH: {}", fg);
                fg
            } else {
                info.hwndFocus
            }
        } else {
            let fg = get_foreground_window_ignoring_vakh();
            println!("[Hooks] No focused control. Using foreground ignoring VAKH: {}", fg);
            fg
        }
    }
}

pub fn get_foreground_window_ignoring_vakh() -> isize {
    unsafe {
        let mut hwnd = GetForegroundWindow();
        
        // Loop to find the first visible window that isn't VAKH or a system tray
        while hwnd != 0 {
            let (title, class) = get_window_details(hwnd as isize);
            let title_lower = title.to_lowercase();
            let class_lower = class.to_lowercase();
            
            let proc_name = get_process_name(hwnd as isize)
                .unwrap_or_else(|| "".to_string())
                .to_lowercase();
            
            let is_vakh = title_lower.contains("vakh") 
                || class_lower.contains("tauri") 
                || proc_name.contains("vakh")
                || proc_name.contains("tauri");
                
            let is_system = class_lower == "shell_traywnd" || class_lower == "workerw" || class_lower == "progman";
            
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

pub fn start_hotkey_listener(handle: AppHandle, state: Arc<Mutex<AppState>>, db: Arc<Database>, ctx: Arc<WhisperContext>, config: Arc<Mutex<AppConfig>>) {
    let mut last_ctrl_press = Instant::now() - Duration::from_secs(10);
    
    std::thread::spawn(move || {
        println!("[Hooks] Global key listener thread started.");
        if let Err(error) = listen(move |event| {
            if let EventType::KeyPress(key) = event.event_type {
                println!("[Hooks] KeyPress event: {:?}", key);
                if key == Key::ControlLeft || key == Key::ControlRight {
                    let now = Instant::now();
                    let diff = now.duration_since(last_ctrl_press);
                    println!("[Hooks] Ctrl press detected (diff={:?})", diff);
                    if diff < Duration::from_millis(400) {
                        println!("[Hooks] Double Ctrl speed match (<400ms)!");
                        
                        // Capture active focused control HWND (ignoring VAKH itself)
                        let hwnd = get_focused_control_hwnd();
                        let proc_name = get_process_name(hwnd as isize).unwrap_or_else(|| "Unknown".to_string());
                        println!("[Hooks] Captured HWND: {} ({})", hwnd, proc_name);
                        
                        {
                            let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
                            if app_state.current_state == VakhState::Idle {
                                app_state.target_hwnd = Some(hwnd as isize);
                            }
                        }

                        // Perform transition asynchronously to prevent deadlocking the global hook thread
                        // during cpal audio stream teardown/pause operations.
                        let handle_clone = handle.clone();
                        let state_clone = Arc::clone(&state);
                        let db_clone = Arc::clone(&db);
                        let ctx_clone = Arc::clone(&ctx);
                        let config_clone = config.clone();
                        std::thread::spawn(move || {
                            println!("[Hooks] Spawning perform_state_transition thread...");
                            perform_state_transition(handle_clone, state_clone, db_clone, ctx_clone, config_clone);
                        });
                        
                        // Reset
                        last_ctrl_press = now - Duration::from_secs(10);
                    } else {
                        last_ctrl_press = now;
                    }
                }
            }
        }) {
            eprintln!("Error listening to events: {:?}", error);
        }
    });
}
