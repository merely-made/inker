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

fn map_native_frame(frame: NativeFrame) -> WeldFrame {
    let size = frame.size();
    let format = map_pixel_format(frame.pixel_format());
    let resource_epoch = frame.generation();
    WeldFrame {
        texture: NativeTextureHandle::OwnedPayload(Box::new(WeldingNativeFramePayload::new(frame))),
        sync: inker::SurfaceSyncHandle::None,
        width: size.width,
        height: size.height,
        format,
        resource_epoch,
    }
}

pub fn map_pixel_format(format: NativeFramePixelFormat) -> SurfaceTextureFormat {
    match format {
        NativeFramePixelFormat::Rgba8Unorm => SurfaceTextureFormat::Rgba8Unorm,
        NativeFramePixelFormat::Rgba8UnormSrgb => SurfaceTextureFormat::Rgba8UnormSrgb,
        NativeFramePixelFormat::Bgra8Unorm => SurfaceTextureFormat::Bgra8Unorm,
        NativeFramePixelFormat::Bgra8UnormSrgb => SurfaceTextureFormat::Bgra8UnormSrgb,
        NativeFramePixelFormat::Unsupported => {
            SurfaceTextureFormat::Other("unsupported Welding native-frame format".into())
        },
        _ => SurfaceTextureFormat::Other("unknown Welding native-frame format".into()),
    }
}

pub fn map_error(error: WeldError) -> SurfaceError {
    match error {
        WeldError::PlatformUnsupported(reason) | WeldError::FeatureRequired(reason) => {
            SurfaceError::Unsupported(reason.into())
        },
        WeldError::BrowserOp(reason) => SurfaceError::NavigationFailed(reason),
        WeldError::Import(error) => SurfaceError::FrameAcquisitionFailed(error.to_string()),
        other => SurfaceError::SpawnFailed(other.to_string()),
    }
}

pub fn map_cookie(cookie: &InkerCookie) -> WeldingCookie {
    WeldingCookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain: cookie.domain.clone(),
        path: cookie.path.clone(),
        secure: cookie.secure,
        http_only: cookie.http_only,
        same_site: cookie.same_site.map(map_same_site),
        expires: cookie.expires,
        partitioned: cookie.partitioned,
    }
}

pub fn map_cookie_from_welding(cookie: WeldingCookie) -> InkerCookie {
    InkerCookie {
        name: cookie.name,
        value: cookie.value,
        domain: cookie.domain,
        path: cookie.path,
        secure: cookie.secure,
        http_only: cookie.http_only,
        same_site: cookie.same_site.map(map_same_site_from_welding),
        expires: cookie.expires,
        partitioned: cookie.partitioned,
    }
}

fn map_same_site(value: InkerSameSite) -> WeldingSameSite {
    match value {
        InkerSameSite::Strict => WeldingSameSite::Strict,
        InkerSameSite::Lax => WeldingSameSite::Lax,
        InkerSameSite::None => WeldingSameSite::None,
    }
}

fn map_same_site_from_welding(value: WeldingSameSite) -> InkerSameSite {
    match value {
        WeldingSameSite::Strict => InkerSameSite::Strict,
        WeldingSameSite::Lax => InkerSameSite::Lax,
        WeldingSameSite::None => InkerSameSite::None,
    }
}

fn cookie_url(cookie: &InkerCookie) -> Result<String, SurfaceError> {
    let host = cookie.domain.trim_start_matches('.');
    if host.is_empty() {
        return Err(SurfaceError::InputFailed(
            "cookie domain is required".into(),
        ));
    }
    let scheme = if cookie.secure { "https" } else { "http" };
    let path = if cookie.path.starts_with('/') {
        cookie.path.as_str()
    } else {
        "/"
    };
    Ok(format!("{scheme}://{host}{path}"))
}

pub fn map_mouse(event: InkerMouseEvent) -> Result<WeldingMouseEvent, SurfaceError> {
    let button = match event.button.unwrap_or(InkerMouseButton::Left) {
        InkerMouseButton::Left => WeldingMouseButton::Left,
        InkerMouseButton::Middle => WeldingMouseButton::Middle,
        InkerMouseButton::Right => WeldingMouseButton::Right,
        InkerMouseButton::Back | InkerMouseButton::Forward => {
            return Err(SurfaceError::Unsupported(
                "Welding's mouse vocabulary has no browser-button variant".into(),
            ));
        },
    };
    let action = match event.kind {
        MouseEventKind::Moved => MouseAction::Moved,
        MouseEventKind::Pressed => MouseAction::Pressed,
        MouseEventKind::Released => MouseAction::Released,
        MouseEventKind::ScrollPixels { delta_x, delta_y } => MouseAction::WheelScrolled {
            delta_x: delta_x as i32,
            delta_y: delta_y as i32,
        },
        MouseEventKind::ScrollLines { delta_x, delta_y } => MouseAction::WheelScrolled {
            delta_x: (delta_x * 120.0) as i32,
            delta_y: (delta_y * 120.0) as i32,
        },
    };
    Ok(WeldingMouseEvent {
        x: event.position.x as i32,
        y: event.position.y as i32,
        button,
        action,
        modifiers: EventModifiers::default(),
    })
}

