pub mod host_api;
pub mod loader;
pub mod registry;
pub mod sandbox;
pub mod types;
pub mod updater;

/// Plugin runtime error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Plugin execution error: {0}")]
    Execution(String),

    #[error("Plugin timeout after {0}s")]
    Timeout(u64),

    #[error("Plugin sandbox violation: {0}")]
    SandboxViolation(String),

    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Registry unavailable: {0}")]
    RegistryUnavailable(String),

    #[error("Checksum mismatch for plugin {0}")]
    ChecksumMismatch(String),

    #[error("Incompatible app version: plugin requires {required}, running {current}")]
    IncompatibleVersion { required: String, current: String },

    #[error("{0}")]
    Other(String),
}
