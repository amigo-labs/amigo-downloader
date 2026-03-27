//! Update event types for WebSocket broadcast.

use serde::Serialize;

/// Events broadcast during update operations.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum UpdateEvent {
    #[serde(rename = "update:check_started")]
    CheckStarted,

    #[serde(rename = "update:check_completed")]
    CheckCompleted {
        core_update: Option<CoreUpdateSummary>,
        plugin_updates: Vec<PluginUpdateSummary>,
    },

    #[serde(rename = "update:plugin_installing")]
    PluginInstalling { plugin_id: String },

    #[serde(rename = "update:plugin_installed")]
    PluginInstalled { plugin_id: String, version: String },

    #[serde(rename = "update:plugin_failed")]
    PluginUpdateFailed { plugin_id: String, error: String },

    #[serde(rename = "update:core_downloading")]
    CoreDownloading { progress: f64 },

    #[serde(rename = "update:core_ready")]
    CoreReady { new_version: String },

    #[serde(rename = "update:core_applied")]
    CoreApplied { new_version: String },

    #[serde(rename = "update:core_failed")]
    CoreUpdateFailed { error: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreUpdateSummary {
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: String,
    pub can_self_update: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginUpdateSummary {
    pub plugin_id: String,
    pub current_version: Option<String>,
    pub available_version: String,
    pub is_new: bool,
}
