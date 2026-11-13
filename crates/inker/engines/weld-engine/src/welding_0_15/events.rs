//! welding 0.15's navigation events, permissions and capabilities in Inker's vocabulary.

use super::*;

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

pub(super) fn map_navigation_event(
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

pub(super) fn map_drag_payload_from_welding(
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

pub(super) fn map_permissions(permissions: Vec<PermissionKind>) -> Vec<PermissionDescriptor> {
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

pub(super) fn map_feature_status(status: BrowserFeatureStatus) -> WebFeatureStatus {
    match status {
        BrowserFeatureStatus::Supported => WebFeatureStatus::Supported,
        BrowserFeatureStatus::Partial(detail) => WebFeatureStatus::Partial {
            detail: detail.into(),
        },
        BrowserFeatureStatus::Unsupported(reason) => WebFeatureStatus::unsupported(reason),
    }
}
