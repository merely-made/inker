// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Type translations between `inker` surface-engine types and the
//! `scrying` producer trait's vocabulary.
//!
//! Inker stays portable to wasm32 / browser targets so its event types are
//! a portable subset. Scrying is platform-bound and richer (drag input,
//! download events, accelerator keys, etc.). These functions are the bridge:
//! they map the inker subset into scrying's superset on the way in (input
//! forwarding) and lossy-map scrying's events to inker's subset on the way
//! out (host polling). Events that have no inker equivalent are dropped at
//! the boundary today; a richer host can downcast to the concrete scrying
//! producer for the full event stream.

use inker::{
    Cookie as InkerCookie, CookieAttributeCapabilities, CookieCapabilities,
    CursorShape as InkerCursorShape, DocumentCapabilities, DragDropCapabilities,
    FocusReason as InkerFocusReason, KeyboardEvent as InkerKeyboardEvent,
    KeyboardModifiers as InkerKeyboardModifiers, MouseButton as InkerMouseButton,
    MouseEvent as InkerMouseEvent, MouseEventKind as InkerMouseEventKind, NativeTextureHandle,
    NavigationEvent as InkerNavEvent, OwnedSurfaceFrame, PointerButtons,
    PointerEvent as InkerPointerEvent, PointerInputCapabilities, PointerPhase as InkerPointerPhase,
    PointerType as InkerPointerType, SameSite as InkerSameSite, ScriptCapabilities, SurfaceError,
    SurfaceFrame, SurfaceSettings, SurfaceSyncHandle, SurfaceTextureFormat, WebFeatureStatus,
    WebFrameTransportMode, WebMessage, WebRequestId as InkerWebRequestId,
    WebSurfaceCapabilities as InkerWebSurfaceCapabilities, WebSurfaceEvent,
};
use scrying::{
    CapabilityStatus as ScryingCapabilityStatus, CursorShape as ScryingCursorShape,
    FocusReason as ScryingFocusReason, KeyEventKind as ScryingKeyEventKind,
    KeyModifierFlags as ScryingKeyModifierFlags, KeyboardInput as ScryingKeyboardInput,
    MouseEventKind as ScryingMouseEventKind, MouseInput as ScryingMouseInput,
    MouseVirtualKeys as ScryingMouseVirtualKeys, NavigationEvent as ScryingNavEvent,
    PointerDevice as ScryingPointerDevice, PointerEventKind as ScryingPointerEventKind,
    PointerInput as ScryingPointerInput, SameSite as ScryingSameSite,
    SystemWebviewBackend as ScryingBackend, WebSurfaceCapabilities as ScryingCapabilities,
    WebSurfaceError, WebSurfaceEvent as ScryingWebSurfaceEvent, WebSurfaceFrame,
    WebSurfaceMode as ScryingSurfaceMode, WebSurfaceSettings,
    native_frame::NativeFrame as ScryingNativeFrame, native_frame::SyncMechanism,
};

// ── Errors ────────────────────────────────────────────────────────────

pub fn map_error(err: WebSurfaceError) -> SurfaceError {
    match err {
        WebSurfaceError::Unsupported(s) => SurfaceError::Unsupported(s.to_string()),
        WebSurfaceError::HostMigrationIndeterminate(s) => {
            SurfaceError::HostMigrationIndeterminate(s.to_string())
        },
        WebSurfaceError::NotReady(s) => SurfaceError::FrameAcquisitionFailed(s.to_string()),
        WebSurfaceError::Interop(e) => SurfaceError::FrameAcquisitionFailed(format!("{e}")),
        WebSurfaceError::Platform(s) => SurfaceError::SpawnFailed(s),
    }
}

// ── Frames ────────────────────────────────────────────────────────────

/// An owned Scry native frame held intact until its selected host importer
/// consumes it. Use [`into_scrying_native_frame`] to recover the frame from an
/// Inker [`SurfaceFrame`] before calling Scry's `TextureImporter`.
pub struct ScryingNativeFramePayload {
    frame: ScryingNativeFrame,
}

impl std::fmt::Debug for ScryingNativeFramePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScryingNativeFramePayload")
            .field("kind", &self.frame.kind())
            .finish()
    }
}

impl ScryingNativeFramePayload {
    fn new(frame: ScryingNativeFrame) -> Self {
        Self { frame }
    }

    /// Consume this payload, preserving Scry/Graft descriptor and retain
    /// custody for its native importer.
    pub fn into_native_frame(self) -> ScryingNativeFrame {
        self.frame
    }
}