/// Preserve richer host mouse input without lowering through legacy MouseEvent.
pub fn map_mouse_pointer(event: PointerEvent) -> Result<WeldingMouseEvent, SurfaceError> {
    if event.pointer_type != PointerType::Mouse {
        return Err(SurfaceError::Unsupported(
            "Welding mouse input requires a mouse pointer".into(),
        ));
    }
    let button = match event.button.unwrap_or(InkerMouseButton::Left) {
        InkerMouseButton::Left => WeldingMouseButton::Left,
        InkerMouseButton::Middle => WeldingMouseButton::Middle,
        InkerMouseButton::Right => WeldingMouseButton::Right,
        InkerMouseButton::Back | InkerMouseButton::Forward => {
            return Err(SurfaceError::Unsupported(
                "Welding's mouse vocabulary has no browser-button variant".into(),
            ));
        },
    };
    let mut modifiers = map_modifiers(event.modifiers);
    modifiers.left_mouse_button = event.buttons.contains(PointerButtons::PRIMARY);
    modifiers.middle_mouse_button = event.buttons.contains(PointerButtons::AUXILIARY);
    modifiers.right_mouse_button = event.buttons.contains(PointerButtons::SECONDARY);
    Ok(WeldingMouseEvent {
        x: event.position.x.round() as i32,
        y: event.position.y.round() as i32,
        button,
        action: match event.phase {
            PointerPhase::Down => MouseAction::Pressed,
            PointerPhase::Move => MouseAction::Moved,
            PointerPhase::Up | PointerPhase::Cancel => MouseAction::Released,
        },
        modifiers,
    })
}

pub fn map_pointer(event: PointerEvent) -> Result<TouchInput, SurfaceError> {
    let device = match event.pointer_type {
        PointerType::Touch => ContactDevice::Touch,
        PointerType::Pen => ContactDevice::Pen,
        PointerType::Mouse => {
            return Err(SurfaceError::Unsupported(
                "mouse pointers must use Inker's mouse input path for Welding".into(),
            ));
        },
        PointerType::Unknown => {
            return Err(SurfaceError::Unsupported(
                "Welding cannot dispatch an unidentified pointer type".into(),
            ));
        },
    };
    Ok(TouchInput {
        id: event.pointer_id,
        device,
        x: event.position.x,
        y: event.position.y,
        radius_x: event.width / 2.0,
        radius_y: event.height / 2.0,
        rotation_angle: event.twist.unwrap_or(0.0),
        pressure: event.pressure.unwrap_or(match event.phase {
            PointerPhase::Up | PointerPhase::Cancel => 0.0,
            PointerPhase::Down | PointerPhase::Move => 1.0,
        }),
        phase: match event.phase {
            PointerPhase::Down => TouchPhase::Started,
            PointerPhase::Move => TouchPhase::Moved,
            PointerPhase::Up => TouchPhase::Ended,
            PointerPhase::Cancel => TouchPhase::Cancelled,
        },
        modifiers: map_modifiers(event.modifiers),
    })
}

pub fn map_keyboard(event: KeyboardEvent) -> KeyEvent {
    KeyEvent {
        kind: if event.pressed {
            KeyEventKind::RawKeyDown
        } else {
            KeyEventKind::KeyUp
        },
        windows_key_code: event.key_code as i32,
        native_key_code: event.scan_code as i32,
        // Composed text is dispatched separately as CHAR after raw key down.
        character: None,
        modifiers: map_modifiers(event.modifiers),
    }
}

fn map_character(event: &KeyboardEvent, character: char) -> KeyEvent {
    KeyEvent {
        kind: KeyEventKind::Char,
        // CEF CHAR uses the character code. Virtual A (65) would insert "A"
        // even when the host supplied lowercase "a".
        windows_key_code: character as i32,
        native_key_code: event.scan_code as i32,
        character: Some(character),
        modifiers: map_modifiers(event.modifiers),
    }
}

fn map_modifiers(modifiers: KeyboardModifiers) -> EventModifiers {
    EventModifiers {
        shift: modifiers.shift,
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        meta: modifiers.meta,
        ..Default::default()
    }
}

