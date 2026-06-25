//! Device Control Layer — Android Intents + File actions.
//!
//! Phase 3.1: Open apps, URLs, settings, camera, files via `am start`.
//! Works on Termux/Android. On desktop, falls back to `open`/`xdg-open`.
//!
//! Examples:
//!   "open youtube"       → am start -a android.intent.action.VIEW -d https://youtube.com
//!   "open whatsapp"      → am start -n com.whatsapp
//!   "open camera"        → am start -a android.media.action.IMAGE_CAPTURE
//!   "open settings"      → am start -a android.settings.SETTINGS
//!   "open downloads"     → am start -a android.intent.action.VIEW -d content://com.android.providers.downloads.documents/

use std::sync::Arc;
use std::process::Command;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::ServerState;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new()
        .route("/api/device/intent", post(open_intent))
        .route("/api/device/apps", get(list_apps))
        .route("/api/device/apps/installed", get(list_installed_apps))
        .route("/api/device/apps/search", get(search_installed_apps))
        .route("/api/device/open_app", post(open_app))
        .route("/api/device/open_url", post(open_url))
        .route("/api/device/camera", post(open_camera))
        .route("/api/device/settings", post(open_settings))
        .route("/api/device/files/open", post(open_file))
        .route("/api/device/files/recent", get(recent_files))
        .route("/api/device/confirm", post(confirm_action))
}

// ===== App name → package resolver =====
fn resolve_app(name: &str) -> Option<String> {
    let lower = name.to_lowercase().trim().to_string();
    let apps: &[(&str, &str)] = &[
        ("youtube", "com.google.android.youtube"),
        ("whatsapp", "com.whatsapp"),
        ("instagram", "com.instagram.android"),
        ("facebook", "com.facebook.katana"),
        ("twitter", "com.twitter.android"),
        ("x", "com.twitter.android"),
        ("telegram", "org.telegram.messenger"),
        ("tiktok", "com.zhiliaoapp.musically"),
        ("spotify", "com.spotify.music"),
        ("chrome", "com.android.chrome"),
        ("firefox", "org.mozilla.firefox"),
        ("gmail", "com.google.android.gm"),
        ("maps", "com.google.android.apps.maps"),
        ("google maps", "com.google.android.apps.maps"),
        ("camera", "android.media.action.IMAGE_CAPTURE"),
        ("calculator", "com.android.calculator2"),
        ("calendar", "com.google.android.calendar"),
        ("clock", "com.android.deskclock"),
        ("files", "com.android.documentsui"),
        ("file manager", "com.android.documentsui"),
        ("downloads", "com.android.providers.downloads.documents"),
        ("settings", "android.settings.SETTINGS"),
        ("bluetooth", "android.settings.BLUETOOTH_SETTINGS"),
        ("wifi", "android.settings.WIFI_SETTINGS"),
        ("play store", "com.android.vending"),
        ("playstore", "com.android.vending"),
        ("discord", "com.discord"),
        ("reddit", "com.reddit.frontpage"),
        ("linkedin", "com.linkedin.android"),
        ("snapchat", "com.snapchat.android"),
        ("netflix", "com.netflix.mediaclient"),
        ("zoom", "us.zoom.videomeetings"),
        ("teams", "com.microsoft.teams"),
        ("slack", "com.Slack"),
        ("notion", "notion.id"),
        ("vscode", "com.microsoft.vscode"),
        ("termux", "com.termux"),
        ("github", "com.github.android"),
        ("amazon", "com.amazon.mShop.android.shopping"),
        ("uber", "com.ubercab"),
        ("careem", "com.careem.acma"),
        ("spotify music", "com.spotify.music"),
        ("soundcloud", "com.soundcloud.android"),
        ("shazam", "com.shazam.android"),
        ("duolingo", "com.duolingo"),
        ("khan academy", "org.khanacademy.android"),
        ("coursera", "org.coursera.android"),
        ("udemy", "com.udemy.android"),
    ];

    // Try exact match first.
    for (alias, pkg) in apps {
        if lower == *alias {
            return Some(pkg.to_string());
        }
    }
    // Try fuzzy: if the name contains any alias.
    for (alias, pkg) in apps {
        if lower.contains(alias) {
            return Some(pkg.to_string());
        }
    }
    // Try installed apps on Android.
    if is_android() {
        if let Some(pkg) = search_installed_packages(&lower) {
            return Some(pkg);
        }
    }
    None
}

