//! Frames, errors, cookies and input events between Inker and welding 0.15.

use super::*;

pub(super) fn map_native_frame(frame: NativeFrame) -> WeldFrame {
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

pub(super) fn map_same_site(value: InkerSameSite) -> WeldingSameSite {
    match value {
        InkerSameSite::Strict => WeldingSameSite::Strict,
        InkerSameSite::Lax => WeldingSameSite::Lax,
        InkerSameSite::None => WeldingSameSite::None,
    }
}

pub(super) fn map_same_site_from_welding(value: WeldingSameSite) -> InkerSameSite {
    match value {
        WeldingSameSite::Strict => InkerSameSite::Strict,
        WeldingSameSite::Lax => InkerSameSite::Lax,
        WeldingSameSite::None => InkerSameSite::None,
    }
}

pub(super) fn cookie_url(cookie: &InkerCookie) -> Result<String, SurfaceError> {
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

pub(super) fn map_character(event: &KeyboardEvent, character: char) -> KeyEvent {
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

pub(super) fn map_modifiers(modifiers: KeyboardModifiers) -> EventModifiers {
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

pub(super) fn map_drag_operations(operations: DragOperationSet) -> DragOperations {
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
