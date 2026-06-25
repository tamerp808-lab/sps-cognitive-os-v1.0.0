//! Settings persistence — save/load user preferences to disk.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/settings", get(get_settings).post(save_settings))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    /// Theme: "dark" or "light".
    #[serde(default = "default_theme")]
    theme: String,
    /// Default system prompt for chat.
    #[serde(default = "default_system_prompt")]
    system_prompt: String,
    /// Default provider id.
    #[serde(default)]
    default_provider: Option<String>,
    /// Whether streaming is enabled.
    #[serde(default = "default_true")]
    streaming: bool,
    /// Font size for code.
    #[serde(default = "default_font_size")]
    font_size: u32,
    /// Whether to show line numbers in code views.
    #[serde(default = "default_true")]
    show_line_numbers: bool,
    /// Auto-save interval (seconds, 0 = disabled).
    #[serde(default)]
    auto_save_interval: u32,
    /// Recent files.
    #[serde(default)]
    recent_files: Vec<String>,
    /// Recent commands (for command palette).
    #[serde(default)]
    recent_commands: Vec<String>,
}

fn default_theme() -> String { "dark".into() }
fn default_system_prompt() -> String { "You are a helpful AI assistant integrated into the SPS Cognitive Operating System. Be concise and accurate.".into() }
fn default_true() -> bool { true }
fn default_font_size() -> u32 { 14 }

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            system_prompt: default_system_prompt(),
            default_provider: None,
            streaming: true,
            font_size: default_font_size(),
            show_line_numbers: true,
            auto_save_interval: 0,
            recent_files: Vec::new(),
            recent_commands: Vec::new(),
        }
    }
}

fn settings_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".sps").join("settings.json")
}

async fn get_settings() -> Json<serde_json::Value> {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str::<Settings>(&content) {
                Ok(s) => Json(json!({
                    "theme": s.theme,
                    "system_prompt": s.system_prompt,
                    "default_provider": s.default_provider,
                    "streaming": s.streaming,
                    "font_size": s.font_size,
                    "show_line_numbers": s.show_line_numbers,
                    "auto_save_interval": s.auto_save_interval,
                    "recent_files": s.recent_files,
                    "recent_commands": s.recent_commands,
                })),
                Err(_) => Json(json!({})), // Return defaults on parse error
            }
        }
        Err(_) => {
            // No settings file — return defaults.
            let s = Settings::default();
            Json(json!({
                "theme": s.theme,
                "system_prompt": s.system_prompt,
                "default_provider": s.default_provider,
                "streaming": s.streaming,
                "font_size": s.font_size,
                "show_line_numbers": s.show_line_numbers,
                "auto_save_interval": s.auto_save_interval,
                "recent_files": s.recent_files,
                "recent_commands": s.recent_commands,
            }))
        }
    }
}

async fn save_settings(
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, serde_json::to_string_pretty(&req).unwrap_or_default()) {
        Ok(_) => Json(json!({ "saved": true, "path": path.to_string_lossy() })),
        Err(e) => Json(json!({ "saved": false, "error": e.to_string() })),
    }
}