pub fn map_drag(event: InkerDragEvent) -> DragInput {
    let allowed_operations = event.data_transfer.allowed_operations;
    let mut payload = DragPayload::default();
    for item in event.data_transfer.items {
        match item {
            DataTransferItem::File {
                path, display_name, ..
            } => {
                payload.files.push(DragFile { path, display_name });
            },
            DataTransferItem::String { mime_type, data } if mime_type == "text/html" => {
                payload.fragment_html = Some(data);
            },
            DataTransferItem::String { mime_type, data } if mime_type == "text/uri-list" => {
                payload.link_url = Some(data);
            },
            DataTransferItem::String { data, .. } => payload.fragment_text = Some(data),
        }
    }
    DragInput {
        kind: match event.phase {
            DragPhase::Enter => DragEventKind::Enter,
            DragPhase::Over => DragEventKind::Over,
            DragPhase::Leave => DragEventKind::Leave,
            DragPhase::Drop => DragEventKind::Drop,
        },
        payload: matches!(event.phase, DragPhase::Enter).then_some(payload),
        x: event.position.x as i32,
        y: event.position.y as i32,
        modifiers: map_modifiers(event.modifiers),
        allowed_operations: map_drag_operations(allowed_operations),
    }
}

fn map_drag_operations(operations: DragOperationSet) -> DragOperations {
    let mut mapped = DragOperations::NONE;
    if operations.contains(DragOperationSet::COPY) {
        mapped = mapped | DragOperations::COPY;
    }
    if operations.contains(DragOperationSet::LINK) {
        mapped = mapped | DragOperations::LINK;
    }
    if operations.contains(DragOperationSet::MOVE) {
        mapped = mapped | DragOperations::MOVE;
    }
    mapped
}

pub fn map_cursor(cursor: WeldingCursorShape) -> InkerCursorShape {
    match cursor {
        WeldingCursorShape::Default => InkerCursorShape::Default,
        WeldingCursorShape::Pointer => InkerCursorShape::Pointer,
        WeldingCursorShape::Text => InkerCursorShape::Text,
        WeldingCursorShape::Crosshair => InkerCursorShape::Crosshair,
        WeldingCursorShape::Move | WeldingCursorShape::ResizeAll => InkerCursorShape::Move,
        WeldingCursorShape::NotAllowed => InkerCursorShape::NotAllowed,
        WeldingCursorShape::ResizeNs => InkerCursorShape::ResizeNs,
        WeldingCursorShape::ResizeEw => InkerCursorShape::ResizeEw,
        WeldingCursorShape::ResizeNeSw => InkerCursorShape::ResizeNesw,
        WeldingCursorShape::ResizeNwSe => InkerCursorShape::ResizeNwse,
        WeldingCursorShape::Grab => InkerCursorShape::Grab,
        WeldingCursorShape::Grabbing => InkerCursorShape::Grabbing,
        WeldingCursorShape::Custom(name) if name == "none" => InkerCursorShape::Hidden,
        _ => InkerCursorShape::Default,
    }
}

pub fn map_event(
    event: CefSurfaceEvent,
    active_find_query: Option<&DocumentFindQuery>,
) -> WebSurfaceEvent {
    match event {
        CefSurfaceEvent::WebMessage(payload) => WebSurfaceEvent::WebMessage(WebMessage {
            tag: "welding".into(),
            payload,
        }),
        CefSurfaceEvent::ScriptCompleted { id, result } => WebSurfaceEvent::ScriptCompleted {
            id: WebRequestId::new(id.get()),
            result: result.map_err(SurfaceError::SpawnFailed),
        },
        CefSurfaceEvent::CookiesCompleted { id, result } => WebSurfaceEvent::CookiesCompleted {
            id: WebRequestId::new(id.get()),
            result: result
                .map(|cookies| cookies.into_iter().map(map_cookie_from_welding).collect())
                .map_err(SurfaceError::SpawnFailed),
        },
        CefSurfaceEvent::Navigation(event) => map_navigation_event(event, active_find_query),
        _ => WebSurfaceEvent::BackendDiagnostic {
            severity: "warning".into(),
            message: "unknown Welding surface event".into(),
        },
    }
}

