//! Producer event vocabulary: navigation, cookies, capabilities and settings.

use super::*;

// ── Producer event vocabulary ──────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationEvent {
    Started { url: String },
    Committed { url: String },
    Finished { url: String, title: Option<String> },
    Failed { url: String, reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShape {
    Default,
    Text,
    Pointer,
    Grab,
    Grabbing,
    Crosshair,
    Move,
    ResizeNs,
    ResizeEw,
    ResizeNesw,
    ResizeNwse,
    NotAllowed,
    Hidden,
}

/// A message posted from the page via the JS bridge (postMessage-style).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebMessage {
    pub tag: String,
    pub payload: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

/// HTTP cookie payload used at the generic web-surface boundary.
///
/// This mirrors the engine-agnostic verso cookie shape so a compatibility flip
/// does not lose cookie metadata before it reaches a concrete web backend.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>,
    pub expires: Option<f64>,
    pub partitioned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebFrameTransportMode {
    ImportedTexture,
    NativeChildOverlay,
    CpuSnapshot,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieAttributeCapabilities {
    pub same_site: WebFeatureStatus,
    pub partitioned: WebFeatureStatus,
    pub http_only: WebFeatureStatus,
    pub secure: WebFeatureStatus,
    pub expires: WebFeatureStatus,
}

impl Default for CookieAttributeCapabilities {
    fn default() -> Self {
        Self {
            same_site: WebFeatureStatus::unsupported("cookie SameSite support is unknown"),
            partitioned: WebFeatureStatus::unsupported("partitioned cookie support is unknown"),
            http_only: WebFeatureStatus::unsupported("HttpOnly cookie support is unknown"),
            secure: WebFeatureStatus::unsupported("secure cookie support is unknown"),
            expires: WebFeatureStatus::unsupported("cookie expiry support is unknown"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieCapabilities {
    pub read: WebFeatureStatus,
    pub write: WebFeatureStatus,
    pub delete: WebFeatureStatus,
    pub change_events: WebFeatureStatus,
    pub attributes: CookieAttributeCapabilities,
}

impl Default for CookieCapabilities {
    fn default() -> Self {
        Self {
            read: WebFeatureStatus::unsupported("cookie reads are not wired"),
            write: WebFeatureStatus::unsupported("cookie writes are not wired"),
            delete: WebFeatureStatus::unsupported("cookie deletes are not wired"),
            change_events: WebFeatureStatus::unsupported("cookie change events are not wired"),
            attributes: CookieAttributeCapabilities::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCapabilities {
    pub execute: WebFeatureStatus,
    pub result: WebFeatureStatus,
    pub exceptions: WebFeatureStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointerInputCapabilities {
    pub mouse: WebFeatureStatus,
    pub pen: WebFeatureStatus,
    pub touch: WebFeatureStatus,
    pub contact_geometry: WebFeatureStatus,
    pub pressure: WebFeatureStatus,
    pub tangential_pressure: WebFeatureStatus,
    pub tilt: WebFeatureStatus,
    pub twist: WebFeatureStatus,
    pub altitude_azimuth: WebFeatureStatus,
}

impl Default for PointerInputCapabilities {
    fn default() -> Self {
        Self {
            mouse: WebFeatureStatus::unsupported("pointer mouse input is not wired"),
            pen: WebFeatureStatus::unsupported("pen input is not wired"),
            touch: WebFeatureStatus::unsupported("touch input is not wired"),
            contact_geometry: WebFeatureStatus::unsupported("contact geometry is not wired"),
            pressure: WebFeatureStatus::unsupported("pointer pressure is not wired"),
            tangential_pressure: WebFeatureStatus::unsupported("tangential pressure is not wired"),
            tilt: WebFeatureStatus::unsupported("pointer tilt is not wired"),
            twist: WebFeatureStatus::unsupported("pointer twist is not wired"),
            altitude_azimuth: WebFeatureStatus::unsupported(
                "pointer altitude/azimuth are not wired",
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DragDropCapabilities {
    pub host_to_page: WebFeatureStatus,
    pub page_to_host: WebFeatureStatus,
    pub file_items: WebFeatureStatus,
    pub string_items: WebFeatureStatus,
}

impl Default for DragDropCapabilities {
    fn default() -> Self {
        Self {
            host_to_page: WebFeatureStatus::unsupported("host-to-page drag is not wired"),
            page_to_host: WebFeatureStatus::unsupported("page-to-host drag is not wired"),
            file_items: WebFeatureStatus::unsupported("dragged files are not wired"),
            string_items: WebFeatureStatus::unsupported("dragged strings are not wired"),
        }
    }
}

/// Runtime feature descriptor for web-surface capabilities that vary by
/// backend instance rather than by the Rust type alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSurfaceCapabilities {
    pub backend_name: String,
    pub backend_version: Option<String>,
    pub frame_transport: WebFrameTransportMode,
    pub cookie: CookieCapabilities,
    pub script: ScriptCapabilities,
    pub pointer: PointerInputCapabilities,
    /// The shared document-facing capability subset. This is the sole
    /// authority for hosted find, page zoom, capture, and navigation status.
    pub document: DocumentCapabilities,
    pub pdf: WebFeatureStatus,
    pub downloads: WebFeatureStatus,
    pub devtools: WebFeatureStatus,
    pub popups: WebFeatureStatus,
    pub permissions: WebFeatureStatus,
    pub auth: WebFeatureStatus,
    pub context_menus: WebFeatureStatus,
    pub drag_drop: DragDropCapabilities,
    pub ime_observability: WebFeatureStatus,
    pub accessibility: WebFeatureStatus,
    pub degradation_reasons: Vec<String>,
}

impl Default for WebSurfaceCapabilities {
    fn default() -> Self {
        Self {
            backend_name: "unknown".into(),
            backend_version: None,
            frame_transport: WebFrameTransportMode::Unsupported,
            cookie: CookieCapabilities::default(),
            script: ScriptCapabilities {
                execute: WebFeatureStatus::unsupported("script execution is not wired"),
                result: WebFeatureStatus::unsupported("script results are not wired"),
                exceptions: WebFeatureStatus::unsupported(
                    "script exception reporting is not wired",
                ),
            },
            pointer: PointerInputCapabilities::default(),
            document: DocumentCapabilities::default(),
            pdf: WebFeatureStatus::unsupported("PDF handling is not wired"),
            downloads: WebFeatureStatus::unsupported("download handling is not wired"),
            devtools: WebFeatureStatus::unsupported("devtools are not wired"),
            popups: WebFeatureStatus::unsupported("popup routing is not wired"),
            permissions: WebFeatureStatus::unsupported("permission prompts are not wired"),
            auth: WebFeatureStatus::unsupported("auth prompts are not wired"),
            context_menus: WebFeatureStatus::unsupported("context menu events are not wired"),
            drag_drop: DragDropCapabilities::default(),
            ime_observability: WebFeatureStatus::unsupported("IME observability is not wired"),
            accessibility: WebFeatureStatus::unsupported("surface accessibility is opaque"),
            degradation_reasons: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WebSurfaceEvent {
    Navigation(NavigationEvent),
    TitleChanged {
        title: String,
    },
    AddressChanged {
        url: String,
    },
    LoadProgress {
        value: f32,
    },
    /// Retained find state from an engine-managed page search.
    DocumentFindChanged(DocumentFindState),
    /// Completion of a request accepted by [`WebSurface::request_page_capture`].
    /// Errors retain the host-minted id so a host can settle the correct
    /// provenance record.
    PageCaptureCompleted {
        id: PageCaptureRequestId,
        result: Result<PageCaptureOutput, SurfaceError>,
    },
    /// Completion of an accepted cookie-read command.
    CookiesCompleted {
        id: WebRequestId,
        result: Result<Vec<Cookie>, SurfaceError>,
    },
    /// Completion of an accepted result-bearing script command.
    ScriptCompleted {
        id: WebRequestId,
        result: Result<String, SurfaceError>,
    },
    ConsoleMessage {
        level: String,
        text: String,
        source: Option<String>,
        line: Option<u32>,
    },
    ScriptException {
        text: String,
        source: Option<String>,
        line: Option<u32>,
    },
    PermissionRequested(PermissionRequest),
    AuthenticationRequested(HttpAuthenticationChallenge),
    DownloadRequested {
        url: String,
        suggested_name: Option<String>,
    },
    NewWindowRequested {
        url: String,
    },
    ContextMenuRequested {
        x: f64,
        y: f64,
        link_url: Option<String>,
        image_url: Option<String>,
    },
    /// A page began a drag. The host owns placement and the native drag loop;
    /// it must eventually answer through `finish_drag_source`.
    PageDragStarted {
        data_transfer: DataTransfer,
        position: PhysicalPosition,
    },
    CookieStoreChanged,
    ProcessCrashed {
        reason: String,
    },
    BackendDiagnostic {
        severity: String,
        message: String,
    },
    WebMessage(WebMessage),
}

// ── Settings ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSettings {
    /// Background fill color (RGBA). Informs pre-composited transparency.
    pub background_color: [u8; 4],
    /// Zoom factor (1.0 = 100 %).
    pub zoom_factor: f64,
    pub dev_tools: bool,
}

impl Default for SurfaceSettings {
    fn default() -> Self {
        Self {
            background_color: [255, 255, 255, 255],
            zoom_factor: 1.0,
            dev_tools: false,
        }
    }
}
