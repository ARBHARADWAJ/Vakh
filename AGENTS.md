# VAKH — AGENTS.md

## Project Overview
Offline AI dictation desktop app (Windows). Tauri 2 + Rust backend, vanilla HTML/JS/CSS frontend. Uses embedded `whisper-rs` (tiny.en model) for local speech-to-text. Global hotkey: double-tap Left/Right Ctrl.

## Architecture
```
src-tauri/           # Rust backend (Tauri 2)
  src/
    lib.rs           # Main logic: state machine, audio pipeline, Whisper worker
    main.rs          # Entry point (windows_subsystem = "windows")
    audio.rs         # cpal + WebRTC VAD processing (16kHz resample)
    hooks.rs         # rdev global hotkey + Win32 window targeting
    db.rs            # SQLite (rusqlite) at %LOCALAPPDATA%\VAKH\vakh_history.db
    config.rs        # JSON config at %LOCALAPPDATA%\VAKH\config.json
    state.rs         # AppState, VakhState (Idle/Listening/Processing/Flushing)
    injection.rs     # Win32 SendInput text injection
src/                 # Frontend (served from ../src per tauri.conf.json)
  index.html         # Main capsule UI (240x48, transparent, always-on-top)
  main.js            # Capsule UI logic
  dashboard.html     # Dashboard window (hidden by default)
  dashboard.js       # Dashboard tabs: overview, history, settings, themes
```

## Key Commands
| Action | Command |
|--------|---------|
| Install deps | `npm install` |
| Dev (Tauri) | `npm run tauri dev` |
| Build release | `npm run tauri build` |
| Cargo check | `cd src-tauri && cargo check` |
| Cargo test | `cd src-tauri && cargo test` |
| Format Rust | `cd src-tauri && cargo fmt` |
| Lint Rust | `cd src-tauri && cargo clippy` |

## State Machine (lib.rs)
`VakhState`: `Idle` → (double Ctrl) → `Listening` → (speech) → `Processing` → (finalize) → `Flushing` → `Idle`

Two audio channels:
- `tx1` (VAD-gated) → Main thread for UI level visualization
- `tx2` (has_speech) → AI Worker thread for Whisper transcription

Worker thread does periodic 20s transcription + final flush on `ContextCommand::Finalize`.

## Tauri Commands (invoke from frontend)
- `toggle_listening` — triggers state transition
- `get_config` / `save_config` — JSON config (VAD, typing delays, theme, opacity)
- `get_dictation_logs` / `delete_log` / `clear_logs` / `get_stats` — history
- `open_dashboard` / `hide_dashboard` / `minimize_dashboard` / `hide_window` / `start_dragging`

## Events (backend → frontend)
- `vakh-show`, `vakh-hide`, `vakh-start-listening`
- `vakh-status` — `{status: "idle"|"listening"|"active"|"warning"|"finalizing"|"processing", level?, duration?}`
- `vakh-config-changed` — full config object

## Platform Specifics
- **Windows only**: Uses `windows-sys` crate for `SendInput`, `GetForegroundWindow`, process enumeration
- Model embedded: `include_bytes!("../tiny.en.bin")` in `lib.rs:433`
- Database & config stored in `%LOCALAPPDATA%\VAKH\`
- Auto-cleans logs older than 30 days (db.rs:40-43)

## Frontend Notes
- No build step — vanilla ES modules served directly
- `data-tauri-drag-region` on `#app` in index.html for window dragging
- Dashboard uses CSS custom properties for theming (`.theme-default`, `.theme-blue`, etc.)
- Waveform animation driven by `vakh-status.level` events

## Common Gotchas
1. **Rust version**: Requires Rust 2021 edition (Cargo.toml)
2. **Tauri 2**: Uses `tauri::Builder::default()` + `.invoke_handler(tauri::generate_handler![...])`
3. **Audio sample rate**: Resampled to 16kHz for VAD + Whisper (audio.rs:127)
4. **VAD warmup**: First ~1s ignored (audio.rs:161-166)
5. **Noise gate**: RMS < 0.008 suppresses VAD (audio.rs:171-176)
6. **No CSP**: `"csp": null` in tauri.conf.json — required for inline styles in dashboard
7. **Single binary**: `crate-type = ["staticlib", "cdylib", "rlib"]` for Tauri

## Testing
No automated test suite exists. Manual verification:
- `npm run tauri dev` → test hotkey, dictation, dashboard
- Check `%LOCALAPPDATA%\VAKH\` for config.json and vakh_history.db
