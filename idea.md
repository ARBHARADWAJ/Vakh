"VAKH" is a brilliant name. In Sanskrit and many Indian languages, it translates to "speech" or "voice," which fits the core identity of this application perfectly.

Here is the finalized, highly detailed Product Requirements Document (PRD) for VAKH. This is formatted exactly how an AI coding assistant needs to see it to generate flawless Rust architecture. You can copy and paste this entire block.

---

# System Architecture & PRD: VAKH (Native Windows AI Dictation)

**Application Name:** VAKH
**Project Type:** Standalone OS-level Desktop Application
**Primary Target:** Windows 11 (Optimized for low-spec, CPU-only machines)
**Core Functionality:** Real-time, highly accurate, completely local speech-to-text dictation that types directly into any active Windows application.

### 1. Technology Stack

* **Core Language:** Rust (chosen for memory safety, OS-level hooks, multithreading, and performance).
* **AI Engine:** `whisper.cpp` (C/C++ port for CPU-bound, Python-free execution).
* **Model:** Whisper `tiny.en` (~75MB). Must be baked directly into the executable at compile time using Rust's `include_bytes!` macro.
* **Database:** SQLite (via `rusqlite` crate) for local, native storage.
* **Distribution:** Single, zero-dependency standalone `.exe` file (~80MB total size).

### 2. Core Engine Rules & State Machine

The VAKH engine must strictly follow this State Machine to prevent memory leaks, audio buffer overflows, and CPU hogging.

| State | Trigger | Action |
| --- | --- | --- |
| **IDLE** | Application launch / Finished flushing. | Consumes near 0% CPU. A global listener waits quietly for the `Double Ctrl` hotkey. |
| **LISTENING** | User presses `Double Ctrl`. | Captures target window `HWND`. Opens shared microphone stream. Activates Smart VAD. |
| **PROCESSING** | VAD detects human speech. | Runs a 4-second sliding window of audio chunks through the `whisper.cpp` engine. |
| **FLUSHING** | 4s of absolute silence OR 5-min hard limit. | Injects final pending text, writes to SQLite, strictly clears audio Vectors, returns to IDLE. |

### 3. Audio & Memory Pipeline

* **VAD Specification:** Must implement a Smart VAD (e.g., WebRTC VAD) to filter out mechanical keyboard clicks, mouse clicks, and background fan noise. Simple audio volume thresholds are strictly prohibited.
* **Audio Handling:** Capture raw Windows microphone audio and downsample on-the-fly to 16kHz, 16-bit Mono (the strict requirement for `whisper.cpp`).
* **Multithreading:** The system must use separate threads for:
1. OS Hotkey listening.
2. Continuous audio capture.
3. AI processing (`whisper.cpp`).
*Note: The audio array must strictly remain sequential and must NOT be split for parallel processing to preserve context.*



### 4. Text Injection & OS Interaction

* **Window Locking:** Upon `Double Ctrl` activation, VAKH immediately captures the active Window Handle (`HWND`). All generated keystrokes are hard-routed to this specific window, even if the user clicks away to a different application or monitor mid-dictation.
* **Draft vs. Committed Buffer Logic:**
* **Draft Buffer:** Unfinalized text from the 4-second sliding window. VAKH uses rapid simulated `Backspace` keystrokes to delete and overwrite the draft text dynamically as the AI gains more context from the user's ongoing speech.
* **Committed Buffer:** Finalized text (triggered by natural speech pauses). Once text is committed, VAKH will never backspace over it.


* **User Input Override:** The application assumes the user will not manually type on the physical keyboard while dictating.

### 5. Local Database Schema (SQLite)

* **File Location:** Stored securely in the Windows `%AppData%` directory (e.g., `vakh_history.db`).
* **Auto-Cleaning Protocol:** On every application launch, VAKH must automatically execute a `DELETE` query for any rows where the timestamp is older than 30 days to prevent infinite file growth.

**Schema (Table: `dictation_logs`):**

| Column Name | Data Type | Description |
| --- | --- | --- |
| `id` | INTEGER | Primary Key, Auto-increment. |
| `timestamp` | DATETIME | Exact OS time the dictation was flushed to the screen. |
| `transcribed_text` | TEXT | The final, committed string of text. |
| `target_app` | TEXT | The executable name of the locked `HWND` (e.g., `chrome.exe`, `Code.exe`). |

---

### The Next Crucial Question: UI/UX Design

