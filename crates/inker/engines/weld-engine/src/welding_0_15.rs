// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.

//! Direct, version-pinned adapter for Welding 0.15.
//!
//! Runtime initialization, subprocess dispatch, profiles, producer creation,
//! and native-frame import caches remain host policy in [`crate::WeldProducerFactory`].

use inker::{
    Cookie as InkerCookie, CookieAttributeCapabilities, CookieCapabilities,
    CursorShape as InkerCursorShape, DataTransferItem, DocumentCapabilities, DocumentFindDirection,
    DocumentFindQuery, DocumentFindState, DragDropCapabilities, DragEvent as InkerDragEvent,
    DragOperationSet, DragPhase, FocusReason, HttpAuthenticationAnswer,
    HttpAuthenticationChallenge, HttpProtectionSpace, KeyboardEvent, KeyboardModifiers,
    MouseButton as InkerMouseButton, MouseEvent as InkerMouseEvent, MouseEventKind,
    NativeTextureHandle, NavigationEvent, OwnedSurfaceFrame, PermissionAnswer,
    PermissionDescriptor, PermissionRequest, PhysicalPosition, PointerButtons, PointerEvent,
    PointerInputCapabilities, PointerPhase, PointerType, SameSite as InkerSameSite,
    ScriptCapabilities, SurfaceError, SurfaceFrame, SurfaceSettings, SurfaceTextureFormat,
    UserAgentRequestId, WebFeatureStatus, WebFrameTransportMode, WebMessage, WebRequestId,
    WebSurfaceCapabilities, WebSurfaceEvent,
};
use welding_0_15::{
    BrowserFeatureStatus, CefSurfaceConfig, CefSurfaceEvent, CefSurfaceMode, CefSurfaceProducer,
    ContactDevice, Cookie as WeldingCookie, CursorShape as WeldingCursorShape, DragEventKind,
    DragFile, DragInput, DragOperations, DragPayload, EventModifiers, FocusDirection, KeyEvent,
    KeyEventKind, MouseAction, MouseButton as WeldingMouseButton, MouseEvent as WeldingMouseEvent,
    NativeFrame, NativeFramePixelFormat, PermissionKind, SameSite as WeldingSameSite, TouchInput,
    TouchPhase, WeldError,
};

use crate::{WeldFrame, WeldSurface};

/// Welding's owned native frame, retained intact until the host imports it.
pub struct WeldingNativeFramePayload {
    frame: NativeFrame,
}

impl WeldingNativeFramePayload {
    fn new(frame: NativeFrame) -> Self {
        Self { frame }
    }

    pub fn into_native_frame(self) -> NativeFrame {
        self.frame
    }
}

impl std::fmt::Debug for WeldingNativeFramePayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WeldingNativeFramePayload")
            .field("kind", &self.frame.kind())
            .finish()
    }
}

