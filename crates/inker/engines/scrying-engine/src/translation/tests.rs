use super::*;
use inker::{
    CapabilityStatus, DocumentFindDirection, DocumentFindQuery, HttpAuthenticationAnswer,
    PermissionAnswer, PhysicalPosition, PointerButtons, SurfaceProducer, UserAgentRequestId,
    WebSurface,
};

fn optimistic_backend_capabilities(backend: ScryingBackend) -> ScryingCapabilities {
    let mut caps = ScryingCapabilities::probe(None);
    caps.backend = backend;
    caps.features.downloads = ScryingCapabilityStatus::Supported;
    caps.features.popups = ScryingCapabilityStatus::Supported;
    caps.features.ime = ScryingCapabilityStatus::Supported;
    caps.features.accessibility = ScryingCapabilityStatus::Supported;
    caps
}

struct OptimisticProducer;

impl scrying::WebSurfaceProducer for OptimisticProducer {
    fn capabilities(&self) -> ScryingCapabilities {
        optimistic_backend_capabilities(ScryingBackend::WebView2)
    }

    fn acquire_frame(&mut self) -> Result<WebSurfaceFrame, WebSurfaceError> {
        Ok(WebSurfaceFrame::OverlayOnly)
    }
}

#[test]
fn capabilities_reject_commands_without_adapter_forwarding() {
    let mut producer = crate::producer::ScryingProducer::new(Box::new(OptimisticProducer), None);
    let caps = producer.capabilities();
    for status in [
        caps.document.find_in_page,
        caps.document.page_zoom,
        caps.pdf,
        caps.permissions,
        caps.auth,
        caps.ime_observability,
        caps.accessibility,
    ] {
        assert!(matches!(status, CapabilityStatus::Unsupported { reason } if !reason.is_empty()));
    }

    // Exercise the public adapter, not just the projection: backend
    // support cannot bypass the unimplemented Inker command boundary.
    assert!(matches!(
        producer.document_find(
            &DocumentFindQuery::new("needle"),
            DocumentFindDirection::Next,
            false
        ),
        Err(SurfaceError::Unsupported(_))
    ));
    assert!(matches!(
        producer.clear_document_find(),
        Err(SurfaceError::Unsupported(_))
    ));
    assert!(matches!(
        producer.set_page_zoom(1.5),
        Err(SurfaceError::Unsupported(_))
    ));
    assert!(matches!(
        producer.answer_permission(UserAgentRequestId::new(1), PermissionAnswer::Grant),
        Err(SurfaceError::Unsupported(_))
    ));
    assert!(matches!(
        producer.answer_http_authentication(
            UserAgentRequestId::new(2),
            &HttpAuthenticationAnswer::Cancel
        ),
        Err(SurfaceError::Unsupported(_))
    ));
}

#[test]
fn capabilities_keep_projected_request_events_partial() {
    let caps = map_capabilities(optimistic_backend_capabilities(ScryingBackend::WebView2));
    assert!(matches!(caps.downloads, CapabilityStatus::Partial { .. }));
    assert!(matches!(caps.popups, CapabilityStatus::Partial { .. }));
    assert!(matches!(
        caps.context_menus,
        CapabilityStatus::Partial { .. }
    ));
    assert!(matches!(
        map_web_event(ScryingNavEvent::DownloadStarted {
            id: scrying::DownloadId(7),
            url: "https://example.test/file".into(),
            suggested_filename: "file.txt".into(),
            destination_path: "file.txt".into(),
            total_bytes_expected: Some(12),
        }),
        Some(WebSurfaceEvent::DownloadRequested { url, suggested_name })
            if url == "https://example.test/file" && suggested_name.as_deref() == Some("file.txt")
    ));
    assert!(matches!(
        map_web_event(ScryingNavEvent::ContextMenuRequested {
            page_url: "https://example.test/page".into(),
            x: 12.0,
            y: 34.0,
            link_url: Some("https://example.test/link".into()),
            image_url: None,
        }),
        Some(WebSurfaceEvent::ContextMenuRequested { x, y, link_url: Some(_), .. })
            if x == 12.0 && y == 34.0
    ));
    assert!(matches!(
        map_web_event(ScryingNavEvent::NewWindowRequested {
            url: "https://example.test/new".into()
        }),
        Some(WebSurfaceEvent::NewWindowRequested { .. })
    ));
}