impl OwnedSurfaceFrame for ScryingNativeFramePayload {
    fn payload_kind(&self) -> &'static str {
        "scrying.native-frame"
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// Recover Scry's owned native frame from a surface frame.
///
/// A mismatched payload leaves the complete `SurfaceFrame` intact for the
/// caller, including any other engine's ownership obligation.
pub fn into_scrying_native_frame(frame: SurfaceFrame) -> Result<ScryingNativeFrame, SurfaceFrame> {
    let SurfaceFrame {
        texture,
        sync,
        width,
        height,
        format,
        resource_epoch,
    } = frame;
    match texture.into_owned_payload::<ScryingNativeFramePayload>() {
        Ok(payload) => Ok(payload.into_native_frame()),
        Err(texture) => Err(SurfaceFrame {
            texture,
            sync,
            width,
            height,
            format,
            resource_epoch,
        }),
    }
}

/// Map Scry's frame enum to Inker's. Native frames retain their complete owned
/// payload; CPU snapshots and overlays remain unsupported by Inker's v1 frame
/// transport and return `None`.
pub fn map_frame(frame: WebSurfaceFrame, fence_handle: Option<u64>) -> Option<SurfaceFrame> {
    match frame {
        WebSurfaceFrame::Native(native) => map_native_frame(native, fence_handle),
        WebSurfaceFrame::CpuRgba { .. }
        | WebSurfaceFrame::PngSnapshot { .. }
        | WebSurfaceFrame::OverlayOnly => None,
        // `WebSurfaceFrame` is `#[non_exhaustive]`; unknown future variants
        // have no inker-shaped path until they're explicitly mapped.
        _ => None,
    }
}

fn map_native_frame(frame: ScryingNativeFrame, fence_handle: Option<u64>) -> Option<SurfaceFrame> {
    let (width, height, format, resource_epoch, sync) = match &frame {
        ScryingNativeFrame::Dx12SharedTexture(tex) => {
            let sync = match (tex.producer_sync, tex.fence_value, fence_handle) {
                (SyncMechanism::ExplicitFence, value, Some(handle)) if value > 0 => {
                    SurfaceSyncHandle::D3d12Fence { handle, value }
                },
                _ => SurfaceSyncHandle::None,
            };
            (
                tex.size.width,
                tex.size.height,
                tex.format,
                tex.generation,
                sync,
            )
        },
        ScryingNativeFrame::MetalTextureRef(tex) => (
            tex.size.width,
            tex.size.height,
            tex.format,
            tex.generation,
            SurfaceSyncHandle::None,
        ),
        ScryingNativeFrame::DmaBufImage(img) => (
            img.size.width,
            img.size.height,
            img.format,
            img.generation,
            SurfaceSyncHandle::None,
        ),
        // `NativeFrame` is `#[non_exhaustive]`; unknown future frames need an
        // explicit metadata mapping before Inker can describe them.
        _ => return None,
    };
    Some(SurfaceFrame {
        texture: NativeTextureHandle::OwnedPayload(Box::new(ScryingNativeFramePayload::new(frame))),
        sync,
        width,
        height,
        format: map_texture_format(format),
        resource_epoch,
    })
}

fn map_texture_format(format: wgpu::TextureFormat) -> SurfaceTextureFormat {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => SurfaceTextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb => SurfaceTextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm => SurfaceTextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => SurfaceTextureFormat::Bgra8UnormSrgb,
        other => SurfaceTextureFormat::Other(format!("{other:?}")),
    }
}

// ── Cookies ───────────────────────────────────────────────────────────

pub fn map_cookie(cookie: &InkerCookie) -> scrying::Cookie {
    scrying::Cookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain: cookie.domain.clone(),
        path: cookie.path.clone(),
        expires_at: cookie.expires,
        is_secure: cookie.secure,
        is_http_only: cookie.http_only,
        same_site: cookie.same_site.map(map_same_site),
        partitioned: cookie.partitioned,
    }
}

pub fn map_cookie_from_scrying(cookie: scrying::Cookie) -> InkerCookie {
    InkerCookie {
        name: cookie.name,
        value: cookie.value,
        domain: cookie.domain,
        path: cookie.path,
        secure: cookie.is_secure,
        http_only: cookie.is_http_only,
        same_site: cookie.same_site.map(map_same_site_from_scrying),
        expires: cookie.expires_at,
        partitioned: cookie.partitioned,
    }
}

fn map_same_site(same_site: InkerSameSite) -> ScryingSameSite {
    match same_site {
        InkerSameSite::Strict => ScryingSameSite::Strict,
        InkerSameSite::Lax => ScryingSameSite::Lax,
        InkerSameSite::None => ScryingSameSite::None,
    }
}

