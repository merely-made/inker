// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Surface-engine traits and registry — parallel dispatch path for
//! long-lived, frame-streaming engines alongside [`crate::engine`].
//!
//! Document engines are request/response: fetch → render → `EngineDocument`.
//! Surface engines are lifecycle-bound: spawn → long-lived session producing
//! a composited-frame stream + events until torn down. Both registries
//! coexist; the host dispatches through whichever holds the resolved engine ID
//! (document registry for `nematic.*` / `genet.web`; surface registry for
//! `scrying.web`).

use std::{any::Any, collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

/// Typed guest-tree action, including its original target tree, node and data.
pub use accesskit::ActionRequest as SurfaceAccessibilityActionRequest;
/// Native guest-tree identity, retained exactly across the producer boundary.
pub use accesskit::TreeId as SurfaceAccessibilityTreeId;
/// One ordered native semantic update, including any nested tree references.
pub use accesskit::TreeUpdate as SurfaceAccessibilityUpdate;

pub use crate::WebFeatureStatus;
use crate::presentation::PresentationDeclaration;
use crate::routing::EngineRouteDecision;
use crate::session_engine::{
    DocumentFindDirection, DocumentFindQuery, DocumentFindState, DocumentZoomState,
};
use crate::{
    A11yCapability, DocumentCapabilities, PageCaptureOutput, PageCaptureRequest,
    PageCaptureRequestId,
};

// ── User-agent requests ────────────────────────────────────────────────────

/// Correlates one user-agent policy request with exactly one later answer.
///
/// The id is scoped to the [`WebSurface`] that emitted it. Hosts which drive
/// several surfaces must retain the surface identity beside this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UserAgentRequestId(u64);

impl UserAgentRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Correlates one accepted result-bearing web command with exactly one later
/// completion on the surface's ordered event stream.
///
/// The caller mints this id. It is scoped to one [`WebSurface`]; a host driving
/// several surfaces must retain the surface identity beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WebRequestId(u64);

impl WebRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}
mod permission;
pub use permission::*;

// ── Errors ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceError {
    EngineNotFound(String),
    SpawnFailed(String),
    NavigationFailed(String),
    InputFailed(String),
    FrameAcquisitionFailed(String),
    Busy {
        operation: String,
    },
    Unsupported(String),
    /// A native host move failed after the producer could not establish which
    /// HWND is authoritative. Source custody must not be resumed in this state.
    HostMigrationIndeterminate(String),
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineNotFound(id) => write!(f, "surface engine not registered: {id}"),
            Self::SpawnFailed(reason) => write!(f, "spawn failed: {reason}"),
            Self::NavigationFailed(reason) => write!(f, "navigation failed: {reason}"),
            Self::InputFailed(reason) => write!(f, "input failed: {reason}"),
            Self::FrameAcquisitionFailed(reason) => write!(f, "frame acquisition: {reason}"),
            Self::Busy { operation } => write!(f, "busy: {operation}"),
            Self::Unsupported(reason) => write!(f, "unsupported: {reason}"),
            Self::HostMigrationIndeterminate(reason) => {
                write!(f, "host migration indeterminate: {reason}")
            },
        }
    }
}

impl std::error::Error for SurfaceError {}

/// A live native host to which a long-lived surface producer may be moved.
///
/// This is an ephemeral platform handle, not a durable window identifier. The
/// caller must keep the native host alive and ensure the rehost operation runs
/// on the producer's owning thread.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeSurfaceHost {
    /// Windows top-level or child window handle.
    Win32 { hwnd: std::num::NonZeroIsize },
}

// ── Spawn request ──────────────────────────────────────────────────────────

/// Persona/session binding passed to the surface engine at spawn time.
///
/// The host resolves `user_data_dir` from persona + graph context before
/// constructing the request. The engine plumbs it to the producer's data-store
/// config (e.g. `WebView2CompositionConfig::user_data_dir` on Windows).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineProfileBinding {
    pub user_data_dir: String,
}

/// Input to [`SurfaceEngine::spawn`].
///
/// Bypasses the inker fetch path entirely — the underlying WebView manages
/// its own HTTP stack; there is no raw body to hand in.
#[derive(Clone, Debug)]
pub struct SurfaceSpawnRequest {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub profile: EngineProfileBinding,
    /// Platform fence share-handle for explicit GPU sync. `None` falls back
    /// to the producer's barrier/cache path. Windows: D3D12 fence HANDLE cast
    /// to u64. Other platforms: reserved.
    pub fence_handle: Option<u64>,
}

mod frame;
pub use frame::*;

mod input;
pub use input::*;

mod events;
pub use events::*;

// ── Traits ─────────────────────────────────────────────────────────────────

