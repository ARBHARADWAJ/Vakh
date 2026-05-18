// ============================================================
// VAKH - Two-Threaded Context Architecture
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
use std::sync::mpsc::{channel, Sender};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use injection::TextInjector;
use db::Database;

struct AppManagedState(Arc<Mutex<AppState>>, Arc<Database>, Arc<WhisperContext>, AppConfig);

enum ContextCommand {
    AudioChunk(Vec<f32>),
    Silence(Vec<f32>), // Trigger high-accuracy sweep on silence
    Finalize(Sender<String>),
}

pub fn perform_state_transition(
    app_handle: AppHandle,
    state: Arc<Mutex<AppState>>,
    db: Arc<Database>,
    ctx: Arc<WhisperContext>,
    config: AppConfig,
) {
    // Ensure window is visible on any interaction
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
    }
    let _ = app_handle.emit("vakh-show", ());

    let (_next_state, start_audio) = {
        let mut app_state = state.lock().unwrap_or_else(|e| e.into_inner());
        let (next_state, start_audio) = match app_state.current_state {
            VakhState::Idle       => (VakhState::Listening, true),
            VakhState::Listening  => (VakhState::Flushing,  false),
            VakhState::Processing => (VakhState::Flushing,  false),
            VakhState::Flushing   => (VakhState::Idle,      false),
        };
        app_state.transition_to(next_state);
        (next_state, start_audio)
    };

    if start_audio {
        let _ = app_handle.emit("vakh-start-listening", ());

        let (tx, rx) = channel::<Vec<f32>>();
        let (status_tx, status_rx) = channel::<AudioStatus>();

        match audio::AudioProcessor::start_listening(tx, status_tx) {
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

                // Capture target app name once, before the thread spawns
                let (target_app_name, target_hwnd) = {
                    let mut s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                    let hwnd = hooks::get_foreground_window_ignoring_vakh();
                    let name = hooks::get_process_name(hwnd)
                        .unwrap_or_else(|| "Unknown App".to_string());
                    println!("[Injection] Target: {} (hwnd={})", name, hwnd);
                    s.target_hwnd = Some(hwnd);
                    (name, hwnd)
                };

                std::thread::spawn(move || {
                    let mut whisper_state = match ctx_for_thread.create_state() {
                        Ok(s)  => s,
                        Err(e) => { eprintln!("[AI Error] State creation failed: {:?}", e); return; }
                    };

                    let mut injector = TextInjector::new(Some(target_hwnd));

                    const PROCESS_STEP: usize  = 16_000 * 1; // 1.0s heartbeat

                    // --- THREAD 2: Context Thread (High Accuracy Polish) ---
                    let (ctx_tx, ctx_rx) = channel::<ContextCommand>();
                    let ctx_context = Arc::clone(&ctx_for_thread);
                    let ctx_config = config_for_thread.clone();
                    let handle_for_ctx = handle_for_thread.clone();
                    
                    std::thread::spawn(move || {
                        let mut ctx_state = match ctx_context.create_state() {
                            Ok(s)  => s,
                            Err(e) => { eprintln!("[AI Error] Context State failed: {:?}", e); return; }
                        };

                        let mut full_session_audio: Vec<f32> = Vec::with_capacity(16000 * 300);
                        let lang = ctx_config.language.clone();
                        let final_params = {
                            let mut p = FullParams::new(SamplingStrategy::BeamSearch {
                                beam_size: 5, patience: 2.0,
                            });
                            p.set_language(Some(&lang));
                            p.set_n_threads(ctx_config.threads);
                            p.set_print_special(false);
                            p.set_print_progress(false);
                            p.set_print_realtime(false);
                            p.set_print_timestamps(false);
                            p
                        };

                        while let Ok(cmd) = ctx_rx.recv() {
                            match cmd {
                                ContextCommand::AudioChunk(samples) => {
                                    full_session_audio.extend_from_slice(&samples);
                                }
                                ContextCommand::Silence(_) => {
                                    // Triggered on Auto-Pause or manual stop
                                    if let Ok(_) = ctx_state.full(final_params.clone(), &full_session_audio) {
                                        let high_accuracy_text = collect_segments(&mut ctx_state);
                                        let _ = handle_for_ctx.emit("vakh-correction", high_accuracy_text);
                                    }
                                }
                                ContextCommand::Finalize(reply_tx) => {
                                    if let Ok(_) = ctx_state.full(final_params.clone(), &full_session_audio) {
                                        let final_text = collect_segments(&mut ctx_state);
                                        let _ = reply_tx.send(final_text);
                                    } else {
                                        let _ = reply_tx.send(String::new());
                                    }
                                    break;
                                }
                            }
                        }
                    });

                    // --- THREAD 1: Main Worker (Fast Live Draft) ---
                    let lang_greedy = config_for_thread.language.clone();
                    let params = {
                        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                        p.set_language(Some(&lang_greedy));
                        p.set_n_threads(config_for_thread.threads);
                        p.set_print_special(false);
                        p.set_print_progress(false);
                        p.set_print_realtime(false);
                        p.set_print_timestamps(false);
                        p
                    };

                    let mut audio_buffer: Vec<f32> = Vec::with_capacity(16000 * 60);
                    let mut last_processed_len: usize = 0;

                    loop {
                        let mut auto_halt = false;
                        while let Ok(status) = status_rx.try_recv() {
                            if let AudioStatus::Idle = status {
                                println!("[System] Auto-Pause detected");
                                auto_halt = true;
                            }
                            let _ = handle_for_thread.emit("vakh-status", status);
                        }
                        if auto_halt { break; }

                        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                            Ok(samples) => {
                                audio_buffer.extend_from_slice(&samples);
                                let _ = ctx_tx.send(ContextCommand::AudioChunk(samples));

                                if audio_buffer.len() >= last_processed_len + PROCESS_STEP {
                                    if let Ok(_) = whisper_state.full(params.clone(), &audio_buffer) {
                                        let draft_text = collect_segments(&mut whisper_state);
                                        if !draft_text.is_empty() {
                                            // Real-time localized backspacing injection
                                            injector.inject_draft(&draft_text);
                                            last_processed_len = audio_buffer.len();
                                        }
                                    }
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout)      => (),
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        }

                        let current_state = {
                            let s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                            s.current_state
                        };
                        if current_state == VakhState::Idle || current_state == VakhState::Flushing {
                            break;
                        }
                    }

                    // --- STOP SEQUENCE: Finalizing (Polish) ---
                    let _ = handle_for_thread.emit("vakh-status", AudioStatus::Finalizing);

                    let (reply_tx, reply_rx) = channel::<String>();
                    let _ = ctx_tx.send(ContextCommand::Finalize(reply_tx));
                    
                    if let Ok(final_text) = reply_rx.recv_timeout(std::time::Duration::from_secs(15)) {
                        if !final_text.is_empty() {
                            // Final polished text replaces the draft
                            injector.inject_draft(&final_text);
                            injector.commit();

                            let _ = db_for_thread.log_dictation(
                                &final_text,
                                Some(&target_app_name),
                            );
                        } else {
                            injector.commit();
                        }
                    }

                    let mut s = state_for_thread.lock().unwrap_or_else(|e| e.into_inner());
                    s.transition_to(VakhState::Idle);
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
            drop(stream);
        }
        app_state.transition_to(VakhState::Idle);
        let _ = app_handle.emit("vakh-status", AudioStatus::Idle);
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
            // Only filter if it's a known short-form hallucination and exactly matches
            if cleaned.len() < 10 && HALLUCINATIONS.iter().any(|h| lower == *h) { continue; }
            
            // Check if segment actually contains characters
            if !cleaned.chars().any(|c| c.is_alphanumeric()) { continue; }
            
            segments.push(cleaned.to_string());
        }
    }

    let mut result = segments.join(" ");
    if result.trim().is_empty() { return String::new(); }
    
    // Capitalize first letter
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(Mutex::new(AppState::new()));
    let config    = AppConfig::load();
    let database  = Arc::new(Database::init().expect("failed to init db"));

    println!("[Whisper] Loading model...");
    let model_data = include_bytes!("../tiny.en.bin");
    let ctx = Arc::new(
        WhisperContext::new_from_buffer_with_params(
            model_data,
            WhisperContextParameters::default(),
        )
        .expect("failed to load embedded model"),
    );
    println!("[Whisper] Model loaded successfully");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let state_for_hooks  = Arc::clone(&app_state);
            let db_for_hooks     = Arc::clone(&database);
            let ctx_for_hooks    = Arc::clone(&ctx);
            let config_for_hooks = config.clone();
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
                    // Give the OS a moment to settle the window size
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    
                    if let Ok(Some(monitor)) = window_clone.current_monitor() {
                        let screen_size = monitor.size();
                        let scale_factor = monitor.scale_factor();
                        
                        if let Ok(window_size) = window_clone.outer_size() {
                            let x = (screen_size.width as i32 - window_size.width as i32) / 2;
                            // Move it slightly higher (150px) to avoid taskbar overlap on some Win10 setups
                            let y = screen_size.height as i32 - window_size.height as i32 - (150.0 * scale_factor) as i32;
                            
                            println!("[Window] Screen: {}x{}, Window: {}x{}, Position: {},{}", 
                                screen_size.width, screen_size.height, window_size.width, window_size.height, x, y);

                            let _ = window_clone.set_position(tauri::PhysicalPosition { x, y });
                            let _ = window_clone.show();
                            let _ = window_clone.set_focus();
                        }
                    }
                });
            }

            app.manage(AppManagedState(
                Arc::clone(&app_state),
                Arc::clone(&database),
                Arc::clone(&ctx),
                config.clone(),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![toggle_listening, hide_window])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
