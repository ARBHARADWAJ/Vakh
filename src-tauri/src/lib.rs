// ============================================================
// VAKH - Single-Worker Context Architecture
// ============================================================

mod state;
mod db;
mod audio;
mod hooks;
mod injection;
mod config;

use state::{AppState, VakhState, SendWrapper, AudioStatus};
use config::AppConfig;
use std::sync::{Arc, Mutex};
use tauri::{State, Manager, Emitter, AppHandle};
use tauri_plugin_notification::NotificationExt;
use std::sync::mpsc::{channel, Sender};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use injection::TextInjector;
use db::{Database, LogEntry, AppStats};
use cpal::traits::StreamTrait;

struct AppManagedState(Arc<Mutex<AppState>>, Arc<Database>, Arc<WhisperContext>, Arc<Mutex<AppConfig>>);

enum ContextCommand {
    Finalize(Sender<String>),
}

pub fn perform_state_transition(
    app_handle: AppHandle,
    state: Arc<Mutex<AppState>>,
    db: Arc<Database>,
    ctx: Arc<WhisperContext>,
    config_mutex: Arc<Mutex<AppConfig>>,
) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
    }
    let _ = app_handle.emit("vakh-show", ());

    let config = {
        let lock = config_mutex.lock().unwrap_or_else(|e| e.into_inner());
        lock.clone()
    };

    let (_next_state, start_audio) = {
        let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
        
        // Guard: If the app is currently in Flushing (Finalizing) state, prevent starting a new recording.
        // We emit a busy warning status to inform the user.
        if app_state.current_state == VakhState::Flushing {
            let _ = app_handle.emit("vakh-status", AudioStatus::Busy { 
                message: "the dictation is in process, kindly wait".to_string() 
            });
            return;
        }

        let (next_state, start_audio) = match app_state.current_state {
            VakhState::Idle       => (VakhState::Listening, true),
            VakhState::Listening  => (VakhState::Flushing,  false),
            VakhState::Processing => (VakhState::Flushing,  false),
            VakhState::Flushing   => (VakhState::Flushing,  false), // guarded above
        };
        app_state.transition_to(next_state);
        (next_state, start_audio)
    };

    if start_audio {
        let _ = app_handle.emit("vakh-start-listening", ());

        let (tx1, rx1) = channel::<Vec<f32>>(); // VAD Gated (for State Manager)
        let (tx2, rx2) = channel::<Vec<f32>>(); // Raw Stream (for AI Worker)
        let (status_tx, status_rx) = channel::<AudioStatus>();

        match audio::AudioProcessor::start_listening(
            tx1,
            tx2,
            status_tx,
            config.vad_sensitivity,
            config.silence_timeout,
        ) {
            Ok(stream) => {
                {
                    let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
                    app_state.audio_stream = Some(SendWrapper(stream));
                }

                let state_for_thread = Arc::clone(&state);
                let db_for_thread    = Arc::clone(&db);
                let ctx_for_thread   = Arc::clone(&ctx);
                let handle_for_thread = app_handle.clone();
                let config_for_thread = config.clone();

                let (target_app_name, target_hwnd) = {
                    let mut s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                    
                    // Use existing HWND if captured by hotkey, otherwise resolve it.
                    // Previously: Resolved via hooks::get_foreground_window_ignoring_vakh() (retrieved top-level window).
                    // Now: Resolves specifically to the focused child-control control via hooks::get_focused_control_hwnd().
                    let hwnd = match s.target_hwnd {
                        Some(h) if h != 0 => h,
                        _ => hooks::get_focused_control_hwnd(),
                    };

                    let name = hooks::get_process_name(hwnd)
                        .unwrap_or_else(|| "Unknown App".to_string());
                    println!("[Injection] Target: {} (hwnd={})", name, hwnd);
                    s.target_hwnd = Some(hwnd);
                    (name, hwnd)
                };

                std::thread::spawn(move || {
                    let injector = Arc::new(Mutex::new(TextInjector::new(
                        Some(target_hwnd),
                        config_for_thread.typing_delay,
                        config_for_thread.backspace_delay,
                    )));

                    // --- WORKER THREAD: Context Thread (High Accuracy Accumulator) ---
                    let (ctx_tx, ctx_rx) = channel::<ContextCommand>();
                    let ctx_context = Arc::clone(&ctx_for_thread);
                    let ctx_config = config_for_thread.clone();
                    let db_for_worker = Arc::clone(&db_for_thread);
                    let app_name_for_worker = target_app_name.clone();

                    let injector_for_worker = Arc::clone(&injector);

                    std::thread::spawn(move || {
                        let mut ctx_state = match ctx_context.create_state() {
                            Ok(s)  => s,
                            Err(e) => { eprintln!("[AI Error] Context State failed: {:?}", e); return; }
                        };

                        let mut full_session_audio: Vec<f32> = Vec::with_capacity(16000 * 25);
                        
                        // Independent Audio Collection from RX2
                        let worker_audio_rx = rx2;
                        
                        let lang_str = ctx_config.language.clone();
                        
                        let final_params = {
                            let mut p = FullParams::new(SamplingStrategy::BeamSearch {
                                beam_size: 5, patience: 2.0,
                            });
                            p.set_language(Some(&lang_str));
                            p.set_n_threads(ctx_config.threads);
                            p.set_print_special(false);
                            p.set_print_progress(false);
                            p.set_print_realtime(false);
                            p.set_print_timestamps(false);
                            p
                        };

                        let mut has_transcribed_periodic = false;
                        // Added in 0.2.8: Accumulate text from prior chunks within the same dictation session.
                        // Previously: Whisper transcribed each chunk in isolation, losing semantic context (which
                        // caused spelling inconsistencies and ignored context at 15s boundaries).
                        // Now: We pass accumulated text as an initial prompt to improve context awareness.
                        let mut accumulated_session_text = String::new();

                        loop {
                            // 1. Collect all available raw audio
                            while let Ok(samples) = worker_audio_rx.try_recv() {
                                full_session_audio.extend_from_slice(&samples);
                            }

                            // 2. Periodic 15-second transcribing & appending
                            if full_session_audio.len() >= 16000 * 15 {
                                println!("[Worker] Periodic 15s append trigger: {} samples", full_session_audio.len());
                                let mut chunk_params = final_params.clone();
                                if !accumulated_session_text.is_empty() {
                                    // Seed next chunk's transcription with the accumulated context
                                    chunk_params.set_initial_prompt(&accumulated_session_text);
                                }
                                if let Ok(_) = ctx_state.full(chunk_params, &full_session_audio) {
                                    let chunk_text = collect_segments(&mut ctx_state);
                                    println!("[Worker] Periodic transcribed text: '{}'", chunk_text);
                                    if !chunk_text.is_empty() {
                                        let mut inj = injector_for_worker.lock().unwrap_or_else(|e| e.into_inner());
                                        inj.inject_draft(&chunk_text);
                                        inj.commit();
                                        let _ = db_for_worker.log_dictation(&chunk_text, Some(&app_name_for_worker));
                                        
                                        if !accumulated_session_text.is_empty() {
                                            accumulated_session_text.push(' ');
                                        }
                                        accumulated_session_text.push_str(&chunk_text);
                                        has_transcribed_periodic = true;
                                    }
                                }
                                full_session_audio.clear();
                            }

                            // 3. Check for commands (Finalize)
                            if let Ok(cmd) = ctx_rx.try_recv() {
                                match cmd {
                                    ContextCommand::Finalize(reply_tx) => {
                                        println!("[Worker] Finalize trigger: {} samples", full_session_audio.len());
                                        let mut final_text = String::new();
                                        let min_samples = if has_transcribed_periodic {
                                            32_000 // 2.0s minimum for final chunk if we already had a periodic chunk
                                        } else {
                                            8_000  // 0.5s minimum for short single dictation
                                        };
                                        if full_session_audio.len() >= min_samples {
                                            let mut chunk_params = final_params.clone();
                                            if !accumulated_session_text.is_empty() {
                                                // Seed final chunk transcription with accumulated context
                                                chunk_params.set_initial_prompt(&accumulated_session_text);
                                            }
                                            if let Ok(_) = ctx_state.full(chunk_params, &full_session_audio) {
                                                final_text = collect_segments(&mut ctx_state);
                                            }
                                        }
                                        println!("[Worker] Final transcribed text: '{}'", final_text);
                                        full_session_audio.clear();
                                        let _ = reply_tx.send(final_text);
                                        break; 
                                    }
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    });

                    // Helper closure to run finalize and inject result
                    let run_finalize = |injector: &Arc<Mutex<TextInjector>>,
                                        ctx_tx: &std::sync::mpsc::Sender<ContextCommand>,
                                        handle: &AppHandle,
                                        db: &Arc<Database>,
                                        app_name: &str| {
                        let _ = handle.emit("vakh-status", AudioStatus::Finalizing);

                        let (reply_tx, reply_rx) = channel::<String>();
                        let _ = ctx_tx.send(ContextCommand::Finalize(reply_tx));

                        match reply_rx.recv_timeout(std::time::Duration::from_secs(60)) {
                            Ok(final_text) => {
                                println!("[Finalize] Result: '{}'", final_text);
                                if !final_text.is_empty() {
                                    let mut inj = injector.lock().unwrap_or_else(|e| e.into_inner());
                                    inj.inject_draft(&final_text);
                                    inj.commit();
                                    let _ = db.log_dictation(&final_text, Some(app_name));
                                }
                            }
                            Err(_) => {}
                        }
                    };

                    // --- MAIN LOOP: VAD & State Router ---
                    let mut auto_halt = false;
                    let session_start = std::time::Instant::now();
                    loop {
                        if session_start.elapsed() >= std::time::Duration::from_secs(300) {
                            println!("[System] 5-minute safety limit reached. Auto-halting.");
                            auto_halt = true;
                        }

                        while let Ok(status) = status_rx.try_recv() {
                            match status {
                                AudioStatus::Idle => {
                                    println!("[System] Auto-Stop ({}s silence)", config_for_thread.silence_timeout);
                                    auto_halt = true;
                                }
                                _ => {
                                    let _ = handle_for_thread.emit("vakh-status", status);
                                }
                            }
                        }

                        if auto_halt {
                            {
                                let mut s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                                if let Some(SendWrapper(stream)) = s.audio_stream.take() {
                                    let _ = stream.pause(); // ensure mic hardware stops
                                    drop(stream);
                                }
                            }
                            run_finalize(&injector, &ctx_tx, &handle_for_thread, &db_for_thread, &target_app_name);
                            break;
                        }

                        // Drain RX1 just to keep it alive (VAD already processed by AudioProcessor)
                        while let Ok(_) = rx1.try_recv() {}

                        let current_state = {
                            let s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                            s.current_state
                        };
                        if current_state == VakhState::Idle || current_state == VakhState::Flushing {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }

                    if !auto_halt {
                        run_finalize(&injector, &ctx_tx, &handle_for_thread, &db_for_thread, &target_app_name);
                    }

                    let mut s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                    s.transition_to(VakhState::Idle);
                    s.target_hwnd = None;
                    let _ = handle_for_thread.emit("vakh-status", AudioStatus::Idle);
                });
            }
            Err(e) => {
                eprintln!("[Audio Error] Could not start mic: {}", e);
                let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
                app_state.transition_to(VakhState::Idle);
            }
        }
    } else {
        let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(SendWrapper(stream)) = app_state.audio_stream.take() {
            let _ = stream.pause(); // ensure mic hardware stops
            drop(stream);
        }
        // Change in 0.2.8: Transition to Flushing during stopping. 
        // The background thread loop will break immediately and run run_finalize, 
        // which will transition to Idle and emit AudioStatus::Idle upon completion.
        app_state.transition_to(VakhState::Flushing);
    }
}

fn collect_segments(whisper_state: &mut whisper_rs::WhisperState) -> String {
    const HALLUCINATIONS: &[&str] = &[
        "bye", "thanks", "thank you", "bye.", "thanks.", "thank you.", "bye-",
    ];

    let mut segments = Vec::new();
    for segment in whisper_state.as_iter() {
        if let Ok(raw) = segment.to_str() {
            let cleaned = raw.trim();
            if cleaned.is_empty() || cleaned.starts_with('[') { continue; }

            let lower = cleaned.to_lowercase();
            if lower.contains("blank_audio") || lower.contains("[blank_audio]") || lower.trim() == ">>" {
                continue;
            }

            if cleaned.len() < 10 && HALLUCINATIONS.iter().any(|h| lower == *h) { continue; }
            if !cleaned.chars().any(|c| c.is_alphanumeric()) { continue; }

            segments.push(cleaned.to_string());
        }
    }

    let mut result = segments.join(" ");
    if result.trim().is_empty() { return String::new(); }

    if let Some(first) = result.chars().next() {
        let upper: String = first.to_uppercase().collect();
        result = upper + &result[first.len_utf8()..];
    }
    result
}

#[tauri::command]
fn hide_window(handle: AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn start_dragging(handle: AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.start_dragging();
    }
}

#[tauri::command]
fn toggle_listening(
    handle: AppHandle,
    state: State<AppManagedState>,
) -> Result<VakhState, String> {
    let state_handle = state.inner().0.clone();
    let db_handle    = state.inner().1.clone();
    let ctx_handle   = state.inner().2.clone();
    let config       = state.inner().3.clone();
    perform_state_transition(handle, state_handle.clone(), db_handle, ctx_handle, config);
    let app_state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    Ok(app_state.current_state)
}

#[tauri::command]
fn get_config(state: State<AppManagedState>) -> AppConfig {
    let lock = state.inner().3.lock().unwrap_or_else(|e| e.into_inner());
    lock.clone()
}

#[tauri::command]
fn save_config(handle: AppHandle, state: State<AppManagedState>, config: AppConfig) -> Result<(), String> {
    // Save to disk
    let mut path = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
    path.push("VAKH");
    let _ = std::fs::create_dir_all(&path);
    path.push("config.json");
    
    if let Ok(json) = serde_json::to_string_pretty(&config) {
        if let Err(e) = std::fs::write(path, json) {
            return Err(format!("Failed to write config: {}", e));
        }
    } else {
        return Err("Failed to serialize config".to_string());
    }

    // Update state
    {
        let mut lock = state.inner().3.lock().unwrap_or_else(|e| e.into_inner());
        *lock = config.clone();
    }

    // Emit configuration change event to let frontend windows react immediately
    let _ = handle.emit("vakh-config-changed", config);

    Ok(())
}

#[tauri::command]
fn get_dictation_logs(state: State<AppManagedState>) -> Result<Vec<LogEntry>, String> {
    state.inner().1.get_dictation_logs().map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_log(state: State<AppManagedState>, id: i64) -> Result<(), String> {
    state.inner().1.delete_log(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_logs(state: State<AppManagedState>) -> Result<(), String> {
    state.inner().1.clear_logs().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stats(state: State<AppManagedState>) -> Result<AppStats, String> {
    state.inner().1.get_stats().map_err(|e| e.to_string())
}

#[tauri::command]
fn open_dashboard(handle: AppHandle) {
    if let Some(window) = handle.get_webview_window("dashboard") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn hide_dashboard(handle: AppHandle) {
    if let Some(window) = handle.get_webview_window("dashboard") {
        let _ = window.hide();
    }
}

#[derive(Debug, serde::Serialize, Clone)]
struct ModelStatus {
    is_custom: bool,
    path: String,
    size_mb: f64,
}

#[tauri::command]
fn get_model_status() -> ModelStatus {
    let mut model_path = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    model_path.push("VAKH");
    model_path.push("model.bin");

    if model_path.exists() {
        let size_bytes = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);
        let size_mb = (size_bytes as f64) / (1024.0 * 1024.0);
        ModelStatus {
            is_custom: true,
            path: model_path.to_string_lossy().to_string(),
            size_mb: (size_mb * 100.0).round() / 100.0,
        }
    } else {
        ModelStatus {
            is_custom: false,
            path: "Embedded (tiny.en.bin)".to_string(),
            size_mb: 74.1,
        }
    }
}

#[tauri::command]
fn change_model() -> Result<String, String> {
    unsafe {
        let mut file_name: [u16; 260] = [0; 260];
        let mut ofn: windows_sys::Win32::UI::Controls::Dialogs::OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<windows_sys::Win32::UI::Controls::Dialogs::OPENFILENAMEW>() as u32;
        ofn.lpstrFile = file_name.as_mut_ptr();
        ofn.nMaxFile = file_name.len() as u32;
        
        let filter = "GGML Whisper Model (*.bin)\0*.bin\0All Files (*.*)\0*.*\0\0";
        let filter_u16: Vec<u16> = filter.encode_utf16().collect();
        ofn.lpstrFilter = filter_u16.as_ptr();
        ofn.nFilterIndex = 1;
        ofn.Flags = 0x00080000 | 0x00001000 | 0x00000800; // OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST
        
        let success = windows_sys::Win32::UI::Controls::Dialogs::GetOpenFileNameW(&mut ofn);
        if success != 0 {
            let len = file_name.iter().position(|&x| x == 0).unwrap_or(file_name.len());
            let path_str = String::from_utf16_lossy(&file_name[..len]);
            let src_path = std::path::PathBuf::from(path_str);

            let mut dest_path = std::env::var("LOCALAPPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            dest_path.push("VAKH");
            let _ = std::fs::create_dir_all(&dest_path);
            dest_path.push("model.bin");

            // Copy selected file
            std::fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("Failed to copy model file: {}", e))?;

            Ok("Model updated successfully. Please restart VAKH to load the new model.".to_string())
        } else {
            Err("cancelled".to_string())
        }
    }
}

#[tauri::command]
fn reset_model() -> Result<String, String> {
    let mut model_path = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    model_path.push("VAKH");
    model_path.push("model.bin");

    if model_path.exists() {
        std::fs::remove_file(model_path)
            .map_err(|e| format!("Failed to delete custom model: {}", e))?;
        Ok("Model reset to embedded. Please restart VAKH to apply changes.".to_string())
    } else {
        Ok("Already using embedded model.".to_string())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
struct UpdateCheckResult {
    has_update: bool,
    current_version: String,
    latest_version: String,
    download_url: String,
    changelog: String,
}

#[derive(Debug, serde::Deserialize)]
struct RemoteUpdateInfo {
    version: String,
    url: String,
    changelog: Option<String>,
}

#[tauri::command]
fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let current_version = "0.2.0".to_string(); // Current VAKH version
    // Endpoint on user's GitHub repository for checking the updater.json manifest
    let update_url = "https://raw.githubusercontent.com/ARBHARADWAJ/Vakh/main/updater.json";
    
    let response = ureq::get(update_url)
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .map_err(|e| format!("Failed to check updates: {}", e))?;
        
    let remote_info: RemoteUpdateInfo = response.into_json::<RemoteUpdateInfo>()
        .map_err(|e| format!("Failed to parse update info: {}", e))?;
        
    let has_update = remote_info.version != current_version;
    
    Ok(UpdateCheckResult {
        has_update,
        current_version,
        latest_version: remote_info.version,
        download_url: remote_info.url,
        changelog: remote_info.changelog.unwrap_or_else(|| "No changelog provided.".to_string()),
    })
}

#[tauri::command]
fn download_and_install_update(url: String) -> Result<(), String> {
    let mut temp_path = std::env::temp_dir();
    temp_path.push("vakh_setup_update.exe");

    println!("[Updater] Downloading update from {} to {:?}", url, temp_path);

    let response = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(120)) // 2 min timeout for download
        .call()
        .map_err(|e| format!("Failed to request download: {}", e))?;

    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| format!("Failed to create local update file: {}", e))?;

    std::io::copy(&mut response.into_reader(), &mut file)
        .map_err(|e| format!("Failed to save update file: {}", e))?;

    println!("[Updater] Download finished. Spawning installer: {:?}", temp_path);

    // Launch installer asynchronously
    std::process::Command::new("cmd")
        .args(&["/C", &temp_path.to_string_lossy()])
        .spawn()
        .map_err(|e| format!("Failed to launch update installer: {}", e))?;

    // Terminate VAKH immediately so the installer doesn't hit locked-file errors
    std::process::exit(0);
}

#[tauri::command]
fn minimize_dashboard(handle: AppHandle) {
    if let Some(window) = handle.get_webview_window("dashboard") {
        let _ = window.minimize();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(Mutex::new(AppState::new()));
    let config    = AppConfig::load();
    let database  = Arc::new(Database::init().expect("failed to init db"));

    println!("[Whisper] Loading model...");
    let mut model_path = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    model_path.push("VAKH");
    model_path.push("model.bin");

    let mut model_data_vec = Vec::new();
    let mut use_embedded = true;
    if model_path.exists() {
        println!("[Whisper] Loading external model from {:?}", model_path);
        match std::fs::read(&model_path) {
            Ok(data) => {
                model_data_vec = data;
                use_embedded = false;
            }
            Err(e) => {
                eprintln!("[Whisper Warning] Failed to read external model: {}. Falling back to embedded.", e);
            }
        }
    }

    let embedded_data = include_bytes!("../tiny.en.bin");
    let model_bytes: &[u8] = if use_embedded {
        embedded_data
    } else {
        &model_data_vec
    };

    let ctx = Arc::new(
        WhisperContext::new_from_buffer_with_params(
            model_bytes,
            WhisperContextParameters::default(),
        )
        .expect("failed to load Whisper model"),
    );
    println!("[Whisper] Model loaded successfully");

    let shared_config = Arc::new(Mutex::new(config));

    let state_for_hooks  = Arc::clone(&app_state);
    let db_for_hooks     = Arc::clone(&database);
    let ctx_for_hooks    = Arc::clone(&ctx);
    let config_for_hooks = Arc::clone(&shared_config);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "dashboard" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .setup(move |app| {
            let handle_for_hooks = app.handle().clone();
            hooks::start_hotkey_listener(
                handle_for_hooks,
                state_for_hooks,
                db_for_hooks,
                ctx_for_hooks,
                config_for_hooks,
            );

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(200));

                    if let Ok(Some(monitor)) = window_clone.current_monitor() {
                        let screen_size = monitor.size();
                        let scale_factor = monitor.scale_factor();

                        if let Ok(window_size) = window_clone.outer_size() {
                            let x = (screen_size.width as i32 - window_size.width as i32) / 2;
                            let y = screen_size.height as i32 - window_size.height as i32 - (150.0 * scale_factor) as i32;

                            println!("[Window] Screen: {}x{}, Window: {}x{}, Position: {},{}",
                                screen_size.width, screen_size.height,
                                window_size.width, window_size.height, x, y);

                            let _ = window_clone.set_position(tauri::PhysicalPosition { x, y });
                        }
                    }
                });
            }

            // Send notification that the app is running in the background
            let _ = app.notification()
                .builder()
                .title("VAKH")
                .body("Running in the background. Double-press Left or Right Ctrl to dictate.")
                .show();

            app.manage(AppManagedState(
                Arc::clone(&app_state),
                Arc::clone(&database),
                Arc::clone(&ctx),
                Arc::clone(&shared_config),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_listening,
            hide_window,
            get_config,
            save_config,
            get_dictation_logs,
            delete_log,
            clear_logs,
            get_stats,
            open_dashboard,
            hide_dashboard,
            minimize_dashboard,
            start_dragging,
            get_model_status,
            change_model,
            reset_model,
            check_for_updates,
            download_and_install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}