fn map_navigation_event(
    event: welding_0_15::NavigationEvent,
    active_find_query: Option<&DocumentFindQuery>,
) -> WebSurfaceEvent {
    use welding_0_15::NavigationEvent as Event;
    match event {
        Event::LoadStart { url } => WebSurfaceEvent::Navigation(NavigationEvent::Started { url }),
        Event::LoadEnd { url, .. } => {
            WebSurfaceEvent::Navigation(NavigationEvent::Finished { url, title: None })
        },
        Event::LoadError {
            url,
            error_code,
            error_text,
        } => WebSurfaceEvent::Navigation(NavigationEvent::Failed {
            url,
            reason: format!("CEF {error_code}: {error_text}"),
        }),
        Event::TitleChanged { title } => WebSurfaceEvent::TitleChanged { title },
        Event::AddressChanged { url } => WebSurfaceEvent::AddressChanged { url },
        Event::ContentProcessTerminated {
            status,
            error_code,
            error_string,
        } => WebSurfaceEvent::ProcessCrashed {
            reason: format!("{status:?} ({error_code}): {error_string}"),
        },
        Event::NewWindowRequested { url, .. } => WebSurfaceEvent::NewWindowRequested { url },
        Event::DragStarted {
            payload,
            allowed_operations,
            x,
            y,
        } => WebSurfaceEvent::PageDragStarted {
            data_transfer: map_drag_payload_from_welding(payload, allowed_operations),
            position: PhysicalPosition {
                x: x as f32,
                y: y as f32,
            },
        },
        Event::FindResult {
            count,
            active_match,
            final_update,
        } => match active_find_query {
            Some(query) => WebSurfaceEvent::DocumentFindChanged(DocumentFindState::engine_managed(
                query.clone(),
                usize::try_from(count.max(0)).unwrap_or(0),
                usize::try_from(active_match - 1).ok(),
                final_update,
            )),
            None => WebSurfaceEvent::BackendDiagnostic {
                severity: "warning".into(),
                message: "Welding reported a find result without an active query".into(),
            },
        },
        Event::PermissionRequested {
            id,
            origin,
            permissions,
            ..
        } => WebSurfaceEvent::PermissionRequested(PermissionRequest {
            id: UserAgentRequestId::new(id.into()),
            origin,
            descriptors: map_permissions(permissions),
        }),
        Event::AuthChallenged {
            id,
            origin_url,
            host,
            port,
            realm,
            scheme,
            is_proxy,
        } => WebSurfaceEvent::AuthenticationRequested(HttpAuthenticationChallenge {
            id: UserAgentRequestId::new(id.into()),
            protection_space: HttpProtectionSpace {
                origin_url,
                host,
                port,
                realm: (!realm.is_empty()).then_some(realm),
                scheme: scheme.to_ascii_lowercase(),
                is_proxy,
            },
        }),
        Event::ConsoleMessage {
            level,
            message,
            source,
            line,
        } => WebSurfaceEvent::ConsoleMessage {
            level: level.to_string(),
            text: message,
            source: (!source.is_empty()).then_some(source),
            line: u32::try_from(line).ok(),
        },
        Event::ContextMenuRequested {
            x,
            y,
            link_url,
            source_url,
            ..
        } => WebSurfaceEvent::ContextMenuRequested {
            x: x.into(),
            y: y.into(),
            link_url: (!link_url.is_empty()).then_some(link_url),
            image_url: (!source_url.is_empty()).then_some(source_url),
        },
        Event::DownloadStarted {
            url,
            suggested_filename,
            ..
        } => WebSurfaceEvent::DownloadRequested {
            url,
            suggested_name: (!suggested_filename.is_empty()).then_some(suggested_filename),
        },
        other => WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: format!("Welding event not represented by Inker: {other:?}"),
        },
    }
}

fn map_drag_payload_from_welding(
    payload: DragPayload,
    allowed_operations: DragOperations,
) -> inker::DataTransfer {
    let mut items = payload
        .files
        .into_iter()
        .map(|file| DataTransferItem::File {
            mime_type: "application/octet-stream".into(),
            path: file.path,
            display_name: file.display_name,
        })
        .collect::<Vec<_>>();
    for (mime_type, data) in [
        ("text/uri-list", payload.link_url),
        ("text/x-welding-link-title", payload.link_title),
        ("text/plain", payload.fragment_text),
        ("text/html", payload.fragment_html),
        (
            "text/x-welding-fragment-base-url",
            payload.fragment_base_url,
        ),
    ] {
        if let Some(data) = data {
            items.push(DataTransferItem::String {
                mime_type: mime_type.into(),
                data,
            });
        }
    }
    let mut operations = DragOperationSet::NONE;
    if allowed_operations.0 & DragOperations::COPY.0 != 0 {
        operations = operations | DragOperationSet::COPY;
    }
    if allowed_operations.0 & DragOperations::LINK.0 != 0 {
        operations = operations | DragOperationSet::LINK;
    }
    if allowed_operations.0 & DragOperations::MOVE.0 != 0 {
        operations = operations | DragOperationSet::MOVE;
    }
    inker::DataTransfer {
        items,
        allowed_operations: operations,
    }
}

