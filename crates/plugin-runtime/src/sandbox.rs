//! Plugin sandboxing and resource limits.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxLimits {
    /// Max execution time per resolve() call in seconds.
    pub max_execution_secs: u64,
    /// Max memory per plugin instance in bytes.
    pub max_memory_bytes: u64,
    /// Max HTTP requests per call.
    pub max_http_requests: u32,
    /// Max storage per plugin in bytes.
    pub max_storage_bytes: u64,
    /// Allow plugin HTTP requests to reach private / loopback / link-local IPs.
    /// Defaults to `false` — a blocked request returns an error from
    /// `amigo.httpGet`/`httpPost`/`httpHead`/etc. Set to `true` only if you
    /// trust every installed plugin and need to talk to LAN hosts.
    #[serde(default)]
    pub allow_private_network: bool,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_execution_secs: 30,
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_http_requests: 20,
            max_storage_bytes: 1024 * 1024, // 1MB
            allow_private_network: false,
        }
    }
}
