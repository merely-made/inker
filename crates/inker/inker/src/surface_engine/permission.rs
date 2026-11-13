//! Permission and HTTP authentication requests.

use super::*;

/// A Permissions API descriptor in engine-neutral web-platform terms.
///
/// Chromium-only values are not allowed to become shared enum variants. A
/// producer which cannot project a request precisely uses `Other` and keeps
/// the stable standard or implementation name it received.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PermissionDescriptor {
    Camera,
    Microphone,
    Geolocation,
    Notifications,
    ClipboardRead,
    Midi { sysex: bool },
    PointerLock,
    KeyboardLock,
    IdleDetection,
    LocalFonts,
    StorageAccess,
    ProtectedMediaIdentifier,
    DisplayCapture { audio: bool, video: bool },
    Other(String),
}

/// The three states exposed by the Permissions API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PermissionState {
    Prompt,
    Granted,
    Denied,
}

/// One answer to a pending permission request.
///
/// `Dismiss` returns the request to `Prompt` and is deliberately distinct
/// from a retained denial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionAnswer {
    Grant,
    Deny,
    Dismiss,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: UserAgentRequestId,
    pub origin: String,
    pub descriptors: Vec<PermissionDescriptor>,
}

/// RFC 9110 protection-space data. It identifies credential lookup without
/// carrying a username or password.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HttpProtectionSpace {
    /// URL whose request provoked the challenge.
    pub origin_url: String,
    pub host: String,
    pub port: u16,
    pub realm: Option<String>,
    /// Authentication scheme token, normalized to ASCII lowercase.
    pub scheme: String,
    pub is_proxy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpAuthenticationChallenge {
    pub id: UserAgentRequestId,
    pub protection_space: HttpProtectionSpace,
}

/// A credential-provider result. Deliberately neither serializable nor
/// printable: secrets may cross the answer call but cannot enter events,
/// facets, traces, or persisted registries through a derive.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpCredentials {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for HttpCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpCredentials")
            .field("username", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum HttpAuthenticationAnswer {
    Credentials(HttpCredentials),
    Cancel,
}

impl fmt::Debug for HttpAuthenticationAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Credentials(_) => f.write_str("Credentials(<redacted>)"),
            Self::Cancel => f.write_str("Cancel"),
        }
    }
}
