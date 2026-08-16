use serde::Serialize;
use cpal::Stream;

pub struct SendWrapper<T>(pub T);
unsafe impl<T> Send for SendWrapper<T> {}
unsafe impl<T> Sync for SendWrapper<T> {}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum VakhState {
    Idle,
    Listening,
    Processing,
    Flushing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum AudioStatus {
    Active,
    Listening,
    Finalizing,
    Idle,
    Level { level: f32 },
    Busy { message: String },
    #[allow(dead_code)]
    Warning { duration: f32 },
}

pub struct AppState {
    pub current_state: VakhState,
    pub target_hwnd: Option<isize>,
    pub audio_stream: Option<SendWrapper<Stream>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current_state: VakhState::Idle,
            target_hwnd: None,
            audio_stream: None,
        }
    }

    pub fn transition_to(&mut self, next: VakhState) {
        println!("State Transition: {:?} -> {:?}", self.current_state, next);
        self.current_state = next;
    }
}
