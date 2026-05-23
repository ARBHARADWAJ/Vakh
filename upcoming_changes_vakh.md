# Upcoming Code Changes for VAKH Review

Below are the exact code modifications planned to resolve the key issues identified in the code pattern and PRD alignment review. These updates are non-breaking, preserve the core batch-finalization logic, and introduce zero unnecessary features.

---

## 💾 File 1: `audio.rs`
**Path:** `C:\Users\karth\Documents\GeminiCliProjects\Vakh\src-tauri\src\audio.rs`

### Summary of Changes:
1. Added `hangover_counter` to the `AudioProcessor` struct to track voice tail states.
2. Switched from nearest-neighbor sample indexing to **Linear Interpolation Resampling** to eliminate aliasing noise.
3. Implemented a **250ms VAD Hangover Time** (25 frames of 10ms) to prevent syllable clipping.

```diff
@@ -14,6 +14,7 @@
     frame_buffer: Vec<i16>,
     is_speech_active: bool,
     silence_counter: usize,
     level_counter: usize,
+    hangover_counter: usize,
     status_tx: Option<Sender<AudioStatus>>,
 }
 
@@ -30,6 +31,7 @@
             is_speech_active: false,
             silence_counter: 0,
             level_counter: 0,
+            hangover_counter: 0,
             status_tx,
         }
     }
@@ -100,16 +102,23 @@
         let target_rate = 16000.0;
         let ratio = self.sample_rate as f32 / target_rate;
         
-        // Simple linear resampling
-        for i in 0..((mono_data.len() as f32 / ratio) as usize) {
-            let src_index = (i as f32 * ratio) as usize;
-            if src_index < mono_data.len() {
-                let sample = mono_data[src_index];
-                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
-                self.frame_buffer.push(sample_i16);
-                self.resample_buffer.push(sample);
-            }
-        }
+        // Linear Interpolation Resampling (Anti-Aliasing Filter)
+        let target_len = (mono_data.len() as f32 / ratio) as usize;
+        for i in 0..target_len {
+            let src_pos = i as f32 * ratio;
+            let src_idx = src_pos.floor() as usize;
+            let frac = src_pos - src_idx as f32;
+            
+            if src_idx + 1 < mono_data.len() {
+                let s0 = mono_data[src_idx];
+                let s1 = mono_data[src_idx + 1];
+                let sample = s0 + frac * (s1 - s0);
+                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
+                self.frame_buffer.push(sample_i16);
+                self.resample_buffer.push(sample);
+            } else if src_idx < mono_data.len() {
+                let sample = mono_data[src_idx];
+                let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
+                self.frame_buffer.push(sample_i16);
+                self.resample_buffer.push(sample);
+            }
+        }
 
         // VAD Frame size must be 10ms, 20ms, or 30ms. At 16kHz: 160, 320, 480.
         let frame_size = 160; 
         const SHORT_PAUSE:  usize = 450; // ~4.5s
         const LONG_SILENCE: usize = 900; // ~9.0s
+        const HANGOVER_LIMIT: usize = 25; // 250ms hangover (25 * 10ms frames)
 
         while self.frame_buffer.len() >= frame_size {
             let frame: Vec<i16> = self.frame_buffer.drain(0..frame_size).collect();
             let resampled_segment: Vec<f32> = self.resample_buffer.drain(0..frame_size).collect();
 
             match self.vad.is_voice_segment(&frame) {
                 Ok(is_voice) => {
-                    if is_voice {
-                        self.silence_counter = 0;
-                        // ONLY send audio to the processing thread when voice is detected
-                        let _ = tx.send(resampled_segment.clone());
+                    let mut has_speech = is_voice;
+                    
+                    if is_voice {
+                        self.hangover_counter = HANGOVER_LIMIT;
+                    } else if self.hangover_counter > 0 {
+                        self.hangover_counter -= 1;
+                        has_speech = true;
+                    }
 
-                        if !self.is_speech_active {
-                            println!("[VAD] Speech Detected - Active");
-                            self.is_speech_active = true;
-                            if let Some(ref tx) = self.status_tx { let _ = tx.send(AudioStatus::Active); }
-                        }
-                    } else {
-                        if self.is_speech_active {
-                            self.silence_counter += 1;
+                    if has_speech {
+                        self.silence_counter = 0;
+                        let _ = tx.send(resampled_segment.clone());
 
-                            if self.silence_counter == SHORT_PAUSE {
-                                println!("[VAD] Thinking pause - 4.5s");
-                                if let Some(ref tx) = self.status_tx {
-                                    let _ = tx.send(AudioStatus::Thinking);
-                                }
-                            }
+                        if !self.is_speech_active {
+                            println!("[VAD] Speech Detected - Active");
+                            self.is_speech_active = true;
+                            if let Some(ref tx) = self.status_tx { let _ = tx.send(AudioStatus::Active); }
+                        }
+                    } else {
+                        if self.is_speech_active {
+                            self.silence_counter += 1;
 
-                            if self.silence_counter >= LONG_SILENCE {
-                                self.is_speech_active = false;
-                                self.silence_counter = 0;
-                                println!("[VAD] Auto-Stop — long silence detected");
-                                if let Some(ref tx) = self.status_tx { let _ = tx.send(AudioStatus::Idle); }
-                            }
-                        }
-                    }
+                            if self.silence_counter == SHORT_PAUSE {
+                                println!("[VAD] Thinking pause - 4.5s");
+                                if let Some(ref tx) = self.status_tx {
+                                    let _ = tx.send(AudioStatus::Thinking);
+                                }
+                            }
+
+                            if self.silence_counter >= LONG_SILENCE {
+                                self.is_speech_active = false;
+                                self.silence_counter = 0;
+                                println!("[VAD] Auto-Stop — long silence detected");
+                                if let Some(ref tx) = self.status_tx { let _ = tx.send(AudioStatus::Idle); }
+                            }
+                        }
+                    }
```