fn map_same_site_from_scrying(same_site: ScryingSameSite) -> InkerSameSite {
    match same_site {
        ScryingSameSite::Strict => InkerSameSite::Strict,
        ScryingSameSite::Lax => InkerSameSite::Lax,
        ScryingSameSite::None => InkerSameSite::None,
    }
}

// ── Input ─────────────────────────────────────────────────────────────

pub fn map_mouse(ev: InkerMouseEvent) -> ScryingMouseInput {
    let (kind, mouse_data) = mouse_kind_to_scrying(ev.kind);
    ScryingMouseInput {
        kind,
        virtual_keys: mouse_buttons_to_scrying(ev.button),
        mouse_data,
        point: (ev.position.x as i32, ev.position.y as i32),
    }
}

fn mouse_kind_to_scrying(kind: InkerMouseEventKind) -> (ScryingMouseEventKind, i32) {
    match kind {
        InkerMouseEventKind::Moved => (ScryingMouseEventKind::Move, 0),
        InkerMouseEventKind::Pressed => (ScryingMouseEventKind::LeftButtonDown, 0),
        InkerMouseEventKind::Released => (ScryingMouseEventKind::LeftButtonUp, 0),
        InkerMouseEventKind::ScrollPixels { delta_y, .. } => {
            (ScryingMouseEventKind::Wheel, delta_y as i32)
        },
        InkerMouseEventKind::ScrollLines { delta_y, .. } => {
            // Convert lines to wheel deltas (120 per line, Win32 convention).
            (ScryingMouseEventKind::Wheel, (delta_y * 120.0) as i32)
        },
    }
}

fn mouse_buttons_to_scrying(button: Option<InkerMouseButton>) -> ScryingMouseVirtualKeys {
    let mut vk = ScryingMouseVirtualKeys::default();
    match button {
        Some(InkerMouseButton::Left) => vk.left_button = true,
        Some(InkerMouseButton::Middle) => vk.middle_button = true,
        Some(InkerMouseButton::Right) => vk.right_button = true,
        Some(InkerMouseButton::Back) => vk.x_button1 = true,
        Some(InkerMouseButton::Forward) => vk.x_button2 = true,
        None => {},
    }
    vk
}

pub fn map_pointer(ev: InkerPointerEvent) -> Result<ScryingPointerInput, SurfaceError> {
    let pointer_id = u32::try_from(ev.pointer_id).map_err(|_| {
        SurfaceError::InputFailed(format!(
            "Pointer Events pointerId must be non-negative for scrying: {}",
            ev.pointer_id
        ))
    })?;
    let kind = match ev.phase {
        InkerPointerPhase::Down => ScryingPointerEventKind::Down,
        InkerPointerPhase::Move => ScryingPointerEventKind::Update,
        InkerPointerPhase::Up => ScryingPointerEventKind::Up,
        InkerPointerPhase::Cancel => ScryingPointerEventKind::CaptureChanged,
    };
    let device = match ev.pointer_type {
        InkerPointerType::Mouse => ScryingPointerDevice::Mouse,
        InkerPointerType::Pen => ScryingPointerDevice::Pen,
        InkerPointerType::Touch => ScryingPointerDevice::Touch,
        InkerPointerType::Unknown => {
            return Err(SurfaceError::Unsupported(
                "scrying cannot dispatch an unidentified pointer type".into(),
            ));
        },
    };
    Ok(ScryingPointerInput {
        kind,
        device,
        pointer_id,
        point: (ev.position.x as i32, ev.position.y as i32),
        pressure: ev
            .pressure
            .unwrap_or(if ev.buttons != PointerButtons::NONE {
                0.5
            } else {
                0.0
            }),
        tilt: (
            ev.tilt_x.unwrap_or(0.0).to_radians(),
            ev.tilt_y.unwrap_or(0.0).to_radians(),
        ),
    })
}

pub fn map_keyboard(ev: InkerKeyboardEvent) -> ScryingKeyboardInput {
    ScryingKeyboardInput {
        kind: if ev.pressed {
            ScryingKeyEventKind::Down
        } else {
            ScryingKeyEventKind::Up
        },
        virtual_key_code: ev.key_code,
        characters: ev.text.clone().unwrap_or_default(),
        characters_ignoring_modifiers: ev.text.unwrap_or_default(),
        modifiers: map_modifiers(ev.modifiers),
        is_repeat: false,
    }
}

