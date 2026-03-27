//! Host API functions exposed to Rune plugins.

/// The host API that plugins can call into.
/// All network/filesystem access is proxied through these functions.
pub struct HostApi;

impl HostApi {
    pub fn new() -> Self {
        Self
    }

    // Network functions — plugins have NO direct network access
    // HTTP requests, cookie management, etc. are all proxied here.

    // Parsing helpers: regex, HTML/CSS selectors, JSON, Base64

    // Crypto: AES decrypt, MD5, SHA256

    // Logging, storage, captcha, notifications
}
