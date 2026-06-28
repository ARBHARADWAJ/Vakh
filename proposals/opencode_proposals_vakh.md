# VAKH v3 Feature Proposals

*Generated from opencode planning session — Windows-first, general productivity + multilingual + accessibility focus*

---

## 🎯 Retention & Engagement (Core Priority)

| Feature | Why it retains users | Effort |
|---------|---------------------|--------|
| **Personal Vocabulary / Custom Words** | Learns user's names, technical terms, acronyms → accuracy improves over time | Medium |
| **Voice Commands (Beyond Dictation)** | "New line", "Delete that", "Capitalize", "Bold" → hands-free editing | Medium |
| **Smart Formatting** | Auto-punctuation, capitalization, number formatting → less post-edit cleanup | Medium |
| **Dictation Sessions & Streaks** | Daily goals, streak counter, session history → habit formation | Low |
| **Export/Integration** | One-click export to Notion, Obsidian, Word, VS Code → workflow stickiness | Medium |
| **Multilingual: Language Switch Hotkey** | `Ctrl+Shift+L` cycles languages → critical for code-switching users | Low |
| **Accessibility: Screen Reader Announcements** | Live region updates for state changes → WCAG compliance | Low |

---

## 🔧 CPU/Memory Optimization (Architectural — Preventive)

| Optimization | Impact | Implementation |
|--------------|--------|----------------|
| **Model Lazy-Load + Cache** | Cold start < 500ms; keep model warm in memory | Load `tiny.en` on first hotkey, keep `WhisperContext` alive |
| **Audio Buffer Pooling** | Zero GC pressure during 5-min sessions | Pre-allocate `Vec<f32>` rings; reuse via `clear()` |
| **Thread Pool for Whisper** | Configurable threads don't starve UI/injection | `rayon::ThreadPoolBuilder` scoped to AI worker |
| **VAD Cooldown After Finalize** | Prevents mic reopen thrash on rapid toggle | 500ms debounce in `perform_state_transition` |
| **Memory-Mapped Model (Optional)** | Faster load, shared across processes | `mmap` the `.bin`; `WhisperContext::new_from_buffer` supports it |

---

## 🫧 The "Small Bubble" (Orb UI) — Polish & Utility

| Enhancement | Details |
|-------------|---------|
| **Compact Mode** | 40px diameter; shows only waveform + mic state; expands on hover |
| **Contextual Tooltip** | Hover → shows: current language, target app, session duration, word count |
| **Quick Actions Ring** | Right-click orb → Language picker, Pause/Resume, Open Dashboard, Quit |
| **Draggable Snap Zones** | Snap to screen edges/corners; remember position per monitor |
| **Per-App Visual Hint** | Orb accent color matches target app (Code=blue, Word=blue, Browser=green) |
| **Accessibility: High-Contrast Mode** | Toggle in settings; thicker waveform, larger hit targets |

---

## 🌐 Multilingual Foundation (v3 Core)

1. **Embedded Multi-Model Support** — Ship `tiny.en`, `tiny.es`, `tiny.fr`, `tiny.de`, `tiny.zh`, `tiny.hi` (each ~39MB). Load on-demand.
2. **Language Config Per-App** — Remember: "Code → English", "WhatsApp → Hindi", "Word → Auto"
3. **Auto-Detect with Whisper** — Use `language="auto"` + first 30s to detect, then lock
4. **UI Language Selector in Orb** — Quick-switch without opening dashboard

---

## ♿ Accessibility-First Features

| Feature | Implementation |
|---------|----------------|
| **Live Region Announcements** | `aria-live="polite"` on status text; announce "Listening", "Processing", "Finalized" |
| **Keyboard-Navigable Orb** | `Tab` to focus orb; `Space` toggles; `Esc` hides; arrow keys navigate quick actions |
| **Reduced Motion** | Respect `prefers-reduced-motion`; disable waveform animation, fade transitions |
| **High Contrast Themes** | WCAG AA compliant color tokens in CSS custom properties |
| **Audio Cues (Optional)** | Subtle beep on start/stop; configurable per user preference |

---

## 📊 Gap Identification & Insights (Dashboard v3)

| Insight | Value |
|---------|-------|
| **Accuracy Proxy** | Track: corrections made (backspace count), re-dictation rate → "Accuracy Score" |
| **App-Specific Stats** | Words/min per target app; error rate per app |
| **Language Distribution** | % sessions per language → shows multilingual usage |
| **Peak Usage Heatmap** | Hour/day grid → optimal dictation times |
| **Vocabulary Growth** | New custom words added over time |

---

## 🏗️ Architectural Hardening (Stability — "Don't Bust")

1. **Structured Error Boundaries** — `Result<T, VakhError>` everywhere; crash reporter UI
2. **Watchdog Timer** — If AI worker hangs > 60s, force-finalize + recover gracefully
3. **Config Validation** — Schema validation on load; migrate old configs automatically
4. **Database WAL Mode + Integrity Check** — `PRAGMA journal_mode=WAL; PRAGMA integrity_check`
5. **Hotkey Guard** — Ignore double-Ctrl if already processing; prevent double-spawn
6. **Integration Tests** — `cargo test --test integration` with mocked audio + injection

---

## 📦 Suggested v3 Milestones

| Milestone | Scope | Target |
|-----------|-------|--------|
| **M1: Foundation** | Multi-model loader, buffer pooling, error boundaries, watchdog | Week 1-2 |
| **M2: Orb Polish** | Compact mode, quick actions, snap zones, high-contrast | Week 2-3 |
| **M3: Multilingual** | Per-app language memory, auto-detect, hotkey switch | Week 3-4 |
| **M4: Voice Commands** | "New line", "Delete", "Capitalize", custom phrases | Week 4-5 |
| **M5: Accessibility** | Screen reader, keyboard nav, reduced motion, audio cues | Week 5-6 |
| **M6: Insights Dashboard** | Accuracy proxy, heatmap, vocabulary growth | Week 6-7 |
| **M7: Harden & Ship** | Integration tests, config migration, crash reporter, MSI | Week 7-8 |

---

## ❓ Quick Decisions Needed

1. **Model Strategy**: Ship all 6 tiny models (~230MB) or download on-demand?
2. **Voice Commands**: Hardcoded set vs user-extensible phrase map?
3. **Telemetry**: Local-only analytics (SQLite) or opt-in cloud sync for multi-device?
4. **Installer**: Keep MSI or add winget/scoop/Chocolatey manifests?

---

*Ready to proceed. Want me to elaborate on any milestone or start drafting M1 implementation plan?*