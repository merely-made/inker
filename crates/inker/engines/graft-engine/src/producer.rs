// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! [`GraftSurface`] — the host seam (graft's composite producer, since graft has
//! no single library producer type) — and [`GraftProducer`], the adapter that
//! satisfies `inker::SurfaceProducer` over it.

use inker::{
    Cookie, CursorShape, DocumentCapabilities, DragEvent, DragOperationSet, FocusReason,
    KeyboardEvent, MouseEvent, NativeTextureHandle, NavigationEvent, PhysicalPosition,
    PointerEvent, SurfaceAccessibilityActionRequest, SurfaceAccessibilityTreeId,
    SurfaceAccessibilityUpdate, SurfaceError, SurfaceFrame, SurfaceProducer, SurfaceSettings,
    SurfaceSyncHandle, SurfaceTextureFormat, WebFeatureStatus, WebFrameTransportMode, WebMessage,
    WebRequestId, WebSurface, WebSurfaceCapabilities, WebSurfaceEvent,
};

/// A frame produced by a [`GraftSurface`]: the shared GPU texture handle the host
/// imports, plus the `resource_epoch` it maps straight onto `inker::SurfaceFrame`.
pub struct GraftFrame {
    /// A platform handle or owned payload paired with the host's importer.
    /// Owned payloads retain native custody, or a texture already imported on
    /// the host device. The host validates the concrete engine payload.
    pub texture: NativeTextureHandle,
    pub sync: SurfaceSyncHandle,
    pub width: u32,
    pub height: u32,
    pub format: SurfaceTextureFormat,
    /// Identity of the underlying allocation, following SurfaceFrame's epoch
    /// contract. A host which returns a fresh normalized texture each paint
    /// advances this every paint. Grafting's content generation alone is not
    /// proof that a shared allocation was replaced; producer/importer pairing
    /// owns that distinction and synchronization is required for every paint.
    pub resource_epoch: u64,
}

/// The host-implemented graft composite: a `servo::Servo` instance + `WebView` +
/// `servo_wgpu_interop_adapter::ServoWgpuInteropAdapter`. This crate cannot
/// fabricate any of those (it does not depend on Servo), so it defines the seam
/// and the host wires it. The live grafting/Servo calls each method maps to,
/// when the Servo lane is built:
///
/// - [`resize`](GraftSurface::resize) → `WebView::resize`, which owns the
///   rendering-context resize as well as the document viewport update.
/// - [`acquire_frame`](GraftSurface::acquire_frame) → `Servo::spin_event_loop()`,
///   then `WebView::paint()` and the adapter's pre-present
///   `take_imported_texture()` path. Importing the GL buffer after its swap can
///   select stale content. CPU readback is a separately reported transport.
/// - [`load_url`](GraftSurface::load_url) / [`load_html`](GraftSurface::load_html)
///   → `WebViewBuilder` / `WebView::load`.
/// - `go_back` / `go_forward` → `WebView::go_back` / `go_forward`.
/// - `notify_*` → `WebView::notify_input_event(servo::InputEvent::…)`.
/// - `poll_*` → drained from the `WebViewDelegate` callbacks the host registers.
///
/// Not `Send`: a graft surface owns Servo's non-`Send` GL context; the host
/// drives all views sharing one process-owned Servo instance on its UI thread.
pub trait GraftSurface {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError>;

    /// Pump Servo and return the latest frame, if a new composited frame is
    /// ready. `Ok(None)` when nothing new this tick.
    fn acquire_frame(&mut self) -> Result<Option<GraftFrame>, SurfaceError>;

    fn load_url(&mut self, url: &str) -> Result<(), SurfaceError>;
    fn load_html(&mut self, html: &str) -> Result<(), SurfaceError>;
    fn reload(&mut self) -> Result<(), SurfaceError>;
    fn stop(&mut self) -> Result<(), SurfaceError>;
    fn go_back(&mut self) -> Result<(), SurfaceError>;
    fn go_forward(&mut self) -> Result<(), SurfaceError>;
    fn can_go_back(&self) -> bool;
    fn can_go_forward(&self) -> bool;

