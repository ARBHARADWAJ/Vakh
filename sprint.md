# VAKH Development Sprint Tracker

This file tracks the real-time progress of the VAKH project.

## Project Guidelines
- **Model Usage:** Use Pro version models for coding tasks and Lite models for explanations to maximize context and cost efficiency.

## Project Flow & Status

### Phase 1: Infrastructure & Database
- [✅] **Task 1.1: SQLite Database Setup**
  - Created `db.rs` with `rusqlite`.
  - Implemented `dictation_logs` table.
  - Implemented 30-day auto-cleaning protocol.
- [✅] **Task 1.2: State Machine Scaffolding**
  - Defined `VakhState` (Idle, Listening, Processing, Flushing).
  - Implemented `AppState` for managed state.

### Phase 2: UI/UX Frontend (Tauri)
- [✅] **Task 2.1: Glass UI Implementation**
  - Designed "macOS Pro / Oily Glass" rectangle UI.
  - Implemented animated audio waveform.
  - Configured transparent, frameless, "Always on Top" window.
- [✅] **Task 2.2: Frontend-Backend Bridge**
  - Connected UI buttons to Rust `tauri::command`.
  - Implemented state-based visual updates (Listening vs Paused).

### Phase 3: OS Integration & Hooks
- [✅] **Task 3.1: Global Hotkey Listener**
  - Implemented `rdev` background listener.
  - Logic for `Double Ctrl` detection (400ms window).
- [✅] **Task 3.2: Window Handle (HWND) Capture**
  - Integrated `windows-sys` to capture the foreground window on activation.
- [✅] **Task 3.3: Keystroke Injection Engine**
  - [✅] Logic for draft backspacing vs. committing created in `injection.rs`.
  - [✅] Integration into the main processing loop.

### Phase 4: Audio Processing Pipeline
- [✅] **Task 4.1: Raw Audio Capture**
  - Integrated `cpal` for native microphone access.
- [✅] **Task 4.2: Resampling Engine**
  - Implemented on-the-fly downsampling to 16kHz Mono.
- [✅] **Task 4.3: Smart VAD (Voice Activity Detection)**
  - Integrated `webrtc-vad` with "Quality" mode (less aggressive).
  - Fixed sample rate mismatch in VAD initialization.

### Phase 5: AI Engine (Whisper)
- [✅] **Task 5.1: Model Embedding**
  - Baked `tiny.en` model into binary. Optimized with `Arc<WhisperContext>` pre-loading at startup.
- [✅] **Task 5.2: Whisper-RS Integration**
  - Implemented 1.5s processing window for natural transcription rhythm.
  - **BREAKTHROUGH:** Added "Smart Statement Validation" (Auto-commit on punctuation `.?!`).
- [✅] **Task 5.3: Real-time Transcription**
  - **BREAKTHROUGH:** "Smart Diff" Injection Engine. Only backspaces changed characters.
  - Dramatically reduced UI flicker and "reloading" lag.

### Phase 6: UX Polish & Refinements
- [✅] **Task 6.1: Target App Identification**
  - Integrated `windows-sys` to extract process names from HWND.
  - Updated `dictation_logs` to store the target application name.
- [✅] **Task 6.2: Window Draggability**
  - Added `data-tauri-drag-region` to the UI for better window management.

### Phase 7: Stability & Error Handling
- [✅] **Task 7.1: Mutex & Poisoning Safety**
  - Replaced all `.lock().unwrap()` with safe `unwrap_or_else` recovery.
- [✅] **Task 7.2: Graceful Initialization**
  - Refactored `db.rs` and `lib.rs` to handle initialization errors without panicking.
- [✅] **Task 7.3: Window & Monitor Robustness**
  - Replaced `.unwrap()` in Tauri setup with safe monitor detection logic.
- [✅] **Task 7.4: Warning Cleanup**
  - Resolved `dead_code` warnings and improved code cleanliness.

### Phase 8: Injection & AI Diagnostics
- [✅] **Task 8.1: Deep Window Routing**
  - Implemented `GetGUIThreadInfo` to target focused child windows.
- [✅] **Task 8.2: Injection Verbosity**
  - Added real-time logging for every character and backspace sent via Win32.
- [✅] **Task 8.3: AI Sensitivity Tuning**
  - Increased processing window to 1.0s to improve transcription context.