impl OwnedSurfaceFrame for WeldingNativeFramePayload {
    fn payload_kind(&self) -> &'static str {
        "welding-0.15.native-frame"
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// Recover Welding's owned native frame for the host's selected importer.
///
/// A mismatched payload returns the complete surface frame so its ownership
/// obligation remains intact.
pub fn into_welding_native_frame(frame: SurfaceFrame) -> Result<NativeFrame, SurfaceFrame> {
    let SurfaceFrame {
        texture,
        sync,
        width,
        height,
        format,
        resource_epoch,
    } = frame;
    match texture.into_owned_payload::<WeldingNativeFramePayload>() {
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

/// Thin direct adapter over a host-constructed Welding producer.
pub struct WeldingSurface {
    producer: Box<dyn CefSurfaceProducer>,
    active_find_query: Option<DocumentFindQuery>,
    configured_background_color: [u8; 4],
    capabilities: WebSurfaceCapabilities,
}

impl WeldingSurface {
    /// Bind a host-constructed producer with the exact config used to create it.
    ///
    /// The config is required because Welding reports build support while
    /// Inker reports instance capability. The host remains responsible for
    /// constructing both objects from the same policy decision.
    pub fn new(producer: Box<dyn CefSurfaceProducer>, config: &CefSurfaceConfig) -> Self {
        let capabilities =
            map_capabilities(producer.capabilities(), producer.surface_mode(), config);
        let configured_background_color = match config.background_color {
            Some([red, green, blue]) => [red, green, blue, 255],
            None => [0, 0, 0, 0],
        };
        Self {
            producer,
            active_find_query: None,
            configured_background_color,
            capabilities,
        }
    }

    pub fn into_inner(self) -> Box<dyn CefSurfaceProducer> {
        self.producer
    }
}

impl WeldSurface for WeldingSurface {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError> {
        self.producer
            .resize(dpi::PhysicalSize::new(width, height))
            .map_err(map_error)
    }

    fn acquire_frame(&mut self) -> Result<Option<WeldFrame>, SurfaceError> {
        Ok(self.producer.acquire_native_frame().map(map_native_frame))
    }

    fn load_url(&mut self, url: &str) -> Result<(), SurfaceError> {
        self.producer.navigate_to_url(url).map_err(map_error)
    }

    fn load_html(&mut self, html: &str) -> Result<(), SurfaceError> {
        self.producer
            .navigate_to_string(html, "text/html")
            .map_err(map_error)
    }

    fn reload(&mut self) -> Result<(), SurfaceError> {
        self.producer.reload().map_err(map_error)
    }

    fn stop(&mut self) -> Result<(), SurfaceError> {
        self.producer.stop().map_err(map_error)
    }

    fn go_back(&mut self) -> Result<(), SurfaceError> {
        self.producer.go_back().map_err(map_error)
    }

    fn go_forward(&mut self) -> Result<(), SurfaceError> {
        self.producer.go_forward().map_err(map_error)
    }

    fn can_go_back(&self) -> bool {
        self.producer.can_go_back()
    }

    fn can_go_forward(&self) -> bool {
        self.producer.can_go_forward()
    }

    fn document_find(
        &mut self,
        query: &DocumentFindQuery,
        direction: DocumentFindDirection,
        find_next: bool,
    ) -> Result<(), SurfaceError> {
        self.producer
            .find(
                &query.text,
                matches!(direction, DocumentFindDirection::Next),
                query.match_case,
                find_next,
            )
            .map_err(map_error)?;
        self.active_find_query = Some(query.clone());
        Ok(())
    }

    fn clear_document_find(&mut self) -> Result<(), SurfaceError> {
        self.producer.stop_finding(true).map_err(map_error)?;
        self.active_find_query = None;
        Ok(())
    }

    fn notify_mouse(&mut self, event: InkerMouseEvent) -> Result<(), SurfaceError> {
        self.producer
            .send_mouse_input(map_mouse(event)?)
            .map_err(map_error)
    }

    fn notify_wheel(&mut self, event: inker::SurfaceWheelEvent) -> Result<(), SurfaceError> {
        let modifiers = map_modifiers(event.modifiers);
        let mut mapped = map_mouse(InkerMouseEvent {
            position: event.position,
            button: None,
            kind: MouseEventKind::ScrollPixels {
                delta_x: event.delta.0,
                delta_y: event.delta.1,
            },
        })?;
        mapped.modifiers = modifiers;
        self.producer.send_mouse_input(mapped).map_err(map_error)
    }

    fn notify_pointer(&mut self, event: PointerEvent) -> Result<(), SurfaceError> {
        if event.pointer_type == PointerType::Mouse {
            return self
                .producer
                .send_mouse_input(map_mouse_pointer(event)?)
                .map_err(map_error);
        }
        self.producer
            .send_touch_input(map_pointer(event)?)
            .map_err(map_error)
    }

    fn notify_drag(&mut self, event: InkerDragEvent) -> Result<(), SurfaceError> {
        self.producer
            .send_drag_input(map_drag(event))
            .map_err(map_error)
    }

    fn finish_drag_source(
        &mut self,
        position: PhysicalPosition,
        operation: DragOperationSet,
    ) -> Result<(), SurfaceError> {
        self.producer
            .finish_drag_source(
                position.x as i32,
                position.y as i32,
                map_drag_operations(operation),
            )
            .map_err(map_error)
    }

    fn notify_keyboard(&mut self, event: KeyboardEvent) -> Result<(), SurfaceError> {
        self.producer
            .send_keyboard_input(map_keyboard(event.clone()))
            .map_err(map_error)?;
        if event.pressed {
            for character in event.text.as_deref().unwrap_or("").chars() {
                self.producer
                    .send_keyboard_input(map_character(&event, character))
                    .map_err(map_error)?;
            }
        }
        Ok(())
    }

    fn focus(&mut self, reason: FocusReason) -> Result<(), SurfaceError> {
        let direction = match reason {
            FocusReason::Tab => FocusDirection::Forward,
            FocusReason::ShiftTab => FocusDirection::Backward,
            // Welding's direction is retained for host vocabulary parity, but
            // CEF's set_focus call only consumes the focused boolean.
            FocusReason::Mouse | FocusReason::Programmatic => FocusDirection::Forward,
        };
        self.producer.move_focus(direction).map_err(map_error)
    }

    fn poll_cursor_shape(&mut self) -> Option<InkerCursorShape> {
        self.producer.poll_cursor_shape().map(map_cursor)
    }

    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        self.producer
            .poll_web_event()
            .map(|event| map_event(event, self.active_find_query.as_ref()))
    }

    fn web_capabilities(&self) -> WebSurfaceCapabilities {
        self.capabilities.clone()
    }

    fn set_cookie(&mut self, cookie: &InkerCookie) -> Result<(), SurfaceError> {
        let url = cookie_url(cookie)?;
        self.producer
            .set_cookie(&url, &map_cookie(cookie))
            .map_err(map_error)
    }

    fn request_cookies_for_url(&mut self, id: WebRequestId, url: &str) -> Result<(), SurfaceError> {
        self.producer
            .request_cookies(welding_0_15::WebRequestId::new(id.get()), Some(url))
            .map_err(map_error)
    }

    fn delete_cookie(&mut self, cookie: &InkerCookie) -> Result<(), SurfaceError> {
        let url = cookie_url(cookie)?;
        self.producer
            .delete_cookies(Some(&url), Some(&cookie.name))
            .map_err(map_error)
    }

    fn answer_permission(
        &mut self,
        id: UserAgentRequestId,
        answer: PermissionAnswer,
    ) -> Result<(), SurfaceError> {
        let id = u32::try_from(id.get())
            .map_err(|_| SurfaceError::InputFailed("Welding permission id exceeds u32".into()))?;
        match answer {
            PermissionAnswer::Grant => self.producer.grant_permission(id),
            PermissionAnswer::Deny | PermissionAnswer::Dismiss => self.producer.deny_permission(id),
        }
        .map_err(map_error)
    }

    fn answer_http_authentication(
        &mut self,
        id: UserAgentRequestId,
        answer: &HttpAuthenticationAnswer,
    ) -> Result<(), SurfaceError> {
        let id = u32::try_from(id.get())
            .map_err(|_| SurfaceError::InputFailed("Welding auth id exceeds u32".into()))?;
        match answer {
            HttpAuthenticationAnswer::Credentials(credentials) => {
                self.producer
                    .answer_auth(id, &credentials.username, &credentials.password)
            },
            HttpAuthenticationAnswer::Cancel => self.producer.cancel_auth(id),
        }
        .map_err(map_error)
    }

    fn request_script_result(
        &mut self,
        id: WebRequestId,
        script: &str,
    ) -> Result<(), SurfaceError> {
        self.producer
            .request_script_result(welding_0_15::WebRequestId::new(id.get()), script)
            .map_err(map_error)
    }

    fn apply_settings(&mut self, settings: &SurfaceSettings) -> Result<(), SurfaceError> {
        if settings.background_color != self.configured_background_color {
            return Err(SurfaceError::Unsupported(
                "Welding background color is construction-time host policy".into(),
            ));
        }
        if settings.dev_tools {
            return Err(SurfaceError::Unsupported(
                "Welding DevTools protocol subscription is construction-time host policy".into(),
            ));
        }
        let level = if settings.zoom_factor > 0.0 {
            settings.zoom_factor.ln() / 1.2_f64.ln()
        } else {
            return Err(SurfaceError::InputFailed(
                "zoom factor must be positive".into(),
            ));
        };
        self.producer.set_zoom_level(level).map_err(map_error)
    }
}

mod convert;
pub use convert::*;

mod events;
pub use events::*;

#[cfg(test)]
mod tests;