    fn notify_mouse(&mut self, ev: MouseEvent) -> Result<(), SurfaceError>;
    fn notify_pointer(&mut self, ev: PointerEvent) -> Result<(), SurfaceError>;
    fn notify_drag(&mut self, ev: DragEvent) -> Result<(), SurfaceError>;
    fn finish_drag_source(
        &mut self,
        position: PhysicalPosition,
        operation: DragOperationSet,
    ) -> Result<(), SurfaceError>;
    fn notify_keyboard(&mut self, ev: KeyboardEvent) -> Result<(), SurfaceError>;
    fn focus(&mut self, reason: FocusReason) -> Result<(), SurfaceError>;

    fn poll_navigation_event(&mut self) -> Option<NavigationEvent>;
    fn poll_cursor_shape(&mut self) -> Option<CursorShape>;
    fn poll_web_message(&mut self) -> Option<WebMessage>;

    /// Drain one event in the producer's callback order. Hosts with a single
    /// delegate queue override this method to retain ordering across navigation,
    /// title/address changes, messages, and correlated completions. The default
    /// preserves the legacy separate-queue behavior for existing implementers.
    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        self.poll_navigation_event()
            .map(WebSurfaceEvent::Navigation)
            .or_else(|| self.poll_web_message().map(WebSurfaceEvent::WebMessage))
    }

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

    fn web_capabilities(&self) -> WebSurfaceCapabilities {
        let mut caps = WebSurfaceCapabilities {
            backend_name: "graft.servo".into(),
            frame_transport: WebFrameTransportMode::ImportedTexture,
            ..WebSurfaceCapabilities::default()
        };
        caps.script.execute = WebFeatureStatus::Partial {
            detail: "Servo script execution depends on the host GraftSurface implementation".into(),
        };
        caps.script.result = WebFeatureStatus::Unsupported {
            reason: "Servo result-bearing script control is not wired through graft-engine yet"
                .into(),
        };
        caps.cookie.write = WebFeatureStatus::Unsupported {
            reason: "Servo cookie control is not wired through graft-engine yet".into(),
        };
        caps.popups = WebFeatureStatus::Partial {
            detail: "popup routing depends on Servo delegate callbacks".into(),
        };
        caps.context_menus = WebFeatureStatus::Partial {
            detail: "context menu routing depends on Servo delegate callbacks".into(),
        };
        caps.document = DocumentCapabilities {
            page_zoom: WebFeatureStatus::Partial {
                detail: "page zoom depends on the host GraftSurface implementation".into(),
            },
            page_capture: WebFeatureStatus::unsupported(
                "graft page capture is not wired to Inker's page-capture protocol",
            ),
            navigation: WebFeatureStatus::Partial {
                detail: "navigation controls depend on the host GraftSurface implementation".into(),
            },
            ..DocumentCapabilities::default()
        };
        caps.degradation_reasons.push(
            "graft-engine reports Servo-backed controls only when the host GraftSurface wires them"
                .into(),
        );
        caps
    }

    fn set_cookie(&mut self, _cookie: &Cookie) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "graft-engine cookie control is not wired yet".into(),
        ))
    }

    fn request_cookies_for_url(
        &mut self,
        _id: WebRequestId,
        _url: &str,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "graft-engine cookie reads are not wired yet".into(),
        ))
    }

    fn delete_cookie(&mut self, _cookie: &Cookie) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "graft-engine cookie delete is not wired yet".into(),
        ))
    }

    fn request_script_result(
        &mut self,
        _id: WebRequestId,
        _script: &str,
    ) -> Result<(), SurfaceError> {
        Err(SurfaceError::Unsupported(
            "graft-engine script result control is not wired yet".into(),
        ))
    }

    fn apply_settings(&mut self, settings: &SurfaceSettings) -> Result<(), SurfaceError>;
}

/// Adapts a `Box<dyn GraftSurface>` onto `inker::SurfaceProducer`.
pub struct GraftProducer {
    inner: Box<dyn GraftSurface>,
}

impl GraftProducer {
    pub fn new(inner: Box<dyn GraftSurface>) -> Self {
        Self { inner }
    }
}

impl SurfaceProducer for GraftProducer {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError> {
        self.inner.resize(width, height)
    }

    /// No-op: a graft surface renders offscreen and the host composites the
    /// imported texture at the tile rect, so there is no producer-side on-host
    /// visual to offset (unlike a scrying WebView2 composition visual).
    fn set_offset(&mut self, _x: i32, _y: i32) -> Result<(), SurfaceError> {
        Ok(())
    }