- [✅] **Task 8.4: Win32 Message Refinement**
  - Migrated from `PostMessageW` to `SendInput` for universal Windows compatibility (Notepad, Chrome, etc.).
- [✅] **Task 8.5: Target HWND Validation**
  - Refined foreground window capture to ignore VAKH window itself.
- [✅] **Task 8.6: Global Key Injection**
  - Verified injection works across all standard Windows inputs.

---

## Development Log & Updates

### Saturday, 16 May 2026 (Today)

**17:45 - Production Readiness & 5-Minute Sessions**
- [x] **Duration Extension:** Increased max session audio limit from 3 minutes to 5 minutes (300s).
- [x] **Rolling Corrections:** Implemented Thread 2 "Rolling Engine" that provides high-accuracy corrections every 3 seconds during live dictation.
- [x] **Injection Safety:** Removed `select_all` from finalization to prevent wiping out user documents.
- [x] **Accuracy Tuning:** Balanced `PROCESS_STEP` to 0.75s to prevent "half-word" slicing.
- [x] **UI Polish:** Implemented Green/Blue state transitions based on live audio level thresholds.
- [x] **Licensing:** Added MIT License and comprehensive `README.md`.

### Thursday, 14 May 2026

**18:15 - Externalized Config & Robust Injection**
- **Change:** Implemented file-based configuration (`config.json`) and auto-focus for text injection.
- **Impact:**
  - Users can now customize `language` and `threads` via JSON.
  - `TextInjector` now uses `SetForegroundWindow` to ensure text is sent to the correct application.
  - Improved UI feedback with structured `AudioStatus` enum.

**18:45 - Hallucination Filtering & VAD Gating**
- **Issue:** Whisper was outputting junk phrases during silence.
- **Change:** 
  - Switched VAD to `Aggressive` mode.
  - Implemented a "Hard Gate": Audio frames only sent to AI when voice is detected.
  - Expanded hallucination blocklist.

### Tuesday, 12 May 2026

**16:30 - Audio Duration & Transcription Clarity**
- **Change:** Increased audio buffer safety limit and switched to Beam Search for better AI recognition.
- **Impact:** 
  - Increased max recording window.
  - Improved transcription quality (beam_size: 5).

### Sunday, 10 May 2026

**21:45 - Engine Latency & UI Fixes**
- **Change:** Reduced Whisper processing step from 1000ms to 250ms.
- **22:10 - Window Visibility Toggle:** Integrated `vakh-show`/`vakh-hide` events.
- **22:45 - Hard-Routed Injection:** Migrated to direct Win32 routing to captured `HWND`.

---
**Current Status:** All core features implemented. Final stability and accuracy tuning completed.
**Next Steps:** Distribute production build to users.

## [0.2.0] - 2026-05-18
- **Audio Overhaul:** Fixed VAD stripping natural pauses; AI now receives continuous speech for higher accuracy.
- **Timing Update:** Thread 1 now waits 5 seconds before the first draft, then updates every 1 second.
- **Correction Logic:** Thread 2 (Context Sweep) now triggers only upon detected silence or session end.
- **Duplication Fix:** Synchronized the Text Injector to prevent premature commits and duplicate text during corrections.
- **Version Bump:** Application version bumped to 0.2.0.

## [0.2.4] - 2026-05-18
- **Live Transcribe Engine:** Implemented high-speed 1-second update cycle with localized backspacing for real-time word refinement.
- **Auto-Pause:** The app now automatically stops and transitions to 'PAUSED' (Idle) after 2.5 seconds of silence.
- **Sentence Finalization:** Upon Auto-Pause, Thread 2 performs a single high-accuracy sweep on the complete sentence to provide a final polished version.
- **Hallucination Control:** By scoped dictation to individual sentences (via Auto-Pause), long-term AI loops are effectively eliminated.

## [0.2.5] - 2026-05-18 (Git Integration)
- **Git Repository:** Initialized a local Git repository to manage version control.
- **Branching Strategy:** Created 'master' for complete code and 'feature/v0.2.5-live-core' for core logic focus.
- **Version Restoration:** Verified v0.2.5 as the primary working version with continuous context and 5s Auto-Pause.
- **Fix Analysis:** Documented the failure of v0.2.3 'buffer wipe' and the move to v0.2.5 'Rolling Window'.
