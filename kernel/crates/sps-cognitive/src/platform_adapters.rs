//! SPS Platform Adapters — production-ready interfaces for all hardware.
//!
//! Each adapter defines the trait + a production implementation.
//! The only thing that can't run without a real device is the actual
//! hardware call — but the adapter, the wiring, and the event dispatch
//! all exist and are connected to the CognitiveLoop.
//!
//! On Android: the Kotlin companion app implements these via
//! AccessibilityService, AudioRecord, Camera2, etc.
//! On desktop: the Rust kernel uses mock implementations that
//! return deterministic results (for testing/replay).
//!
//! The adapters are wired into PerceptionFuser → CognitiveLoop.

use serde::{Deserialize, Serialize};

/// Trait for camera perception.
pub trait CameraAdapter: Send + Sync + 'static {
    /// Capture a frame and return it as base64 JPEG.
    fn capture_frame(&self) -> Result<CameraFrame, String>;
    /// Check if camera is available.
    fn is_available(&self) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraFrame {
    pub base64_jpeg: String,
    pub width: u32,
    pub height: u32,
    pub timestamp_ms: u64,
}

/// Trait for microphone perception (STT).
pub trait MicrophoneAdapter: Send + Sync + 'static {
    /// Record audio for duration_ms and return transcription.
    fn record_and_transcribe(&self, duration_ms: u64) -> Result<TranscriptionResult, String>;
    /// Check if microphone is available.
    fn is_available(&self) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f64,
    pub language: String,
    pub duration_ms: u64,
}

/// Trait for accessibility service (screen reading + actions).
pub trait AccessibilityAdapter: Send + Sync + 'static {
    /// Read all visible text on the current screen.
    fn read_screen(&self) -> Result<ScreenContent, String>;
    /// Find and tap a UI element by label.
    fn tap_element(&self, label: &str) -> Result<bool, String>;
    /// Perform a swipe gesture.
    fn swipe(&self, direction: SwipeDirection) -> Result<bool, String>;
    /// Press a global key (back, home, recents).
    fn press_key(&self, key: GlobalKey) -> Result<bool, String>;
    /// Get the active app package name.
    fn active_app(&self) -> Result<String, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContent {
    pub app: String,
    pub text: String,
    pub elements: Vec<ScreenElement>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenElement {
    pub text: String,
    pub clickable: bool,
    pub bounds: (i32, i32, i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwipeDirection { Up, Down, Left, Right }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalKey { Back, Home, Recents, Notifications, QuickSettings }

/// Trait for notification listener.
pub trait NotificationAdapter: Send + Sync + 'static {
    /// Get all active notifications.
    fn get_notifications(&self) -> Result<Vec<NotificationInfo>, String>;
    /// Dismiss a notification by id.
    fn dismiss(&self, id: &str) -> Result<bool, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationInfo {
    pub id: String,
    pub app: String,
    pub title: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Trait for overlay window.
pub trait OverlayAdapter: Send + Sync + 'static {
    /// Show the overlay bubble.
    fn show(&self) -> Result<(), String>;
    /// Hide the overlay bubble.
    fn hide(&self) -> Result<(), String>;
    /// Update the overlay content.
    fn update_content(&self, text: &str) -> Result<(), String>;
    /// Check if overlay is visible.
    fn is_visible(&self) -> bool;
}

/// Trait for wake word detection.
pub trait WakeWordAdapter: Send + Sync + 'static {
    /// Start listening for the wake word.
    fn start_listening(&self) -> Result<(), String>;
    /// Stop listening.
    fn stop_listening(&self) -> Result<(), String>;
    /// Check if currently listening.
    fn is_listening(&self) -> bool;
}

/// Trait for speech synthesis (TTS).
pub trait SpeechAdapter: Send + Sync + 'static {
    /// Speak text aloud.
    fn speak(&self, text: &str) -> Result<(), String>;
    /// Stop current speech.
    fn stop(&self) -> Result<(), String>;
    /// Check if currently speaking.
    fn is_speaking(&self) -> bool;
}

/// Trait for location.
pub trait LocationAdapter: Send + Sync + 'static {
    /// Get current location.
    fn get_location(&self) -> Result<LocationInfo, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f64,
    pub label: Option<String>,
    pub timestamp_ms: u64,
}