    fn acquire_frame(&mut self) -> Result<Option<SurfaceFrame>, SurfaceError> {
        // GraftFrame maps 1:1 onto SurfaceFrame now that the contract carries
        // `resource_epoch` (the host's import cache reads it directly).
        Ok(self.inner.acquire_frame()?.map(|f| SurfaceFrame {
            texture: f.texture,
            sync: f.sync,
            width: f.width,
            height: f.height,
            format: f.format,
            resource_epoch: f.resource_epoch,
        }))
    }

    fn send_mouse_input(&mut self, ev: MouseEvent) -> Result<(), SurfaceError> {
        self.inner.notify_mouse(ev)
    }

    fn send_pointer_input(&mut self, ev: PointerEvent) -> Result<(), SurfaceError> {
        self.inner.notify_pointer(ev)
    }

    fn send_drag_input(&mut self, ev: DragEvent) -> Result<(), SurfaceError> {
        self.inner.notify_drag(ev)
    }

    fn finish_drag_source(
        &mut self,
        position: PhysicalPosition,
        operation: DragOperationSet,
    ) -> Result<(), SurfaceError> {
        self.inner.finish_drag_source(position, operation)
    }

    fn send_keyboard_input(&mut self, ev: KeyboardEvent) -> Result<(), SurfaceError> {
        self.inner.notify_keyboard(ev)
    }

    fn move_focus(&mut self, reason: FocusReason) -> Result<(), SurfaceError> {
        self.inner.focus(reason)
    }

    fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
        self.inner.poll_cursor_shape()
    }

    fn apply_settings(&mut self, settings: &SurfaceSettings) -> Result<(), SurfaceError> {
        self.inner.apply_settings(settings)
    }

    fn set_accessibility_active(
        &mut self,
        active: bool,
    ) -> Result<Option<SurfaceAccessibilityTreeId>, SurfaceError> {
        self.inner.set_accessibility_active(active)
    }

    fn poll_accessibility_update(&mut self) -> Option<SurfaceAccessibilityUpdate> {
        self.inner.poll_accessibility_update()
    }

    fn request_accessibility_resync(&mut self) -> Result<SurfaceAccessibilityTreeId, SurfaceError> {
        self.inner.request_accessibility_resync()
    }

    fn send_accessibility_action(
        &mut self,
        request: SurfaceAccessibilityActionRequest,
    ) -> Result<(), SurfaceError> {
        self.inner.send_accessibility_action(request)
    }

    fn as_web_surface(&mut self) -> Option<&mut dyn WebSurface> {
        Some(self)
    }
}

impl WebSurface for GraftProducer {
    fn capabilities(&self) -> WebSurfaceCapabilities {
        self.inner.web_capabilities()
    }

    fn navigate_to_url(&mut self, url: &str) -> Result<(), SurfaceError> {
        self.inner.load_url(url)
    }

    fn navigate_to_string(&mut self, html: &str) -> Result<(), SurfaceError> {
        self.inner.load_html(html)
    }

    fn reload(&mut self) -> Result<(), SurfaceError> {
        self.inner.reload()
    }

    fn stop(&mut self) -> Result<(), SurfaceError> {
        self.inner.stop()
    }

    fn go_back(&mut self) -> Result<(), SurfaceError> {
        self.inner.go_back()
    }

    fn go_forward(&mut self) -> Result<(), SurfaceError> {
        self.inner.go_forward()
    }

    fn can_go_back(&self) -> bool {
        self.inner.can_go_back()
    }

    fn can_go_forward(&self) -> bool {
        self.inner.can_go_forward()
    }

    fn set_cookie(&mut self, cookie: &Cookie) -> Result<(), SurfaceError> {
        self.inner.set_cookie(cookie)
    }

    fn request_cookies_for_url(&mut self, id: WebRequestId, url: &str) -> Result<(), SurfaceError> {
        self.inner.request_cookies_for_url(id, url)
    }

    fn delete_cookie(&mut self, cookie: &Cookie) -> Result<(), SurfaceError> {
        self.inner.delete_cookie(cookie)
    }

    fn request_script_result(
        &mut self,
        id: WebRequestId,
        script: &str,
    ) -> Result<(), SurfaceError> {
        self.inner.request_script_result(id, script)
    }

    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        self.inner.poll_web_event()
    }
}