#[test]
fn capabilities_preserve_backend_refusals_and_partial_details() {
    let mut caps = optimistic_backend_capabilities(ScryingBackend::Unknown);
    caps.features.downloads = ScryingCapabilityStatus::Unsupported(
        scrying::native_frame::UnsupportedReason::PlatformNotImplemented,
    );
    caps.features.popups = ScryingCapabilityStatus::Partial("backend popup limit");
    let caps = map_capabilities(caps);
    assert!(matches!(
        caps.downloads,
        CapabilityStatus::Unsupported { .. }
    ));
    assert!(matches!(
        caps.context_menus,
        CapabilityStatus::Unsupported { .. }
    ));
    assert!(matches!(caps.popups, CapabilityStatus::Partial { detail }
            if detail.contains("backend popup limit") && detail.contains("new-window requests")));
}

fn touch_pointer(pointer_id: i32, phase: InkerPointerPhase) -> InkerPointerEvent {
    InkerPointerEvent {
        pointer_id,
        pointer_type: InkerPointerType::Touch,
        is_primary: pointer_id == 2,
        phase,
        position: PhysicalPosition { x: 12.0, y: 34.0 },
        button: None,
        buttons: PointerButtons::PRIMARY,
        width: 8.0,
        height: 10.0,
        pressure: None,
        tangential_pressure: None,
        tilt_x: None,
        tilt_y: None,
        twist: None,
        altitude_angle: None,
        azimuth_angle: None,
        modifiers: InkerKeyboardModifiers::default(),
    }
}

#[test]
fn touch_pointer_preserves_contact_id_and_uses_pointer_events_default_pressure() {
    let first = map_pointer(touch_pointer(2, InkerPointerPhase::Down)).unwrap();
    let second = map_pointer(touch_pointer(3, InkerPointerPhase::Down)).unwrap();
    assert_eq!(first.pointer_id, 2);
    assert_eq!(second.pointer_id, 3);
    assert_eq!(first.device, ScryingPointerDevice::Touch);
    assert_eq!(first.pressure, 0.5);
}

#[test]
fn indeterminate_host_migration_keeps_its_dedicated_error() {
    let error = map_error(WebSurfaceError::HostMigrationIndeterminate(
        "controller parent could not be queried".into(),
    ));
    assert!(matches!(
        error,
        SurfaceError::HostMigrationIndeterminate(reason)
            if reason.contains("could not be queried")
    ));
}

#[test]
fn negative_pointer_id_is_rejected_instead_of_wrapping() {
    assert!(matches!(
        map_pointer(touch_pointer(-1, InkerPointerPhase::Down)),
        Err(SurfaceError::InputFailed(_))
    ));
}

#[test]
fn mouse_press_maps_to_left_button_down() {
    let ev = InkerMouseEvent {
        position: PhysicalPosition { x: 10.0, y: 20.0 },
        button: Some(InkerMouseButton::Left),
        kind: InkerMouseEventKind::Pressed,
    };
    let out = map_mouse(ev);
    assert_eq!(out.kind, ScryingMouseEventKind::LeftButtonDown);
    assert_eq!(out.point, (10, 20));
    assert!(out.virtual_keys.left_button);
}

#[test]
fn scroll_lines_converts_to_wheel_deltas() {
    let ev = InkerMouseEvent {
        position: PhysicalPosition { x: 0.0, y: 0.0 },
        button: None,
        kind: InkerMouseEventKind::ScrollLines {
            delta_x: 0.0,
            delta_y: 2.0,
        },
    };
    let out = map_mouse(ev);
    assert_eq!(out.kind, ScryingMouseEventKind::Wheel);
    assert_eq!(out.mouse_data, 240);
}

#[test]
fn keyboard_pressed_maps_to_down() {
    let ev = InkerKeyboardEvent {
        key_code: 65,
        scan_code: 0,
        modifiers: InkerKeyboardModifiers {
            shift: true,
            ..Default::default()
        },
        pressed: true,
        text: Some("A".into()),
    };
    let out = map_keyboard(ev);
    assert_eq!(out.kind, ScryingKeyEventKind::Down);
    assert_eq!(out.virtual_key_code, 65);
    assert_eq!(out.characters, "A");
    assert!(out.modifiers.shift);
}

#[test]
fn focus_tab_maps_to_next() {
    assert_eq!(
        map_focus_reason(InkerFocusReason::Tab),
        ScryingFocusReason::Next
    );
    assert_eq!(
        map_focus_reason(InkerFocusReason::ShiftTab),
        ScryingFocusReason::Previous
    );
}

