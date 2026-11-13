//! Scrying's event vocabulary mapped to Inker's portable subset.

use super::*;

// ── Events ────────────────────────────────────────────────────────────

/// Lossy map scrying's rich event vocabulary to inker's portable subset.
/// Returns `None` for events that have no inker equivalent today (downloads,
/// permissions, drag, context menu, etc.); they're dropped at the boundary.
pub fn map_navigation_event(ev: ScryingNavEvent) -> Option<InkerNavEvent> {
    match ev {
        ScryingNavEvent::Starting { url } => Some(InkerNavEvent::Started { url }),
        ScryingNavEvent::SourceChanged { url } => Some(InkerNavEvent::Committed { url }),
        ScryingNavEvent::Completed { url, success } => {
            if success {
                Some(InkerNavEvent::Finished { url, title: None })
            } else {
                Some(InkerNavEvent::Failed {
                    url,
                    reason: "navigation failed".into(),
                })
            }
        },
        ScryingNavEvent::TitleChanged { title } => Some(InkerNavEvent::Finished {
            url: String::new(),
            title: Some(title),
        }),
        _ => None,
    }
}

pub fn map_web_event(ev: ScryingNavEvent) -> Option<WebSurfaceEvent> {
    match ev {
        ScryingNavEvent::Starting { url } => {
            Some(WebSurfaceEvent::Navigation(InkerNavEvent::Started { url }))
        },
        ScryingNavEvent::SourceChanged { url } => {
            Some(WebSurfaceEvent::AddressChanged { url: url.clone() }).or(Some(
                WebSurfaceEvent::Navigation(InkerNavEvent::Committed { url }),
            ))
        },
        ScryingNavEvent::Completed { url, success } => {
            let nav = if success {
                InkerNavEvent::Finished { url, title: None }
            } else {
                InkerNavEvent::Failed {
                    url,
                    reason: "navigation failed".into(),
                }
            };
            Some(WebSurfaceEvent::Navigation(nav))
        },
        ScryingNavEvent::TitleChanged { title } => Some(WebSurfaceEvent::TitleChanged { title }),
        ScryingNavEvent::NewWindowRequested { url } => {
            Some(WebSurfaceEvent::NewWindowRequested { url })
        },
        ScryingNavEvent::ContentProcessTerminated => Some(WebSurfaceEvent::ProcessCrashed {
            reason: "web content process terminated".into(),
        }),
        ScryingNavEvent::AuthChallenged { url, host, .. } => {
            Some(WebSurfaceEvent::BackendDiagnostic {
                severity: "info".into(),
                message: format!(
                    "authentication challenge handled by the system provider: {}",
                    if host.is_empty() { url } else { host }
                ),
            })
        },
        ScryingNavEvent::DownloadStarted {
            url,
            suggested_filename,
            ..
        } => Some(WebSurfaceEvent::DownloadRequested {
            url,
            suggested_name: Some(suggested_filename),
        }),
        ScryingNavEvent::DownloadProgress { .. } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: "download progressed".into(),
        }),
        ScryingNavEvent::DownloadFinished { error, .. } => {
            Some(WebSurfaceEvent::BackendDiagnostic {
                severity: if error.is_some() { "warn" } else { "info" }.into(),
                message: error.unwrap_or_else(|| "download finished".into()),
            })
        },
        ScryingNavEvent::DownloadCancelled { .. } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "warn".into(),
            message: "download cancelled".into(),
        }),
        ScryingNavEvent::DropDetected {
            x, y, primary_url, ..
        } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: format!("drop detected at {x},{y}; url={primary_url:?}"),
        }),
        ScryingNavEvent::MediaCaptureStateChanged {
            audio_active_tracks,
            video_active_tracks,
        } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: format!(
                "media capture changed: audio={audio_active_tracks}; video={video_active_tracks}"
            ),
        }),
        ScryingNavEvent::ContextMenuRequested {
            x,
            y,
            link_url,
            image_url,
            ..
        } => Some(WebSurfaceEvent::ContextMenuRequested {
            x,
            y,
            link_url,
            image_url,
        }),
        ScryingNavEvent::AcceleratorKeyPressed { .. } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: "browser accelerator key pressed".into(),
        }),
        ScryingNavEvent::TextInputFocused { .. } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: "text input focused".into(),
        }),
        ScryingNavEvent::TextInputChanged { .. } => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: "text input changed".into(),
        }),
        ScryingNavEvent::TextInputBlurred => Some(WebSurfaceEvent::BackendDiagnostic {
            severity: "info".into(),
            message: "text input blurred".into(),
        }),
        _ => None,
    }
}

pub fn map_web_surface_event(ev: ScryingWebSurfaceEvent) -> WebSurfaceEvent {
    match ev {
        ScryingWebSurfaceEvent::Navigation(event) => {
            map_web_event(event).unwrap_or_else(|| WebSurfaceEvent::BackendDiagnostic {
                severity: "warn".to_owned(),
                message: "scrying emitted a navigation event this adapter does not yet project"
                    .to_owned(),
            })
        },
        ScryingWebSurfaceEvent::WebMessage(payload) => {
            WebSurfaceEvent::WebMessage(wrap_web_message(payload))
        },
        ScryingWebSurfaceEvent::ScriptCompleted { id, result } => {
            WebSurfaceEvent::ScriptCompleted {
                id: InkerWebRequestId::new(id.get()),
                result: result.map_err(SurfaceError::SpawnFailed),
            }
        },
        ScryingWebSurfaceEvent::CookiesCompleted { id, result } => {
            WebSurfaceEvent::CookiesCompleted {
                id: InkerWebRequestId::new(id.get()),
                result: result
                    .map(|cookies| cookies.into_iter().map(map_cookie_from_scrying).collect())
                    .map_err(SurfaceError::SpawnFailed),
            }
        },
        _ => WebSurfaceEvent::BackendDiagnostic {
            severity: "warn".to_owned(),
            message: "scrying emitted an event this adapter does not yet project".to_owned(),
        },
    }
}

pub fn map_cursor_shape(shape: ScryingCursorShape) -> InkerCursorShape {
    match shape {
        ScryingCursorShape::Default => InkerCursorShape::Default,
        ScryingCursorShape::Pointer => InkerCursorShape::Pointer,
        ScryingCursorShape::Text => InkerCursorShape::Text,
        ScryingCursorShape::Crosshair => InkerCursorShape::Crosshair,
        ScryingCursorShape::Move | ScryingCursorShape::ResizeAll => InkerCursorShape::Move,
        ScryingCursorShape::NotAllowed => InkerCursorShape::NotAllowed,
        ScryingCursorShape::ResizeNs => InkerCursorShape::ResizeNs,
        ScryingCursorShape::ResizeEw => InkerCursorShape::ResizeEw,
        ScryingCursorShape::ResizeNeSw => InkerCursorShape::ResizeNesw,
        ScryingCursorShape::ResizeNwSe => InkerCursorShape::ResizeNwse,
        ScryingCursorShape::Grab => InkerCursorShape::Grab,
        ScryingCursorShape::Grabbing => InkerCursorShape::Grabbing,
        // Wait / Help / Progress / ZoomIn / ZoomOut / Custom — no inker
        // equivalent; map to Default. Hosts wanting full fidelity can
        // downcast to the concrete producer.
        _ => InkerCursorShape::Default,
    }
}

pub fn wrap_web_message(payload: String) -> WebMessage {
    WebMessage {
        tag: String::new(),
        payload,
    }
}