fn map_permissions(permissions: Vec<PermissionKind>) -> Vec<PermissionDescriptor> {
    let mut descriptors = Vec::with_capacity(permissions.len());
    let mut desktop_audio = false;
    let mut desktop_video = false;
    for permission in permissions {
        let descriptor = match permission {
            PermissionKind::CameraStream => PermissionDescriptor::Camera,
            PermissionKind::MicStream => PermissionDescriptor::Microphone,
            PermissionKind::Geolocation => PermissionDescriptor::Geolocation,
            PermissionKind::Notifications => PermissionDescriptor::Notifications,
            PermissionKind::Clipboard => PermissionDescriptor::ClipboardRead,
            PermissionKind::MidiSysex => PermissionDescriptor::Midi { sysex: true },
            PermissionKind::PointerLock => PermissionDescriptor::PointerLock,
            PermissionKind::KeyboardLock => PermissionDescriptor::KeyboardLock,
            PermissionKind::IdleDetection => PermissionDescriptor::IdleDetection,
            PermissionKind::LocalFonts => PermissionDescriptor::LocalFonts,
            PermissionKind::StorageAccess => PermissionDescriptor::StorageAccess,
            PermissionKind::ProtectedMediaIdentifier => {
                PermissionDescriptor::ProtectedMediaIdentifier
            },
            PermissionKind::DesktopAudioCapture => {
                desktop_audio = true;
                continue;
            },
            PermissionKind::DesktopVideoCapture => {
                desktop_video = true;
                continue;
            },
            PermissionKind::Other(raw) => PermissionDescriptor::Other(format!("cef:{raw:#x}")),
            _ => PermissionDescriptor::Other(format!("welding:{permission:?}")),
        };
        descriptors.push(descriptor);
    }
    if desktop_audio || desktop_video {
        descriptors.push(PermissionDescriptor::DisplayCapture {
            audio: desktop_audio,
            video: desktop_video,
        });
    }
    descriptors
}

pub fn map_capabilities(
    caps: welding_0_15::CefSurfaceCapabilities,
    surface_mode: CefSurfaceMode,
    config: &CefSurfaceConfig,
) -> WebSurfaceCapabilities {
    let supported = |status| map_feature_status(status);
    let configured = |enabled: bool, status, disabled_reason: &'static str| {
        if enabled {
            supported(status)
        } else {
            WebFeatureStatus::unsupported(disabled_reason)
        }
    };
    let mut degradation_reasons = Vec::new();
    if !config.handle_permission_requests {
        degradation_reasons.push("permission requests are configured for automatic denial".into());
    }
    if !config.handle_auth_challenges {
        degradation_reasons
            .push("authentication challenges are configured for automatic cancellation".into());
    }
    if config.download_dir.is_none() {
        degradation_reasons
            .push("downloads are disabled because no destination directory is configured".into());
    }
    if config.devtools_protocol {
        degradation_reasons
            .push("CDP is enabled in Welding but not exposed by the Inker adapter".into());
    }
    WebSurfaceCapabilities {
        backend_name: "welding.cef".into(),
        backend_version: Some("0.15".into()),
        frame_transport: match surface_mode {
            CefSurfaceMode::AcceleratedPaint => WebFrameTransportMode::ImportedTexture,
            _ => WebFrameTransportMode::Unsupported,
        },
        cookie: CookieCapabilities {
            read: supported(caps.cookies),
            write: supported(caps.cookies),
            delete: supported(caps.cookies),
            change_events: supported(caps.cookie_change_events),
            attributes: CookieAttributeCapabilities {
                same_site: supported(caps.cookies),
                partitioned: supported(caps.cookies),
                http_only: supported(caps.cookies),
                secure: supported(caps.cookies),
                expires: supported(caps.cookies),
            },
        },
        script: ScriptCapabilities {
            execute: supported(caps.script_execution),
            result: supported(caps.script_result),
            exceptions: supported(caps.script_result),
        },
        pointer: PointerInputCapabilities {
            mouse: WebFeatureStatus::Supported,
            pen: supported(caps.touch),
            touch: supported(caps.touch),
            contact_geometry: supported(caps.touch),
            pressure: supported(caps.touch),
            tangential_pressure: WebFeatureStatus::unsupported(
                "Welding touch input has no tangential pressure",
            ),
            tilt: WebFeatureStatus::unsupported("Welding touch input has no tilt fields"),
            twist: supported(caps.touch),
            altitude_azimuth: WebFeatureStatus::unsupported(
                "Welding touch input has no altitude/azimuth fields",
            ),
        },
        document: DocumentCapabilities {
            find_in_page: WebFeatureStatus::Supported,
            page_zoom: WebFeatureStatus::Supported,
            page_capture: WebFeatureStatus::unsupported(
                "Welding snapshots are not yet mapped to Inker's correlated capture protocol",
            ),
            navigation: WebFeatureStatus::Supported,
        },
        pdf: WebFeatureStatus::unsupported(
            "the Inker Weld adapter does not expose Welding's print command",
        ),
        downloads: if config.download_dir.is_some()
            && matches!(caps.downloads, BrowserFeatureStatus::Supported)
        {
            WebFeatureStatus::Partial {
                detail: "download events are exposed; pause, resume, and cancel controls are not"
                    .into(),
            }
        } else {
            WebFeatureStatus::unsupported("downloads are disabled for this Welding surface")
        },
        devtools: WebFeatureStatus::unsupported(
            "the Inker Weld adapter does not expose Welding's CDP channel",
        ),
        popups: if matches!(caps.popups, BrowserFeatureStatus::Supported) {
            WebFeatureStatus::Partial {
                detail: "new-window requests are exposed; popup-widget textures are not".into(),
            }
        } else {
            supported(caps.popups)
        },
        permissions: configured(
            config.handle_permission_requests,
            caps.permission_requests,
            "permission requests are auto-denied for this Welding surface",
        ),
        auth: configured(
            config.handle_auth_challenges,
            caps.auth_challenges,
            "authentication challenges are auto-cancelled for this Welding surface",
        ),
        context_menus: supported(caps.context_menus),
        drag_drop: DragDropCapabilities {
            host_to_page: supported(caps.drag_drop),
            page_to_host: supported(caps.drag_drop),
            file_items: supported(caps.drag_drop),
            string_items: supported(caps.drag_drop),
        },
        ime_observability: WebFeatureStatus::unsupported(
            "the Weld adapter does not yet expose CEF composition geometry",
        ),
        accessibility: WebFeatureStatus::unsupported(
            "Welding exposes pixels rather than an accessibility tree",
        ),
        degradation_reasons,
    }
}

