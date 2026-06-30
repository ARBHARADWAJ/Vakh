use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub language: String,
    pub threads: i32,
    pub silence_timeout: f32,
    pub typing_delay: u64,
    pub backspace_delay: u64,
    pub vad_sensitivity: i32,
    pub capsule_opacity: f32,
    pub dashboard_opacity: f32,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            threads: 2,
            silence_timeout: 15.0,
            typing_delay: 8,
            backspace_delay: 25,
            vad_sensitivity: 0, // Quality mode
            capsule_opacity: 0.85,
            dashboard_opacity: 0.75,
            theme: "default".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let mut path = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
        
        path.push("VAKH");
        let _ = fs::create_dir_all(&path);
        path.push("config.json");

        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    println!("[Config] Loaded: {:?}", config);
                    return config;
                }
            }
        }

        // If not exists or failed to parse, create default
        let default_config = Self::default();
        if let Ok(json) = serde_json::to_string_pretty(&default_config) {
            let _ = fs::write(path, json);
        }
        println!("[Config] Using default: {:?}", default_config);
        default_config
    }
}