/// Search installed packages via `pm list packages` on Android.
fn search_installed_packages(query: &str) -> Option<String> {
    let output = Command::new("pm")
        .args(["list", "packages"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse: "package:com.example.app"
    let mut matches: Vec<String> = Vec::new();
    for line in stdout.lines() {
        if let Some(pkg) = line.strip_prefix("package:") {
            let pkg_lower = pkg.to_lowercase();
            // Check if query matches the package name.
            if pkg_lower.contains(query) {
                matches.push(pkg.to_string());
            }
        }
    }
    // Return best match (shortest = most likely the app itself).
    matches.sort_by_key(|s| s.len());
    matches.into_iter().next()
}

/// List all installed packages.
fn list_all_packages() -> Vec<String> {
    let output = Command::new("pm")
        .args(["list", "packages"])
        .output();
    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout
                .lines()
                .filter_map(|l| l.strip_prefix("package:").map(String::from))
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

fn is_android() -> bool {
    std::path::Path::new("/system/bin/am").exists() || std::env::var("TERMUX_VERSION").is_ok()
}

fn run_am(args: &[&str]) -> Result<String, String> {
    let cmd = if is_android() { "am" } else { "xdg-open" };
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.is_empty() {
            Ok(String::new()) // am sometimes returns non-zero but still works
        } else {
            Err(stderr)
        }
    }
}

// ===== Handlers =====

#[derive(Debug, Deserialize)]
struct IntentRequest {
    action: String,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    package: Option<String>,
}

#[derive(Debug, Serialize)]
struct IntentResult {
    success: bool,
    message: String,
    platform: String,
}

async fn open_intent(
    Json(req): Json<IntentRequest>,
) -> Json<IntentResult> {
    let mut args = vec!["start", "-a", &req.action];
    if let Some(ref data) = req.data {
        args.push("-d");
        args.push(data);
    }
    if let Some(ref pkg) = req.package {
        args.push("-n");
        args.push(pkg);
    }

    match run_am(&args) {
        Ok(_) => Json(IntentResult {
            success: true,
            message: format!("Intent {} executed", req.action),
            platform: if is_android() { "android".into() } else { "desktop".into() },
        }),
        Err(e) => Json(IntentResult {
            success: false,
            message: e,
            platform: if is_android() { "android".into() } else { "desktop".into() },
        }),
    }
}

#[derive(Debug, Deserialize)]
struct OpenAppRequest {
    name: String,
}

async fn open_app(
    Json(req): Json<OpenAppRequest>,
) -> Json<serde_json::Value> {
    let name = req.name.trim();

    // Check if it's a URL.
    if name.starts_with("http://") || name.starts_with("https://") {
        return open_url_internal(name);
    }

    // Resolve app name to package.
    let pkg = match resolve_app(name) {
        Some(p) => p,
        None => {
            // Try as direct package name.
            if name.contains('.') {
                name.to_string()
            } else {
                return Json(json!({
                    "success": false,
                    "error": format!("App '{}' not recognized. Try: youtube, whatsapp, telegram, chrome, camera, settings, etc.", name),
                    "suggestions": ["youtube", "whatsapp", "telegram", "chrome", "camera", "settings", "maps", "spotify", "gmail", "files"],
                }));
            }
        }
    };

    // Special: camera uses action, not package.
    if pkg.starts_with("android.media.action") || pkg.starts_with("android.settings") {
        let args = vec!["start", "-a", &pkg];
        match run_am(&args) {
            Ok(_) => return Json(json!({"success": true, "action": pkg, "platform": if is_android() {"android"} else {"desktop"}})),
            Err(e) => return Json(json!({"success": false, "error": e})),
        }
    }

    // Open the app.
    let args = vec!["start", "-n", &pkg];
    match run_am(&args) {
        Ok(_) => Json(json!({
            "success": true,
            "app": name,
            "package": pkg,
            "platform": if is_android() { "android" } else { "desktop" },
        })),
        Err(e) => {
            // Fallback: try as URL (maybe it's a domain).
            if name.contains('.') {
                open_url_internal(&format!("https://{}", name))
            } else {
                Json(json!({"success": false, "error": e, "app": name, "package": pkg}))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenUrlRequest {
    url: String,
}

async fn open_url(
    Json(req): Json<OpenUrlRequest>,
) -> Json<serde_json::Value> {
    open_url_internal(&req.url)
}

fn open_url_internal(url: &str) -> Json<serde_json::Value> {
    let url = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    if is_android() {
        let args = vec!["start", "-a", "android.intent.action.VIEW", "-d", &url];
        match run_am(&args) {
            Ok(_) => Json(json!({"success": true, "url": url, "platform": "android"})),
            Err(e) => Json(json!({"success": false, "error": e, "url": url})),
        }
    } else {
        // Desktop: use open (macOS) or xdg-open (Linux).
        let cmd = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        match Command::new(cmd).arg(&url).output() {
            Ok(_) => Json(json!({"success": true, "url": url, "platform": "desktop"})),
            Err(e) => Json(json!({"success": false, "error": e.to_string(), "url": url})),
        }
    }
}

async fn open_camera() -> Json<serde_json::Value> {
    if is_android() {
        let args = vec!["start", "-a", "android.media.action.IMAGE_CAPTURE"];
        match run_am(&args) {
            Ok(_) => Json(json!({"success": true, "action": "camera", "platform": "android"})),
            Err(e) => Json(json!({"success": false, "error": e})),
        }
    } else {
        Json(json!({"success": false, "error": "Camera only available on Android", "platform": "desktop"}))
    }
}

#[derive(Debug, Deserialize)]
struct SettingsRequest {
    #[serde(default)]
    setting: Option<String>,
}

async fn open_settings(
    Json(req): Json<SettingsRequest>,
) -> Json<serde_json::Value> {
    let setting = req.setting.unwrap_or_default();
    let action = match setting.to_lowercase().as_str() {
        "wifi" => "android.settings.WIFI_SETTINGS",
        "bluetooth" => "android.settings.BLUETOOTH_SETTINGS",
        "location" => "android.settings.LOCATION_SOURCE_SETTINGS",
        "display" => "android.settings.DISPLAY_SETTINGS",
        "sound" => "android.settings.SOUND_SETTINGS",
        "battery" => "android.settings.BATTERY_SAVER_SETTINGS",
        "apps" => "android.settings.APPLICATION_SETTINGS",
        "storage" => "android.settings.INTERNAL_STORAGE_SETTINGS",
        "security" => "android.settings.SECURITY_SETTINGS",
        "about" => "android.settings.DEVICE_INFO_SETTINGS",
        "developer" => "android.settings.APPLICATION_DEVELOPMENT_SETTINGS",
        "airplane" => "android.settings.AIRPLANE_MODE_SETTINGS",
        "data" => "android.settings.DATA_USAGE_SETTINGS",
        "nfc" => "android.settings.NFC_SETTINGS",
        "hotspot" => "android.settings.TETHER_SETTINGS",
        _ => "android.settings.SETTINGS",
    };

    if is_android() {
        let args = vec!["start", "-a", action];
        match run_am(&args) {
            Ok(_) => Json(json!({"success": true, "action": action, "platform": "android"})),
            Err(e) => Json(json!({"success": false, "error": e})),
        }
    } else {
        Json(json!({"success": false, "error": "Settings only available on Android", "platform": "desktop"}))
    }
}

async fn list_apps() -> Json<serde_json::Value> {
    let known_apps = json!([
        {"name": "YouTube", "alias": "youtube", "icon": "📺"},
        {"name": "WhatsApp", "alias": "whatsapp", "icon": "💬"},
        {"name": "Telegram", "alias": "telegram", "icon": "✈️"},
        {"name": "Instagram", "alias": "instagram", "icon": "📷"},
        {"name": "Chrome", "alias": "chrome", "icon": "🌐"},
        {"name": "Gmail", "alias": "gmail", "icon": "📧"},
        {"name": "Google Maps", "alias": "maps", "icon": "🗺️"},
        {"name": "Spotify", "alias": "spotify", "icon": "🎵"},
        {"name": "Camera", "alias": "camera", "icon": "📸"},
        {"name": "Settings", "alias": "settings", "icon": "⚙️"},
        {"name": "Files", "alias": "files", "icon": "📁"},
        {"name": "Calculator", "alias": "calculator", "icon": "🔢"},
        {"name": "Calendar", "alias": "calendar", "icon": "📅"},
        {"name": "Clock", "alias": "clock", "icon": "⏰"},
        {"name": "Play Store", "alias": "play store", "icon": "🛒"},
        {"name": "Discord", "alias": "discord", "icon": "🎮"},
        {"name": "Reddit", "alias": "reddit", "icon": "🤖"},
        {"name": "Netflix", "alias": "netflix", "icon": "🎬"},
        {"name": "Zoom", "alias": "zoom", "icon": "🎥"},
        {"name": "Notion", "alias": "notion", "icon": "📝"},
        {"name": "Duolingo", "alias": "duolingo", "icon": "🦉"},
        {"name": "Termux", "alias": "termux", "icon": "💻"},
    ]);
    Json(json!({"apps": known_apps, "platform": if is_android() {"android"} else {"desktop"}}))
}

#[derive(Debug, Deserialize)]
struct OpenFileRequest {
    path: String,
}

async fn open_file(
    Json(req): Json<OpenFileRequest>,
) -> Json<serde_json::Value> {
    let path = req.path.trim();

    if is_android() {
        // Try to open with am.
        let file_url = format!("file://{}", path);
        let args = vec!["start", "-a", "android.intent.action.VIEW", "-d", &file_url, "-t", "*/*"];
        match run_am(&args) {
            Ok(_) => Json(json!({"success": true, "path": path, "platform": "android"})),
            Err(e) => {
                // Fallback: try termux-open.
                match Command::new("termux-open").arg(path).output() {
                    Ok(_) => Json(json!({"success": true, "path": path, "platform": "android", "method": "termux-open"})),
                    Err(_) => Json(json!({"success": false, "error": e, "path": path})),
                }
            }
        }
    } else {
        let cmd = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        match Command::new(cmd).arg(path).output() {
            Ok(_) => Json(json!({"success": true, "path": path, "platform": "desktop"})),
            Err(e) => Json(json!({"success": false, "error": e.to_string(), "path": path})),
        }
    }
}

// ===== Installed Apps Discovery =====

async fn list_installed_apps() -> Json<serde_json::Value> {
    if !is_android() {
        return Json(json!({"apps": [], "platform": "desktop", "note": "Only available on Android"}));
    }
    let packages = list_all_packages();
    let apps: Vec<serde_json::Value> = packages.iter().map(|p| {
        // Extract a readable name from the package.
        let parts: Vec<&str> = p.split('.').collect();
        let name = parts.last().unwrap_or(&p.as_str()).to_string();
        json!({"package": p, "name": name})
    }).collect();
    Json(json!({"apps": apps, "count": apps.len(), "platform": "android"}))
}

#[derive(Debug, Deserialize)]
struct SearchAppQuery {
    q: String,
}

async fn search_installed_apps(
    axum::extract::Query(q): axum::extract::Query<SearchAppQuery>,
) -> Json<serde_json::Value> {
    if !is_android() {
        return Json(json!({"results": [], "platform": "desktop"}));
    }
    let query = q.q.to_lowercase();
    let packages = list_all_packages();
    let results: Vec<serde_json::Value> = packages.iter()
        .filter(|p| p.to_lowercase().contains(&query))
        .map(|p| {
            let parts: Vec<&str> = p.split('.').collect();
            let name = parts.last().unwrap_or(&p.as_str()).to_string();
            json!({"package": p, "name": name})
        })
        .take(20)
        .collect();
    Json(json!({"results": results, "count": results.len(), "query": q.q, "platform": "android"}))
}

// ===== Recent Files =====

#[derive(Debug, Deserialize)]
struct RecentFilesQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    ext: Option<String>,
}

fn default_limit() -> usize { 20 }

async fn recent_files(
    axum::extract::Query(q): axum::extract::Query<RecentFilesQuery>,
) -> Json<serde_json::Value> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let home_path = std::path::PathBuf::from(&home);

    // Common download/media directories.
    let dirs = [
        home_path.join("Downloads"),
        home_path.join("Download"),
        home_path.join("DCIM"),
        home_path.join("Pictures"),
        home_path.join("Documents"),
        home_path.join("storage/downloads"),
        home_path.join("storage/shared/Download"),
    ];

    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime, u64)> = Vec::new();

    for dir in &dirs {
        if !dir.exists() { continue; }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        let size = metadata.len();
                        // Filter by extension if specified.
                        if let Some(ref ext) = q.ext {
                            if let Some(file_ext) = entry.path().extension() {
                                if file_ext.to_string_lossy().to_lowercase() != ext.to_lowercase() {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        }
                        files.push((entry.path(), modified, size));
                    }
                }
            }
        }
    }

    // Sort by modified time (newest first).
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.truncate(q.limit);

    let results: Vec<serde_json::Value> = files.iter().map(|(path, modified, size)| {
        let filename = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let path_str = path.to_string_lossy().to_string();
        let ext = path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or("FILE".into());
        let timestamp = modified.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        json!({
            "name": filename,
            "path": path_str,
            "extension": ext,
            "size": size,
            "modified": timestamp,
        })
    }).collect();

    Json(json!({"files": results, "count": results.len()}))
}

// ===== Dangerous Command Confirmation =====

const DANGEROUS_KEYWORDS: &[&str] = &[
    "delete", "remove", "rm", "del", "erase", "wipe",  // File deletion
    "send", "message", "sms", "whatsapp", "email", "mail",  // Communication
    "buy", "purchase", "order", "pay", "checkout",  // Financial
    "transfer", "wire", "send money",  // Financial
    "publish", "post", "share", "upload",  // Social
    "uninstall", "disable", "reset",  // System
    "format", "factory reset",  // Destructive
    "call", "dial",  // Phone
    "install",  // Software installation
];

#[derive(Debug, Deserialize)]
struct ConfirmRequest {
    command: String,
    #[serde(default)]
    confirmed: bool,
}

#[derive(Debug, Serialize)]
struct ConfirmResult {
    /// Whether the command is dangerous.
    is_dangerous: bool,
    /// Whether it can proceed.
    can_proceed: bool,
    /// The matched dangerous keyword.
    matched_keyword: Option<String>,
    /// Confirmation message.
    message: String,
}

async fn confirm_action(
    Json(req): Json<ConfirmRequest>,
) -> Json<ConfirmResult> {
    let lower = req.command.to_lowercase();
    let matched = DANGEROUS_KEYWORDS.iter().find(|kw| lower.contains(**kw));

    match matched {
        Some(kw) => {
            if req.confirmed {
                Json(ConfirmResult {
                    is_dangerous: true,
                    can_proceed: true,
                    matched_keyword: Some(kw.to_string()),
                    message: format!("Confirmed: proceeding with '{}'", req.command),
                })
            } else {
                Json(ConfirmResult {
                    is_dangerous: true,
                    can_proceed: false,
                    matched_keyword: Some(kw.to_string()),
                    message: format!("⚠️ This command contains '{}'. Say 'yes' or tap confirm to proceed.", kw),
                })
            }
        }
        None => Json(ConfirmResult {
            is_dangerous: false,
            can_proceed: true,
            matched_keyword: None,
            message: "Safe to proceed.".to_string(),
        })
    }
}