/// Factory for [`SurfaceProducer`] instances.
///
/// Parallel to [`crate::Engine`] for surface-producing engines. A single
/// `SurfaceEngine` may spawn many producers (one per tile).
pub trait SurfaceEngine: Send + Sync {
    /// Stable engine identifier. Must match the `engine_id` of the
    /// [`EngineRouteDecision`] that selected this engine.
    fn engine_id(&self) -> &str;

    /// Spawn a new producer for the given request.
    fn spawn(
        &self,
        request: &SurfaceSpawnRequest,
    ) -> Result<Box<dyn SurfaceProducer>, SurfaceError>;

    /// This surface's accessibility capability (see [`crate::a11y`]).
    /// Frame-streaming surfaces default to [`A11yCapability::Opaque`] — a raw
    /// GPU frame / system WebView has no semantics the host can read. A surface
    /// that *bridges* its content (e.g. scrying's DOM bridge) overrides this to
    /// declare [`A11yCapability::Partial`], per the non-silent-degradation rule.
    fn a11y_capability(&self) -> A11yCapability {
        A11yCapability::Opaque
    }

    /// This engine's declaration (see [`crate::presentation`]). The default
    /// is a surface that presents nothing by scheme, reachable by pin or
    /// override; an engine that presents more, or keeps its id and
    /// accessibility in the declaration, overrides it.
    fn declaration(&self) -> PresentationDeclaration {
        PresentationDeclaration::surface(self.engine_id()).a11y(self.a11y_capability())
    }
}

/// Long-lived surface producer. Owns a WebView control until dropped.
///
/// All methods take `&mut self`: the producer is single-owner, driven
/// sequentially by the host's render loop. Input flows in through `send_*`
/// and `move_focus`; output flows out through `acquire_frame` and `poll_*`.
///
/// Not `Send`: producers may be STA-bound (Windows WebView2 COM) or
/// main-thread-only (macOS WKWebView, gpui main thread). The host drives them
/// from a single thread per producer.
pub trait SurfaceProducer {
    // ── Layout ──────────────────────────────────────────────────────────────
    fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError>;
    fn set_offset(&mut self, x: i32, y: i32) -> Result<(), SurfaceError>;

    // ── Frame acquisition ────────────────────────────────────────────────────
    fn acquire_frame(&mut self) -> Result<Option<SurfaceFrame>, SurfaceError>;

    // ── Input ────────────────────────────────────────────────────────────────
    fn send_mouse_input(&mut self, ev: MouseEvent) -> Result<(), SurfaceError>;
    fn send_pointer_input(&mut self, ev: PointerEvent) -> Result<(), SurfaceError>;
    fn send_wheel_input(&mut self, event: SurfaceWheelEvent) -> Result<(), SurfaceError> {
        if event.modifiers != KeyboardModifiers::default() {
            return Err(SurfaceError::Unsupported(
                "modified wheel input is not wired for this surface".into(),
            ));
        }
        self.send_mouse_input(MouseEvent {
            position: event.position,
            button: None,
            kind: MouseEventKind::ScrollPixels {
                delta_x: event.delta.0,
                delta_y: event.delta.1,
            },
        })
    }
    fn set_focus(&mut self, focused: bool) -> Result<(), SurfaceError> {
        if focused {
            self.move_focus(FocusReason::Programmatic)
        } else {
            Err(SurfaceError::Unsupported(
                "focus loss is not wired for this surface".into(),
            ))
        }
    }