fn map_modifiers(m: InkerKeyboardModifiers) -> ScryingKeyModifierFlags {
    ScryingKeyModifierFlags {
        shift: m.shift,
        control: m.ctrl,
        alt: m.alt,
        meta: m.meta,
        caps_lock: false,
    }
}

pub fn map_focus_reason(reason: InkerFocusReason) -> ScryingFocusReason {
    match reason {
        InkerFocusReason::Mouse | InkerFocusReason::Programmatic => {
            ScryingFocusReason::Programmatic
        },
        InkerFocusReason::Tab => ScryingFocusReason::Next,
        InkerFocusReason::ShiftTab => ScryingFocusReason::Previous,
    }
}

// ── Settings ──────────────────────────────────────────────────────────

pub fn map_settings(s: &SurfaceSettings) -> WebSurfaceSettings {
    WebSurfaceSettings {
        zoom_factor: Some(s.zoom_factor),
        user_agent: None,
        devtools_enabled: Some(s.dev_tools),
        javascript_enabled: None,
        default_context_menus_enabled: None,
        builtin_accelerator_keys_enabled: None,
        inactive_scheduling_policy: None,
    }
}

// ── Capabilities ──────────────────────────────────────────────────────

pub fn map_capabilities(caps: ScryingCapabilities) -> InkerWebSurfaceCapabilities {
    let transport_supported = capability_status(caps.imported_texture);
    let overlay_supported = capability_status(caps.native_child_overlay);
    let backend = caps.backend;
    let features = caps.features;
    let cookie_read = match backend {
        ScryingBackend::WkWebView => WebFeatureStatus::unsupported(
            "WKHTTPCookieStore has no exact URL-scoped query and host-only matching cannot be reconstructed safely",
        ),
        ScryingBackend::WebKitGtk => WebFeatureStatus::unsupported(
            "scrying's GTK producers expose only a blocking cookie-read path",
        ),
        _ => capability_status(features.cookies.read),
    };
    let basic_cookie_attrs = CookieAttributeCapabilities {
        same_site: capability_status(features.cookies.same_site),
        partitioned: capability_status(features.cookies.partitioned),
        http_only: capability_status(features.cookies.http_only),
        secure: capability_status(features.cookies.secure),
        expires: capability_status(features.cookies.expires),
    };
    InkerWebSurfaceCapabilities {
        backend_name: backend_name(backend).into(),
        backend_version: None,
        frame_transport: map_surface_mode(caps.preferred_mode),
        cookie: CookieCapabilities {
            read: cookie_read,
            write: capability_status(features.cookies.write),
            delete: WebFeatureStatus::unsupported(
                "scrying-engine does not yet project cookie deletion onto Inker's WebSurface",
            ),
            change_events: WebFeatureStatus::unsupported(
                "scrying-engine does not yet project cookie-store changes onto Inker's event stream",
            ),
            attributes: basic_cookie_attrs,
        },
        script: ScriptCapabilities {
            execute: capability_status(features.script.execute),
            result: capability_status(features.script.result),
            exceptions: capability_status(features.script.exceptions),
        },
        pointer: PointerInputCapabilities {
            mouse: WebFeatureStatus::Supported,
            pen: WebFeatureStatus::Partial {
                detail:
                    "device kind, id, pressure, and tilt survive; concrete system webviews vary"
                        .into(),
            },
            touch: WebFeatureStatus::Partial {
                detail: "multi-touch ids survive; concrete system webviews vary".into(),
            },
            contact_geometry: WebFeatureStatus::unsupported(
                "scrying's portable PointerInput has no contact geometry",
            ),
            pressure: WebFeatureStatus::Supported,
            tangential_pressure: WebFeatureStatus::unsupported(
                "scrying's portable PointerInput has no tangential pressure",
            ),
            tilt: WebFeatureStatus::Supported,
            twist: WebFeatureStatus::unsupported("scrying's portable PointerInput has no twist"),
            altitude_azimuth: WebFeatureStatus::unsupported(
                "scrying's portable PointerInput has no altitude/azimuth angles",
            ),
        },
        document: DocumentCapabilities {
            find_in_page: WebFeatureStatus::unsupported(
                "scrying-engine does not forward Inker's document find commands or results",
            ),
            page_zoom: WebFeatureStatus::unsupported(
                "scrying-engine forwards legacy zoom settings but does not implement typed set_page_zoom or its outcome",
            ),
            // P1 defines a correlated capture protocol. The legacy CPU
            // snapshot capability cannot satisfy it until this adapter accepts
            // requests and emits correlated completions.
            page_capture: WebFeatureStatus::unsupported(
                "scrying CPU snapshots are not wired to Inker's correlated page-capture protocol",
            ),
            navigation: WebFeatureStatus::Partial {
                detail: "navigation controls are forwarded to the concrete system webview backend"
                    .into(),
            },
        },
        pdf: WebFeatureStatus::unsupported(
            "scrying-engine does not expose the backend's PDF export commands or completions",
        ),
        downloads: projected_event_capability(
            features.downloads,
            "download requests are projected, but destination decisions, correlated lifecycle, and controls are not",
        ),
        devtools: capability_status(features.devtools),
        popups: projected_event_capability(
            features.popups,
            "new-window requests are projected, but popup-widget surfaces are not",
        ),
        permissions: WebFeatureStatus::unsupported(
            "scrying-engine does not expose retained permission requests or forward Inker's answers",
        ),
        auth: WebFeatureStatus::unsupported(
            "scrying-engine emits authentication diagnostics but does not forward Inker's credential answers",
        ),
        context_menus: match backend {
            ScryingBackend::WebView2 | ScryingBackend::WkWebView => WebFeatureStatus::Partial {
                detail: "context-menu position and link/image targets are projected; backend menu items and decisions are not".into(),
            },
            _ => WebFeatureStatus::unsupported(
                "scrying-engine has no verified context-menu event source for this backend",
            ),
        },
        drag_drop: DragDropCapabilities {
            host_to_page: WebFeatureStatus::unsupported(
                "scrying's portable DragInput does not carry DataTransfer items",
            ),
            page_to_host: WebFeatureStatus::unsupported(
                "scrying does not expose a portable page drag-start event",
            ),
            file_items: WebFeatureStatus::unsupported(
                "file payload forwarding requires a backend-native drag object",
            ),
            string_items: WebFeatureStatus::unsupported(
                "string payload forwarding is not present in scrying's portable drag API",
            ),
        },
        ime_observability: WebFeatureStatus::unsupported(
            "scrying-engine emits text-input diagnostics but does not project composition or caret geometry",
        ),
        accessibility: WebFeatureStatus::unsupported(
            "scrying-engine does not project an accessibility tree through Inker",
        ),
        degradation_reasons: std::iter::once(caps.reason.to_owned())
            .chain(features.degradation_reasons.into_iter().map(str::to_owned))
            .chain([
                format!("imported_texture={transport_supported:?}"),
                format!("native_child_overlay={overlay_supported:?}"),
            ])
            .collect(),
    }
}

