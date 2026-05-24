use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK,
    VK_LCONTROL, VK_RCONTROL, VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU
};
use std::mem::size_of;

pub struct TextInjector {
    current_draft: String,
    _target_hwnd: Option<isize>,
    typing_delay: u64,
    backspace_delay: u64,
}

impl TextInjector {
    pub fn new(target_hwnd: Option<isize>, typing_delay: u64, backspace_delay: u64) -> Self {
        Self {
            current_draft: String::new(),
            _target_hwnd: target_hwnd,
            typing_delay,
            backspace_delay,
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

        unsafe {
            // Send keystrokes directly to the currently active foreground window 
            // where the user's cursor is focused, without yanking focus back.

            // Programmatically release modifier keys (Ctrl, Shift, Alt)
            let modifiers = [
                VK_LCONTROL, VK_RCONTROL,
                VK_LSHIFT, VK_RSHIFT,
                VK_LMENU, VK_RMENU
            ];
            let mut modifier_inputs = Vec::new();
            for &vk in &modifiers {
                let mut input: INPUT = std::mem::zeroed();
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki = KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                };
                modifier_inputs.push(input);
            }
            SendInput(modifier_inputs.len() as u32, modifier_inputs.as_ptr(), size_of::<INPUT>() as i32);

            // 2. Send Backspaces for changed characters
            if common_prefix_len < old_text.len() {
                let old_suffix = &old_text[common_prefix_len..];
                let backspace_count = old_suffix.chars().count();
                println!("[Injection] Pacing {} backspaces", backspace_count);
                
                for _ in 0..backspace_count {
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
                    
                    SendInput(1, &input_down, size_of::<INPUT>() as i32);
                    std::thread::sleep(std::time::Duration::from_millis(5)); // let the down state register
                    SendInput(1, &input_up, size_of::<INPUT>() as i32);
                    std::thread::sleep(std::time::Duration::from_millis(self.typing_delay)); // interval between backspaces
                }

                // Crucial: sleep to let the OS and target app completely finish backspacing
                // before we type the new characters. This avoids scrambled/backward text.
                std::thread::sleep(std::time::Duration::from_millis(self.backspace_delay));
            }

            // 3. Paced typing of the new part
            let new_part = &new_text[common_prefix_len..];
            if !new_part.is_empty() {
                println!("[Injection] Pacing type for: '{}'", new_part);
                for c in new_part.encode_utf16() {
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
                    
                    SendInput(1, &input_down, size_of::<INPUT>() as i32);
                    std::thread::sleep(std::time::Duration::from_millis(5)); // let the down state register
                    SendInput(1, &input_up, size_of::<INPUT>() as i32);
                    std::thread::sleep(std::time::Duration::from_millis(self.typing_delay)); // interval between characters
                }
            }
        }

        self.current_draft = new_text;
    }

    pub fn commit(&mut self) {
        self.current_draft = String::new();
    }
}