    fn send_drag_input(&mut self, _ev: DragEvent) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "host-to-page drag input is not wired for this surface".into(),
        ))
    }
    fn finish_drag_source(
        &mut self,
        _position: PhysicalPosition,
        _operation: DragOperationSet,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "page-to-host drag completion is not wired for this surface".into(),
        ))
    }
    fn send_keyboard_input(&mut self, ev: KeyboardEvent) -> Result<(), SurfaceError>;
    fn move_focus(&mut self, reason: FocusReason) -> Result<(), SurfaceError>;

    /// Move the producer's native embedding to another host window.
    ///
    /// This is unsafe because the host handle is borrowed by convention and
    /// the producer may retain it after this call. Implementations must either
    /// complete the move atomically or leave the producer attached to its
    /// previous host. `HostMigrationIndeterminate` is the terminal exception:
    /// the native owner could not establish either host, so the caller must
    /// retain the destination and must not resume ordinary source presentation.
    unsafe fn rehost(&mut self, _host: NativeSurfaceHost) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "surface producer rehosting is not wired for this surface".into(),
        ))
    }

    // ── Events ───────────────────────────────────────────────────────────────
    fn poll_cursor_shape(&mut self) -> Option<CursorShape>;

    // ── Settings ─────────────────────────────────────────────────────────────
    fn apply_settings(&mut self, settings: &SurfaceSettings) -> Result<(), SurfaceError>;

    /// Present the hosted document at an absolute page-zoom `factor`
    /// (1.0 = 100 %), the same user-agent document scale retained sessions
    /// answer through [`crate::DocumentSession::set_page_zoom`].
    ///
    /// This is the typed spelling of [`SurfaceSettings::zoom_factor`]: a
    /// producer that implements it lets a host change zoom alone, without
    /// resending an otherwise unrelated settings blob whose remaining fields it
    /// would have to reconstruct. Producers still on `apply_settings` leave this
    /// unsupported, and the returned state names the bounds they enforce.
    fn set_page_zoom(&mut self, _factor: f32) -> Result<DocumentZoomState, SurfaceError> {
        Err(SurfaceError::Unsupported(
            "page zoom is not wired for this surface".into(),
        ))
    }

    // ── Optional native accessibility ────────────────────────────────────────

    /// Activate or deactivate this producer's native semantic export.
    ///
    /// Activation returns the guest root tree identity; deactivation returns
    /// `None` and retires the producer's pending semantic updates. An active
    /// exporter must return `Some`, or an explicit error if unavailable.
    /// The host must publish its graft node before forwarding guest updates
    /// to the OS adapter. Retaining/validating updates may precede publication.
    /// This protocol does not itself upgrade an accessibility capability.
    fn set_accessibility_active(
        &mut self,
        _active: bool,
    ) -> Result<Option<SurfaceAccessibilityTreeId>, SurfaceError> {
        Err(SurfaceError::Unsupported(
            "native accessibility activation is not wired for this surface".into(),
        ))
    }

    /// Drain one semantic update in the producer's original callback order.
    ///
    /// Preserve all tree identities and nested graft references. Updates are
    /// independent of GPU frame acquisition; the host owns wake/publication,
    /// identity validation, pane bounds, and lifecycle generation checks.
    fn poll_accessibility_update(&mut self) -> Option<SurfaceAccessibilityUpdate> {
        None
    }

    /// Request a fresh initialization stream and return its root tree identity.
    ///
    /// A producer may reactivate its exporter and replace the root identity.
    /// The host must retire the old graft before requesting resynchronization,
    /// then publish the returned root's graft before forwarding the new updates
    /// to the OS adapter. Retaining/validating updates may precede publication.
    /// Old pending updates must not be replayed into the new activation.
    fn request_accessibility_resync(&mut self) -> Result<SurfaceAccessibilityTreeId, SurfaceError> {
        Err(SurfaceError::Unsupported(
            "native accessibility resynchronization is not wired for this surface".into(),
        ))
    }

    /// Deliver a supported typed action without rewriting its target or data.
    ///
    /// The host validates current tree/node ownership, generation and advertised
    /// actions. The producer still explicitly refuses unsupported operations.
    fn send_accessibility_action(
        &mut self,
        _request: SurfaceAccessibilityActionRequest,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "native accessibility action delivery is not wired for this surface".into(),
        ))
    }

    // ── Optional web control plane ───────────────────────────────────────────
    fn as_web_surface(&mut self) -> Option<&mut dyn WebSurface> {
        None
    }
}

/// Web-specific control plane layered over the raw surface transport.
///
/// Navigation methods start work and return promptly. Completion and every
/// other web-facing notification are observed through one ordered event
/// stream from the driving frame loop. Consumers must drain
/// [`WebSurface::poll_web_event`] directly: filtered convenience pollers can
/// discard intervening events and therefore are deliberately not part of this
/// contract.
pub trait WebSurface: SurfaceProducer {
    fn capabilities(&self) -> WebSurfaceCapabilities {
        WebSurfaceCapabilities::default()
    }

    /// The document-control subset of this hosted surface's capabilities.
    fn document_capabilities(&self) -> DocumentCapabilities {
        self.capabilities().document
    }