#[test]
fn settings_zoom_and_devtools_pass_through() {
    let s = SurfaceSettings {
        background_color: [0, 0, 0, 255],
        zoom_factor: 1.25,
        dev_tools: true,
    };
    let out = map_settings(&s);
    assert_eq!(out.zoom_factor, Some(1.25));
    assert_eq!(out.devtools_enabled, Some(true));
}

#[test]
fn capabilities_keep_hosted_document_limits_explicit() {
    let caps = map_capabilities(ScryingCapabilities {
        backend: ScryingBackend::Unknown,
        preferred_mode: ScryingSurfaceMode::NativeChildOverlay,
        imported_texture: ScryingCapabilityStatus::Unsupported(
            scrying::native_frame::UnsupportedReason::PlatformNotImplemented,
        ),
        native_child_overlay: ScryingCapabilityStatus::Supported,
        cpu_snapshot: ScryingCapabilityStatus::Unsupported(
            scrying::native_frame::UnsupportedReason::PlatformNotImplemented,
        ),
        supported_frames: Vec::new(),
        reason: "test stub",
        features: ScryingCapabilities::probe(None).features,
    })
    .document;
    assert!(matches!(
        caps.find_in_page,
        CapabilityStatus::Unsupported { .. }
    ));
    assert!(matches!(
        caps.page_zoom,
        CapabilityStatus::Unsupported { .. }
    ));
    assert!(matches!(
        caps.page_capture,
        CapabilityStatus::Unsupported { .. }
    ));
    assert!(matches!(caps.navigation, CapabilityStatus::Partial { .. }));
}

#[test]
fn capabilities_follow_backend_async_operations() {
    let mut wk = ScryingCapabilities::probe(None);
    wk.backend = ScryingBackend::WkWebView;
    wk.features.cookies.read = ScryingCapabilityStatus::Supported;
    wk.features.script.result = ScryingCapabilityStatus::Partial("serialized result");
    let wk = map_capabilities(wk);
    assert!(matches!(
        wk.cookie.read,
        CapabilityStatus::Unsupported { .. }
    ));
    assert!(matches!(wk.script.result, CapabilityStatus::Partial { .. }));

    let mut webview2 = ScryingCapabilities::probe(None);
    webview2.backend = ScryingBackend::WebView2;
    webview2.features.cookies.read = ScryingCapabilityStatus::Supported;
    let webview2 = map_capabilities(webview2);
    assert_eq!(webview2.cookie.read, CapabilityStatus::Supported);
}

#[test]
fn nav_completed_success_maps_to_finished() {
    let out = map_navigation_event(ScryingNavEvent::Completed {
        url: "https://example.com".into(),
        success: true,
    });
    assert!(matches!(out, Some(InkerNavEvent::Finished { .. })));
}

#[test]
fn nav_completed_failure_maps_to_failed() {
    let out = map_navigation_event(ScryingNavEvent::Completed {
        url: "https://example.com".into(),
        success: false,
    });
    assert!(matches!(out, Some(InkerNavEvent::Failed { .. })));
}

#[test]
fn nav_download_event_drops() {
    // Events that have no inker equivalent return None.
    let out = map_navigation_event(ScryingNavEvent::ContentProcessTerminated);
    assert!(out.is_none());
}

#[test]
fn ordered_script_completion_preserves_request_id_and_error() {
    let event = map_web_surface_event(ScryingWebSurfaceEvent::ScriptCompleted {
        id: scrying::WebRequestId::new(41),
        result: Err("script exploded".to_owned()),
    });

    assert!(matches!(
        event,
        WebSurfaceEvent::ScriptCompleted {
            id,
            result: Err(SurfaceError::SpawnFailed(reason)),
        } if id.get() == 41 && reason == "script exploded"
    ));
}

#[test]
fn ordered_cookie_completion_preserves_request_id_and_payload() {
    let event = map_web_surface_event(ScryingWebSurfaceEvent::CookiesCompleted {
        id: scrying::WebRequestId::new(73),
        result: Ok(vec![scrying::Cookie {
            name: "session".to_owned(),
            value: "value".to_owned(),
            domain: "example.test".to_owned(),
            path: "/".to_owned(),
            expires_at: Some(42.0),
            is_secure: true,
            is_http_only: true,
            same_site: Some(ScryingSameSite::Strict),
            partitioned: false,
        }]),
    });

    assert!(matches!(
        event,
        WebSurfaceEvent::CookiesCompleted { id, result: Ok(cookies) }
            if id.get() == 73
                && cookies.len() == 1
                && cookies[0].name == "session"
                && cookies[0].same_site == Some(InkerSameSite::Strict)
    ));
}