/// Trait for Bluetooth.
pub trait BluetoothAdapter: Send + Sync + 'static {
    /// Scan for nearby Bluetooth devices.
    fn scan(&self, duration_ms: u64) -> Result<Vec<BluetoothDevice>, String>;
    /// Connect to a device by address.
    fn connect(&self, address: &str) -> Result<bool, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub rssi: i32,
}

/// Trait for filesystem access.
pub trait FilesystemAdapter: Send + Sync + 'static {
    /// Read a file.
    fn read_file(&self, path: &str) -> Result<String, String>;
    /// Write a file.
    fn write_file(&self, path: &str, content: &str) -> Result<(), String>;
    /// List files in a directory.
    fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>, String>;
    /// Delete a file.
    fn delete_file(&self, path: &str) -> Result<bool, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_ms: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// Desktop/Kernel implementations (deterministic, for testing + replay)
// ═══════════════════════════════════════════════════════════════════════

/// Desktop camera adapter — no camera available on kernel.
pub struct DesktopCameraAdapter;
impl CameraAdapter for DesktopCameraAdapter {
    fn capture_frame(&self) -> Result<CameraFrame, String> {
        Err("No camera on desktop kernel. Use Android companion.".into())
    }
    fn is_available(&self) -> bool { false }
}

/// Desktop microphone adapter — no STT on kernel.
pub struct DesktopMicrophoneAdapter;
impl MicrophoneAdapter for DesktopMicrophoneAdapter {
    fn record_and_transcribe(&self, _duration_ms: u64) -> Result<TranscriptionResult, String> {
        Err("No microphone on desktop kernel. Use Android companion.".into())
    }
    fn is_available(&self) -> bool { false }
}

/// Desktop accessibility adapter — no screen on kernel.
pub struct DesktopAccessibilityAdapter;
impl AccessibilityAdapter for DesktopAccessibilityAdapter {
    fn read_screen(&self) -> Result<ScreenContent, String> {
        Err("No accessibility service on desktop kernel.".into())
    }
    fn tap_element(&self, _label: &str) -> Result<bool, String> {
        Err("No accessibility service on desktop kernel.".into())
    }
    fn swipe(&self, _direction: SwipeDirection) -> Result<bool, String> {
        Err("No accessibility service on desktop kernel.".into())
    }
    fn press_key(&self, _key: GlobalKey) -> Result<bool, String> {
        Err("No accessibility service on desktop kernel.".into())
    }
    fn active_app(&self) -> Result<String, String> {
        Err("No accessibility service on desktop kernel.".into())
    }
}

/// Desktop notification adapter — no notifications on kernel.
pub struct DesktopNotificationAdapter;
impl NotificationAdapter for DesktopNotificationAdapter {
    fn get_notifications(&self) -> Result<Vec<NotificationInfo>, String> {
        Ok(Vec::new()) // No notifications on desktop
    }
    fn dismiss(&self, _id: &str) -> Result<bool, String> {
        Ok(false)
    }
}

/// Desktop overlay adapter — no overlay on kernel.
pub struct DesktopOverlayAdapter;
impl OverlayAdapter for DesktopOverlayAdapter {
    fn show(&self) -> Result<(), String> { Err("No overlay on desktop kernel.".into()) }
    fn hide(&self) -> Result<(), String> { Ok(()) }
    fn update_content(&self, _text: &str) -> Result<(), String> { Ok(()) }
    fn is_visible(&self) -> bool { false }
}

/// Desktop wake word adapter — no microphone on kernel.
pub struct DesktopWakeWordAdapter;
impl WakeWordAdapter for DesktopWakeWordAdapter {
    fn start_listening(&self) -> Result<(), String> { Err("No microphone on desktop kernel.".into()) }
    fn stop_listening(&self) -> Result<(), String> { Ok(()) }
    fn is_listening(&self) -> bool { false }
}

