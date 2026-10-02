pub mod bytecode_cache;
pub mod engine;
pub mod host_api;
pub mod loader;
pub mod permissions;
pub mod registry;
pub mod sandbox;
pub mod transpiler;
pub mod types;
pub mod updater;

/// Major version of the `amigo.*` host API this runtime implements.
///
/// Plugins declare the major they were written against as `apiVersion`.
/// Within a major the surface only grows (new functions, new optional
/// arguments, new fields on returned objects); renaming or removing a
/// function, changing a return shape or tightening an argument bumps it.
/// This constant is the single source of truth — the loader, the registry
/// filter and the docs all derive from it.
pub const HOST_API_VERSION: u32 = 1;

/// Oldest host-API major still accepted. The previous major stays supported
/// for one release after a bump (see `docs/plugin-api.md`).
pub const MIN_SUPPORTED_API_VERSION: u32 = 1;

/// Whether a plugin declaring host-API major `version` can be loaded.
pub fn is_supported_api_version(version: u32) -> bool {
    (MIN_SUPPORTED_API_VERSION..=HOST_API_VERSION).contains(&version)
}

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

    #[error("Incompatible version: plugin requires {required}, running {current}")]
    IncompatibleVersion { required: String, current: String },

    /// Installing or updating would grant the plugin network access the user
    /// has not approved yet. The message lists the domains.
    #[error("Permission approval required: {0}")]
    PermissionApprovalRequired(String),

    #[error("{0}")]
    Other(String),
}