    /// Start a viewport capture. Completion is emitted in producer order as a
    /// [`WebSurfaceEvent::PageCaptureCompleted`] carrying this request's id.
    ///
    /// An implementation limited to one capture in flight returns
    /// [`SurfaceError::Busy`] before accepting another request.
    fn request_page_capture(&mut self, _request: PageCaptureRequest) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "page capture is not wired for this web surface".into(),
        ))
    }

    // ── Navigation ───────────────────────────────────────────────────────────
    fn navigate_to_url(&mut self, url: &str) -> Result<(), SurfaceError>;
    fn navigate_to_string(&mut self, html: &str) -> Result<(), SurfaceError>;
    fn reload(&mut self) -> Result<(), SurfaceError>;
    fn stop(&mut self) -> Result<(), SurfaceError>;
    fn go_back(&mut self) -> Result<(), SurfaceError>;
    fn go_forward(&mut self) -> Result<(), SurfaceError>;
    fn can_go_back(&self) -> bool;
    fn can_go_forward(&self) -> bool;

    /// Start or step the hosted engine's page search. Results arrive through
    /// [`WebSurfaceEvent::DocumentFindChanged`], preserving the surface event
    /// order rather than adding a filtered result poller.
    fn document_find(
        &mut self,
        _query: &DocumentFindQuery,
        _direction: DocumentFindDirection,
        _find_next: bool,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "document find is not wired for this web surface".into(),
        ))
    }

    fn clear_document_find(&mut self) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "document find is not wired for this web surface".into(),
        ))
    }

    // ── Session/script/events ────────────────────────────────────────────────
    fn set_cookie(&mut self, cookie: &Cookie) -> Result<(), SurfaceError>;
    /// Start a URL-scoped cookie read. Accepted work settles exactly once as a
    /// [`WebSurfaceEvent::CookiesCompleted`] event carrying `id`.
    fn request_cookies_for_url(
        &mut self,
        _id: WebRequestId,
        _url: &str,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "cookie reads are not wired for this web surface".into(),
        ))
    }
    fn delete_cookie(&mut self, cookie: &Cookie) -> Result<(), SurfaceError> {
        let _ = cookie;
        Err(SurfaceError::Unsupported(
            "cookie delete is not wired for this web surface".into(),
        ))
    }
    /// Answer a held Permissions API request emitted on this surface.
    fn answer_permission(
        &mut self,
        _id: UserAgentRequestId,
        _answer: PermissionAnswer,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "permission answers are not wired for this web surface".into(),
        ))
    }
    /// Answer or cancel a held RFC 9110 authentication challenge.
    fn answer_http_authentication(
        &mut self,
        _id: UserAgentRequestId,
        _answer: &HttpAuthenticationAnswer,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "authentication answers are not wired for this web surface".into(),
        ))
    }
    /// Start result-bearing script execution. Accepted work settles exactly
    /// once as a [`WebSurfaceEvent::ScriptCompleted`] event carrying `id`.
    fn request_script_result(
        &mut self,
        _id: WebRequestId,
        _script: &str,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "script results are not wired for this web surface".into(),
        ))
    }
    /// Return the next event in producer order.
    ///
    /// An implementation must not inspect and discard events of another kind
    /// while answering this call. Asynchronous commands will add correlation
    /// ids to this stream. An accepted result-bearing command must settle once;
    /// synchronous acceptance failure emits no completion.
    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        None
    }
}

// ── Registry ───────────────────────────────────────────────────────────────

/// Engine ID → `SurfaceEngine` instance dispatch. Parallel to
/// [`crate::EngineRegistry`] for the surface dispatch path.
#[derive(Default)]
pub struct SurfaceEngineRegistry {
    engines: HashMap<String, Box<dyn SurfaceEngine>>,
}

impl SurfaceEngineRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, engine: Box<dyn SurfaceEngine>) {
        let id = engine.engine_id().to_string();
        self.engines.insert(id, engine);
    }

    pub fn engine(&self, id: &str) -> Option<&dyn SurfaceEngine> {
        self.engines.get(id).map(|e| e.as_ref())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.engines.contains_key(id)
    }

    pub fn engine_ids(&self) -> impl Iterator<Item = &str> {
        self.engines.keys().map(String::as_str)
    }

    /// Spawn the selected platform browser on the engine-neutral contract.
    /// The host/engine pair supplies its concrete non-blocking frame custody
    /// conversion; borrowed handles are never guessed into owned shared frames.
    pub fn spawn_shared(
        &self,
        decision: &EngineRouteDecision,
        request: &SurfaceSpawnRequest,
        scope: webcue::Scope,
        viewport: webcue::Viewport,
        presentation: webcue::PresentationMode,
        frames: crate::cue_browser::FrameReader,
    ) -> webcue::Result<crate::cue_browser::BrowserSurface> {
        let producer = self
            .spawn(decision, request)
            .map_err(|error| webcue::Error::Backend(error.to_string()))?;
        crate::cue_browser::BrowserSurface::new(producer, scope, viewport, presentation, frames)
    }

    /// Spawn a producer using the engine selected by `decision.engine_id`.
    #[tracing::instrument(
        level = "debug",
        skip(self, decision, request),
        fields(engine_id = %decision.engine_id, url = %request.url),
    )]
    pub fn spawn(
        &self,
        decision: &EngineRouteDecision,
        request: &SurfaceSpawnRequest,
    ) -> Result<Box<dyn SurfaceProducer>, SurfaceError> {
        let engine = self.engine(&decision.engine_id).ok_or_else(|| {
            tracing::warn!(
                engine_id = %decision.engine_id,
                "surface engine not registered"
            );
            SurfaceError::EngineNotFound(decision.engine_id.clone())
        })?;
        engine.spawn(request)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