// A projected request event is useful without completing the backend's control
// path. Keep that implemented subset partial, including the backend's own
// limits, rather than inheriting its fuller producer capability.
fn projected_event_capability(
    status: ScryingCapabilityStatus,
    projection: &str,
) -> WebFeatureStatus {
    match status {
        ScryingCapabilityStatus::Supported => WebFeatureStatus::Partial {
            detail: projection.into(),
        },
        ScryingCapabilityStatus::Partial(detail) => WebFeatureStatus::Partial {
            detail: format!("{detail}; {projection}"),
        },
        status => capability_status(status),
    }
}

fn capability_status(status: ScryingCapabilityStatus) -> WebFeatureStatus {
    match status {
        ScryingCapabilityStatus::Supported => WebFeatureStatus::Supported,
        ScryingCapabilityStatus::Partial(detail) => WebFeatureStatus::Partial {
            detail: detail.into(),
        },
        ScryingCapabilityStatus::Unsupported(reason) => WebFeatureStatus::Unsupported {
            reason: format!("{reason:?}"),
        },
    }
}

fn map_surface_mode(mode: ScryingSurfaceMode) -> WebFrameTransportMode {
    match mode {
        ScryingSurfaceMode::ImportedTexture => WebFrameTransportMode::ImportedTexture,
        ScryingSurfaceMode::NativeChildOverlay => WebFrameTransportMode::NativeChildOverlay,
        ScryingSurfaceMode::CpuSnapshot => WebFrameTransportMode::CpuSnapshot,
        ScryingSurfaceMode::Unsupported => WebFrameTransportMode::Unsupported,
        _ => WebFrameTransportMode::Unsupported,
    }
}

fn backend_name(backend: ScryingBackend) -> &'static str {
    match backend {
        ScryingBackend::WebView2 => "scrying.webview2",
        ScryingBackend::WkWebView => "scrying.wkwebview",
        ScryingBackend::Wpe => "scrying.wpe",
        ScryingBackend::WebKitGtk => "scrying.webkitgtk",
        ScryingBackend::Unknown => "scrying.unknown",
        _ => "scrying.unknown",
    }
}

mod events;
pub use events::*;

#[cfg(test)]
mod tests;