UI/UX Design Specification: VAKH Frontend

Project Phase: Frontend Implementation (Tauri + HTML/CSS/JS)
Design Language: "macOS Pro / Oily Glass" (Dark, Sleek, Translucent)

1. Core Visual Concept

VAKH operates as a floating, frameless window over the user's OS. The UI must feel like a physical piece of premium dark glass resting on the screen. It avoids harsh gradients and pure blacks, favoring strict 0.6 opacity translucency, subtle rim lighting, and vibrant state-based accents.

2. Color Palette & Typography

Base Glass Color: rgba(20, 20, 25, 0.6)

Button Backgrounds: rgba(255, 255, 255, 0.05)

Primary Active Color (Listening): Apple System Blue #0a84ff

Stop Action Color: Apple System Red #ff453a

Resume/Play Action Color: Apple System Green #32d74b

Inactive/Paused Text Color: Apple Secondary Text #8e8e93 / #636366

Typography: System native sans-serif (-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif).

3. Global Window Setup (Tauri Configuration)

Window Properties: The OS-level window must be configured in Tauri as transparent: true, decorations: false, and alwaysOnTop: true.

Body Background: The <body> background must be completely transparent so only the VAKH widget is visible.

4. Component Specifications

A. The Main Container (The Glass Rectangle)

Shape: border-radius: 24px (A modern rounded rectangle, strictly NOT a pill).

Padding & Spacing: padding: 16px 24px with a flex gap: 28px between internal elements.

Material (The Oily Glass Effect):

Background: rgba(20, 20, 25, 0.6)

Blur: backdrop-filter: blur(8px)

Lighting (Box Shadows):

Drop Shadow: 0 24px 48px rgba(0, 0, 0, 0.4) (gives it physical height).

Top Rim Light: inset 0 1px 1px rgba(255, 255, 255, 0.08) (simulates glass reflection).

Inner Border: inset 0 0 0 1px rgba(255, 255, 255, 0.03) (crisp edges).

B. Interactive Buttons (Close & Toggle)

Dimensions: 48px width, 48px height, border-radius: 50% (Perfect circles).

Base State: Flat, translucent background rgba(255, 255, 255, 0.05).

Icons: Scaled to 22px width/height. Must use SVG paths with stroke-width: 2.2, no fill (except for the stop square).

Interactions:

Hover: Background brightens to rgba(255, 255, 255, 0.12).

Active (Click): Scales down to transform: scale(0.95) for physical feedback.

Close Button Specific: On hover, the icon turns #ff453a and the background takes on a subtle red tint rgba(255, 69, 58, 0.1).

C. The Audio Waveform (Center Graphic)

Container Dimensions: 64px total width, 48px max height. Flex layout with gap: 6px.

Bars: 5 individual <div class="bar"> elements.

Bar Styling: 6px width, border-radius: 6px.

5. State Management Definitions

The UI operates in two strict visual states controlled by a CSS class on the main wrapper.

State 1: is-listening (Active Capture)

Waveform:

Color: #0a84ff

Glow: box-shadow: 0 0 16px rgba(10, 132, 255, 0.6)

Animation: CSS infinite bounce (transform: scaleY). Staggered animation delays for the 5 bars (0.0s, 0.15s, 0.3s, 0.45s, 0.6s).

Right Toggle Button: Displays a square Stop icon. Icon color is #ff453a (Red).

Status Text: Positioned underneath the widget. Reads "LISTENING" in uppercase, heavily tracked (letter-spacing: 0.15em), sized at 0.65rem, colored #0a84ff.

State 2: is-paused (Idle / Flushed)

Waveform:

All bars flatten to exactly height: 6px.

Animation stops completely.

Color changes to flat #636366 (No glow/box-shadow).

Right Toggle Button: Displays a Microphone or Play icon. Icon color is #32d74b (Green).

Status Text: Reads "PAUSED". Color changes to #8e8e93.

6. Animation & Motion Design

Easing: All transitions (color changes, bar height changes, hover states) must use Apple's signature spring-like bezier curve: transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1).

Dismissal: When the Close button is clicked, the entire widget should smoothly scale down transform: scale(0.9) and fade to opacity: 0 before the OS window is destroyed.






### 6. Development & LLM Guidelines

* **Model Usage:** Use Pro version models for coding tasks and Lite models for explanations to maximize context and cost efficiency.

==========================================================

tasks:

mention tasks here:
task task_no [ yes and no symbols]: task name