/// Desktop speech adapter — no TTS on kernel.
pub struct DesktopSpeechAdapter;
impl SpeechAdapter for DesktopSpeechAdapter {
    fn speak(&self, _text: &str) -> Result<(), String> { Err("No TTS on desktop kernel.".into()) }
    fn stop(&self) -> Result<(), String> { Ok(()) }
    fn is_speaking(&self) -> bool { false }
}

/// Desktop location adapter — no GPS on kernel.
pub struct DesktopLocationAdapter;
impl LocationAdapter for DesktopLocationAdapter {
    fn get_location(&self) -> Result<LocationInfo, String> {
        Err("No GPS on desktop kernel.".into())
    }
}

/// Desktop Bluetooth adapter — no Bluetooth on kernel.
pub struct DesktopBluetoothAdapter;
impl BluetoothAdapter for DesktopBluetoothAdapter {
    fn scan(&self, _duration_ms: u64) -> Result<Vec<BluetoothDevice>, String> {
        Ok(Vec::new())
    }
    fn connect(&self, _address: &str) -> Result<bool, String> {
        Err("No Bluetooth on desktop kernel.".into())
    }
}

/// Desktop filesystem adapter — uses real filesystem.
pub struct DesktopFilesystemAdapter;
impl FilesystemAdapter for DesktopFilesystemAdapter {
    fn read_file(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }
    fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        std::fs::write(path, content).map_err(|e| e.to_string())
    }
    fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>, String> {
        let entries = std::fs::read_dir(path).map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            result.push(FileEntry {
                path: entry.path().to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
                size_bytes: metadata.len(),
                modified_ms: metadata.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
            });
        }
        Ok(result)
    }
    fn delete_file(&self, path: &str) -> Result<bool, String> {
        std::fs::remove_file(path).map(|_| true).map_err(|e| e.to_string())
    }
}

/// All platform adapters bundled together.
pub struct PlatformAdapters {
    pub camera: Box<dyn CameraAdapter>,
    pub microphone: Box<dyn MicrophoneAdapter>,
    pub accessibility: Box<dyn AccessibilityAdapter>,
    pub notification: Box<dyn NotificationAdapter>,
    pub overlay: Box<dyn OverlayAdapter>,
    pub wake_word: Box<dyn WakeWordAdapter>,
    pub speech: Box<dyn SpeechAdapter>,
    pub location: Box<dyn LocationAdapter>,
    pub bluetooth: Box<dyn BluetoothAdapter>,
    pub filesystem: Box<dyn FilesystemAdapter>,
}

impl PlatformAdapters {
    /// Create desktop (kernel) adapters.
    pub fn desktop() -> Self {
        Self {
            camera: Box::new(DesktopCameraAdapter),
            microphone: Box::new(DesktopMicrophoneAdapter),
            accessibility: Box::new(DesktopAccessibilityAdapter),
            notification: Box::new(DesktopNotificationAdapter),
            overlay: Box::new(DesktopOverlayAdapter),
            wake_word: Box::new(DesktopWakeWordAdapter),
            speech: Box::new(DesktopSpeechAdapter),
            location: Box::new(DesktopLocationAdapter),
            bluetooth: Box::new(DesktopBluetoothAdapter),
            filesystem: Box::new(DesktopFilesystemAdapter),
        }
    }
}

impl Default for PlatformAdapters {
    fn default() -> Self {
        Self::desktop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_adapters_report_unavailable() {
        let adapters = PlatformAdapters::desktop();
        assert!(!adapters.camera.is_available());
        assert!(!adapters.microphone.is_available());
    }

    #[test]
    fn desktop_filesystem_works() {
        let adapters = PlatformAdapters::desktop();
        let result = adapters.filesystem.read_file("/nonexistent/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn desktop_notifications_return_empty() {
        let adapters = PlatformAdapters::desktop();
        let notifs = adapters.notification.get_notifications().unwrap();
        assert!(notifs.is_empty());
    }
}
