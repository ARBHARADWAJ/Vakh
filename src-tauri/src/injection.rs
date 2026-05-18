use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK
};
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
use std::mem::size_of;

pub struct TextInjector {
    current_draft: String,
    target_hwnd: Option<isize>,
}

impl TextInjector {
    pub fn new(target_hwnd: Option<isize>) -> Self {
        Self {
            current_draft: String::new(),
            target_hwnd,
        }
    }

    pub fn inject_draft(&mut self, text: &str) {
        let new_text = text.to_string();
        let old_text = self.current_draft.clone();

        if new_text == old_text { return; }

        // 1. Find common prefix length
        let mut common_prefix_len = 0;
        for (c1, c2) in old_text.chars().zip(new_text.chars()) {
            if c1 == c2 {
                common_prefix_len += c1.len_utf8();
            } else {
                break;
            }
        }

        // OPTIMIZATION: If the change is at the very end and only adds text, 
        // don't backspace anything.
        let mut inputs = Vec::new();

        if common_prefix_len < old_text.len() {
            // Backspace the characters that changed
            let old_suffix = &old_text[common_prefix_len..];
            let backspace_count = old_suffix.chars().count();
            
            for _ in 0..backspace_count {
                unsafe {
                    let mut input_down: INPUT = std::mem::zeroed();
                    input_down.r#type = INPUT_KEYBOARD;
                    input_down.Anonymous.ki = KEYBDINPUT {
                        wVk: VK_BACK,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                    let mut input_up: INPUT = std::mem::zeroed();
                    input_up.r#type = INPUT_KEYBOARD;
                    input_up.Anonymous.ki = KEYBDINPUT {
                        wVk: VK_BACK,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                    inputs.push(input_down);
                    inputs.push(input_up);
                }
            }
        }

        // 3. Type the new part
        let new_part = &new_text[common_prefix_len..];
        for c in new_part.encode_utf16() {
            unsafe {
                let mut input_down: INPUT = std::mem::zeroed();
                input_down.r#type = INPUT_KEYBOARD;
                input_down.Anonymous.ki = KEYBDINPUT {
                    wVk: 0,
                    wScan: c,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                };
                let mut input_up: INPUT = std::mem::zeroed();
                input_up.r#type = INPUT_KEYBOARD;
                input_up.Anonymous.ki = KEYBDINPUT {
                    wVk: 0,
                    wScan: c,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                };
                inputs.push(input_down);
                inputs.push(input_up);
            }
        }

        // 4. Send the batch
        if !inputs.is_empty() {
            unsafe {
                if let Some(hwnd) = self.target_hwnd {
                    SetForegroundWindow(hwnd as _);
                }
                SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
            }
        }

        self.current_draft = new_text;
    }

    pub fn commit(&mut self) {
        self.current_draft = String::new();
    }
}
