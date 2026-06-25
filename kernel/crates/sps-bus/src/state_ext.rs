//! Owner profile extension to Canonical State.
//!
//! This is the first "real" extension slice beyond the kernel meta slice.
//! It tracks the singleton owner's display name, preferences, and whether
//! a local password lock is set.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;

/// Extension key under which the owner state is stored.
pub const EXTENSION_KEY: &str = "owner";

/// Owner preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerPreferences {
    /// Default LLM provider id, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<SmolStr>,
    /// Preferred language for UI/output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<SmolStr>,
    /// Whether autonomy is enabled.
    #[serde(default)]
    pub autonomy_enabled: bool,
}

/// The owner profile (singleton).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerProfile {
    /// Display name (auto-set to "Owner" on first boot).
    pub display_name: SmolStr,
    /// Whether a local password lock is set (just the flag, not the hash).
    pub has_password: bool,
    /// Owner preferences.
    #[serde(default)]
    pub preferences: OwnerPreferences,
    /// Wall time the owner profile was created (display only).
    pub created_at: u64,
}

/// The owner state slice stored under `state.extensions["owner"]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerState {
    /// The owner profile.
    pub profile: OwnerProfile,
}

impl OwnerState {
    /// Read from canonical state.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        state.get_extension(EXTENSION_KEY)
    }

    /// Write to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }
}

/// Reducer for owner events.
#[derive(Debug, Default)]
pub struct OwnerReducer;

impl OwnerReducer {
    /// Register this reducer for owner event types.
    pub fn register(registry: &mut ReducerRegistry) {
        let r = std::sync::Arc::new(Self);
        for et in &[
            "owner.profile_created",
            "owner.name_changed",
            "owner.password_set",
            "owner.password_cleared",
            "owner.preferences_updated",
            "owner.autonomy_toggled",
        ] {
            registry.register(*et, r.clone());
        }
    }
}

impl Reducer for OwnerReducer {
    fn name(&self) -> &'static str {
        "owner"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        let mut owner = OwnerState::from_state(state).unwrap_or_default();
        match event.event_type.as_str() {
            "owner.profile_created" => {
                let display_name = event.payload["display_name"]
                    .as_str()
                    .unwrap_or("Owner")
                    .to_string();
                let created_at = event.payload["created_at"].as_u64().unwrap_or(0);
                owner.profile = OwnerProfile {
                    display_name: display_name.into(),
                    has_password: false,
                    preferences: OwnerPreferences::default(),
                    created_at,
                };
            }
            "owner.name_changed" => {
                if let Some(name) = event.payload["display_name"].as_str() {
                    owner.profile.display_name = name.into();
                }
            }
            "owner.password_set" => {
                owner.profile.has_password = true;
            }
            "owner.password_cleared" => {
                owner.profile.has_password = false;
            }
            "owner.preferences_updated" => {
                if let Some(prefs_val) = event.payload.get("preferences") {
                    if let Ok(prefs) = serde_json::from_value::<OwnerPreferences>(prefs_val.clone())
                    {
                        owner.profile.preferences = prefs;
                    }
                }
            }
            "owner.autonomy_toggled" => {
                if let Some(enabled) = event.payload["enabled"].as_bool() {
                    owner.profile.preferences.autonomy_enabled = enabled;
                }
            }
            _ => {}
        }
        owner.save_to(state)?;
        Ok(())
    }
}