fn map_feature_status(status: BrowserFeatureStatus) -> WebFeatureStatus {
    match status {
        BrowserFeatureStatus::Supported => WebFeatureStatus::Supported,
        BrowserFeatureStatus::Partial(detail) => WebFeatureStatus::Partial {
            detail: detail.into(),
        },
        BrowserFeatureStatus::Unsupported(reason) => WebFeatureStatus::unsupported(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    #[derive(Default)]
    struct InputLog {
        keys: Vec<KeyEvent>,
        mice: Vec<WeldingMouseEvent>,
        touches: Vec<TouchInput>,
        fail_key_call: Option<usize>,
        events: VecDeque<CefSurfaceEvent>,
    }

    struct RecordingProducer(Rc<RefCell<InputLog>>);

    // Required non-input methods refuse use so this fixture cannot accidentally
    // simulate successful navigation, frame import, or script execution.
    macro_rules! unused_operation {
        ($($name:ident($($arg:ident: $ty:ty),*);)*) => {$ (
            fn $name(&mut self, $($arg: $ty),*) -> Result<(), WeldError> {
                $(let _ = $arg;)*
                Err(WeldError::PlatformUnsupported("not part of the input fixture"))
            }
        )*};
    }

    impl CefSurfaceProducer for RecordingProducer {
        fn open_devtools(&self) -> Result<(), WeldError> {
            Err(WeldError::PlatformUnsupported(
                "not part of the input fixture",
            ))
        }
        fn browser_id(&self) -> i32 {
            1
        }
        fn surface_mode(&self) -> CefSurfaceMode {
            CefSurfaceMode::AcceleratedPaint
        }
        fn acquire_native_frame(&mut self) -> Option<NativeFrame> {
            None
        }
        fn acquire_frame(
            &mut self,
            _: &welding_0_15::HostWgpuContext,
        ) -> Result<Option<welding_0_15::ImportedTexture>, WeldError> {
            Ok(None)
        }
        unused_operation! {
            resize(size: dpi::PhysicalSize<u32>);
            navigate_to_url(url: &str);
            navigate_to_string(content: &str, mime_type: &str);
            reload(); stop(); go_back(); go_forward(); close();
            move_focus(direction: FocusDirection);
            post_web_message(message: &str);
            execute_script(script: &str, source_url: &str);
        }
        fn send_mouse_input(&mut self, event: WeldingMouseEvent) -> Result<(), WeldError> {
            self.0.borrow_mut().mice.push(event);
            Ok(())
        }
        fn send_touch_input(&mut self, event: TouchInput) -> Result<(), WeldError> {
            self.0.borrow_mut().touches.push(event);
            Ok(())
        }
        fn send_keyboard_input(&mut self, event: KeyEvent) -> Result<(), WeldError> {
            let mut log = self.0.borrow_mut();
            log.keys.push(event);
            if log.fail_key_call == Some(log.keys.len()) {
                return Err(WeldError::BrowserOp("input rejected".into()));
            }
            Ok(())
        }
        fn poll_web_event(&mut self) -> Option<CefSurfaceEvent> {
            self.0.borrow_mut().events.pop_front()
        }
    }

    fn recording_surface() -> (WeldingSurface, Rc<RefCell<InputLog>>) {
        let log = Rc::new(RefCell::new(InputLog::default()));
        let surface = WeldingSurface::new(
            Box::new(RecordingProducer(log.clone())),
            &CefSurfaceConfig::default(),
        );
        (surface, log)
    }

    fn keyboard(pressed: bool, text: Option<&str>) -> KeyboardEvent {
        KeyboardEvent {
            key_code: 65,
            scan_code: 30,
            modifiers: KeyboardModifiers {
                shift: true,
                ctrl: true,
                alt: true,
                meta: true,
            },
            pressed,
            text: text.map(str::to_owned),
        }
    }

    fn pointer() -> PointerEvent {
        PointerEvent {
            pointer_id: 7,
            pointer_type: PointerType::Mouse,
            is_primary: true,
            phase: PointerPhase::Down,
            position: PhysicalPosition { x: 12.6, y: 20.4 },
            button: Some(InkerMouseButton::Right),
            buttons: PointerButtons::PRIMARY
                | PointerButtons::SECONDARY
                | PointerButtons::AUXILIARY,
            width: 1.0,
            height: 1.0,
            pressure: None,
            tangential_pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            altitude_angle: None,
            azimuth_angle: None,
            modifiers: KeyboardModifiers {
                shift: true,
                ctrl: true,
                alt: true,
                meta: true,
            },
        }
    }

    #[test]
    fn pressed_text_is_ordered_after_raw_key_with_character_codes() {
        let (mut surface, log) = recording_surface();
        surface.notify_keyboard(keyboard(true, Some("aé"))).unwrap();
        surface
            .notify_keyboard(keyboard(false, Some("ignored")))
            .unwrap();
        surface.notify_keyboard(keyboard(true, None)).unwrap();
        let log = log.borrow();
        assert_eq!(
            log.keys.iter().map(|key| key.kind).collect::<Vec<_>>(),
            vec![
                KeyEventKind::RawKeyDown,
                KeyEventKind::Char,
                KeyEventKind::Char,
                KeyEventKind::KeyUp,
                KeyEventKind::RawKeyDown
            ]
        );
        assert_eq!(
            log.keys
                .iter()
                .map(|key| key.windows_key_code)
                .collect::<Vec<_>>(),
            vec![65, 97, 233, 65, 65]
        );
        assert_eq!(
            log.keys.iter().map(|key| key.character).collect::<Vec<_>>(),
            vec![None, Some('a'), Some('é'), None, None]
        );
        for key in &log.keys {
            assert_eq!(key.native_key_code, 30);
            assert!(
                key.modifiers.shift
                    && key.modifiers.ctrl
                    && key.modifiers.alt
                    && key.modifiers.meta
            );
        }
    }

    #[test]
    fn keyboard_dispatch_stops_at_the_first_rejected_event() {
        for rejected_call in [1, 2] {
            let (mut surface, log) = recording_surface();
            log.borrow_mut().fail_key_call = Some(rejected_call);
            assert!(
                matches!(surface.notify_keyboard(keyboard(true, Some("ab"))), Err(SurfaceError::NavigationFailed(reason)) if reason == "input rejected")
            );
            assert_eq!(log.borrow().keys.len(), rejected_call);
        }
    }

    #[test]
    fn rich_mouse_dispatch_keeps_changed_and_held_buttons_through_release() {
        let (mut surface, log) = recording_surface();
        let mut event = pointer();
        surface.notify_pointer(event.clone()).unwrap();
        event.phase = PointerPhase::Move;
        event.button = None;
        surface.notify_pointer(event.clone()).unwrap();
        event.phase = PointerPhase::Up;
        event.button = Some(InkerMouseButton::Right);
        event.buttons = PointerButtons::PRIMARY;
        surface.notify_pointer(event.clone()).unwrap();
        event.phase = PointerPhase::Cancel;
        event.buttons = PointerButtons::NONE;
        surface.notify_pointer(event).unwrap();
        let log = log.borrow();
        assert!(log.touches.is_empty());
        assert_eq!(
            log.mice
                .iter()
                .map(|mouse| mouse.action)
                .collect::<Vec<_>>(),
            vec![
                MouseAction::Pressed,
                MouseAction::Moved,
                MouseAction::Released,
                MouseAction::Released
            ]
        );
        assert_eq!(log.mice[0].button, WeldingMouseButton::Right);
        assert_eq!(log.mice[2].button, WeldingMouseButton::Right);
        for mouse in &log.mice {
            assert_eq!((mouse.x, mouse.y), (13, 20));
            assert!(
                mouse.modifiers.shift
                    && mouse.modifiers.ctrl
                    && mouse.modifiers.alt
                    && mouse.modifiers.meta
            );
        }
        for mouse in &log.mice[..2] {
            assert!(
                mouse.modifiers.left_mouse_button
                    && mouse.modifiers.middle_mouse_button
                    && mouse.modifiers.right_mouse_button
            );
        }
        assert!(log.mice[2].modifiers.left_mouse_button);
        assert!(
            !log.mice[2].modifiers.right_mouse_button && !log.mice[2].modifiers.middle_mouse_button
        );
        assert!(
            !log.mice[3].modifiers.left_mouse_button && !log.mice[3].modifiers.right_mouse_button
        );
    }

    #[test]
    fn contact_dispatch_and_typed_mouse_refusals_are_preserved() {
        let (mut surface, log) = recording_surface();
        for device in [PointerType::Touch, PointerType::Pen] {
            let mut event = pointer();
            event.pointer_type = device;
            surface.notify_pointer(event).unwrap();
        }
        for button in [InkerMouseButton::Back, InkerMouseButton::Forward] {
            let mut event = pointer();
            event.button = Some(button);
            assert!(matches!(
                surface.notify_pointer(event),
                Err(SurfaceError::Unsupported(_))
            ));
        }
        let mut unknown = pointer();
        unknown.pointer_type = PointerType::Unknown;
        assert!(matches!(
            surface.notify_pointer(unknown),
            Err(SurfaceError::Unsupported(_))
        ));
        assert!(log.borrow().mice.is_empty());
        assert_eq!(log.borrow().touches.len(), 2);
        assert!(matches!(
            log.borrow().touches[0].device,
            ContactDevice::Touch
        ));
        assert!(matches!(log.borrow().touches[1].device, ContactDevice::Pen));
    }

    #[test]
    fn input_keeps_the_same_ordered_completion_queue() {
        let (mut surface, log) = recording_surface();
        log.borrow_mut().events.extend([
            CefSurfaceEvent::ScriptCompleted {
                id: welding_0_15::WebRequestId::new(9),
                result: Ok("42".into()),
            },
            CefSurfaceEvent::CookiesCompleted {
                id: welding_0_15::WebRequestId::new(3),
                result: Ok(vec![]),
            },
        ]);
        surface.notify_pointer(pointer()).unwrap();
        surface.notify_keyboard(keyboard(true, Some("a"))).unwrap();
        assert!(
            matches!(surface.poll_web_event(), Some(WebSurfaceEvent::ScriptCompleted { id, .. }) if id.get() == 9)
        );
        assert!(
            matches!(surface.poll_web_event(), Some(WebSurfaceEvent::CookiesCompleted { id, .. }) if id.get() == 3)
        );
        assert!(surface.poll_web_event().is_none());
    }

    #[test]
    fn completion_mapping_preserves_full_width_request_ids() {
        let id = u64::from(u32::MAX) + 1;
        let event = map_event(
            CefSurfaceEvent::ScriptCompleted {
                id: welding_0_15::WebRequestId::new(id),
                result: Ok("42".into()),
            },
            None,
        );
        assert!(matches!(
            event,
            WebSurfaceEvent::ScriptCompleted {
                id: actual,
                result: Ok(ref value),
            } if actual.get() == id && value == "42"
        ));
    }

    #[test]
    fn find_results_retain_the_query_and_convert_the_one_based_ordinal() {
        let query = DocumentFindQuery {
            text: "needle".into(),
            match_case: true,
        };
        let event = map_event(
            CefSurfaceEvent::Navigation(welding_0_15::NavigationEvent::FindResult {
                count: 3,
                active_match: 2,
                final_update: true,
            }),
            Some(&query),
        );
        assert!(matches!(
            event,
            WebSurfaceEvent::DocumentFindChanged(DocumentFindState {
                query: actual_query,
                count: 3,
                current: Some(1),
                complete: true,
                ..
            }) if actual_query == query
        ));
    }

    #[test]
    fn permission_mapping_combines_desktop_capture_bits() {
        let descriptors = map_permissions(vec![
            PermissionKind::Geolocation,
            PermissionKind::DesktopAudioCapture,
            PermissionKind::DesktopVideoCapture,
        ]);
        assert!(descriptors.contains(&PermissionDescriptor::Geolocation));
        assert!(descriptors.contains(&PermissionDescriptor::DisplayCapture {
            audio: true,
            video: true,
        }));
    }
}