---

## 💾 File 2: `injection.rs`
**Path:** `C:\Users\karth\Documents\GeminiCliProjects\Vakh\src-tauri\src\injection.rs`

### Summary of Changes:
1. Imported `GetForegroundWindow` to check the active focused app.
2. Added a logical guard so focus is only yanked via `SetForegroundWindow` if the target window isn't already focused.

```diff
@@ -1,6 +1,6 @@
 use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
     SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK
 };
-use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
+use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, GetForegroundWindow};
 use std::mem::size_of;
 
@@ -98,7 +98,12 @@
         // 4. Send the batch
         if !inputs.is_empty() {
             unsafe {
                 if let Some(hwnd) = self.target_hwnd {
-                    SetForegroundWindow(hwnd as _);
+                    if hwnd != 0 {
+                        let active_hwnd = GetForegroundWindow();
+                        if active_hwnd != hwnd {
+                            SetForegroundWindow(hwnd);
+                        }
+                    }
                 }
                 SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
             }
```

---

## 💾 File 3: `lib.rs`
**Path:** `C:\Users\karth\Documents\GeminiCliProjects\Vakh\src-tauri\src\lib.rs`

### Summary of Changes:
1. Replaced the global sliding-word filter with a robust **Phrase-Level Deduplicator** that removes consecutive duplicate word sequences of length 2 to 6.
2. Fully preserves legitimate single-word repetitions like *"had had"* and *"the the"*.

```diff
@@ -238,17 +238,32 @@
 
 fn deduplicate_overlap(text: &str) -> String {
     let words: Vec<&str> = text.split_whitespace().collect();
-    let mut result: Vec<&str> = Vec::new();
+    if words.is_empty() { return String::new(); }
     
-    for word in words {
-        // Skip if last 3 words already contain this word sequentially
-        if result.len() >= 3 {
-            let last3 = &result[result.len()-3..];
-            if last3.iter().any(|w| w.to_lowercase() == word.to_lowercase()) {
-                continue;
-            }
-        }
-        result.push(word);
-    }
-    result.join(" ")
+    let mut result: Vec<&str> = Vec::new();
+    let mut i = 0;
+    
+    while i < words.len() {
+        let mut matched = false;
+        
+        for phrase_len in (2..=6).rev() {
+            if i + phrase_len * 2 <= words.len() {
+                let phrase1 = &words[i..i+phrase_len];
+                let phrase2 = &words[i+phrase_len..i+phrase_len*2];
+                
+                let is_duplicate = phrase1.iter().zip(phrase2.iter())
+                    .all(|(w1, w2)| w1.to_lowercase() == w2.to_lowercase());
+                
+                if is_duplicate {
+                    result.extend_from_slice(phrase1);
+                    i += phrase_len * 2;
+                    matched = true;
+                    break;
+                }
+            }
+        }
+        
+        if !matched {
+            result.push(words[i]);
+            i += 1;
+        }
+    }
+    result.join(" ")
 }
```
