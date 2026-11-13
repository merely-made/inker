use std::collections::VecDeque;

use super::*;
use crate::routing::{SurfaceContract, SurfaceContractMode, SurfaceTargetId};
use crate::{PageCaptureImageArtifact, PageCaptureScope, PageCaptureViewportFacts};

#[derive(Debug, PartialEq, Eq)]
struct TestOwnedFrame(u32);

impl OwnedSurfaceFrame for TestOwnedFrame {
    fn payload_kind(&self) -> &'static str {
        "test.owned-frame"
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

#[test]
fn owned_payload_is_consumed_without_raw_handle_extraction() {
    let payload = NativeTextureHandle::OwnedPayload(Box::new(TestOwnedFrame(7)));
    assert_eq!(
        payload.into_owned_payload::<TestOwnedFrame>().unwrap(),
        TestOwnedFrame(7)
    );
}

#[test]
fn owned_payload_type_mismatch_preserves_custody() {
    let payload = NativeTextureHandle::OwnedPayload(Box::new(TestOwnedFrame(7)));
    let payload = payload.into_owned_payload::<String>().unwrap_err();
    assert_eq!(
        payload.into_owned_payload::<TestOwnedFrame>().unwrap(),
        TestOwnedFrame(7)
    );
}

struct StubProducer;

impl SurfaceProducer for StubProducer {
    fn resize(&mut self, _: u32, _: u32) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn set_offset(&mut self, _: i32, _: i32) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn acquire_frame(&mut self) -> Result<Option<SurfaceFrame>, SurfaceError> {
        Ok(None)
    }
    fn send_mouse_input(&mut self, _: MouseEvent) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn send_pointer_input(&mut self, _: PointerEvent) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn send_keyboard_input(&mut self, _: KeyboardEvent) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn move_focus(&mut self, _: FocusReason) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
        None
    }
    fn apply_settings(&mut self, _: &SurfaceSettings) -> Result<(), SurfaceError> {
        Ok(())
    }
}

struct StubSurfaceEngine;

impl SurfaceEngine for StubSurfaceEngine {
    fn engine_id(&self) -> &str {
        "test.surface"
    }
    fn spawn(&self, _: &SurfaceSpawnRequest) -> Result<Box<dyn SurfaceProducer>, SurfaceError> {
        Ok(Box::new(StubProducer))
    }
}

#[test]
fn legacy_surface_accessibility_is_explicitly_unsupported() {
    let mut producer: Box<dyn SurfaceProducer> = Box::new(StubProducer);
    assert!(producer.poll_accessibility_update().is_none());
    assert!(matches!(
        producer.set_accessibility_active(true),
        Err(SurfaceError::Unsupported(_))
    ));
    assert!(matches!(
        producer.set_accessibility_active(false),
        Err(SurfaceError::Unsupported(_))
    ));
    assert!(matches!(
        producer.request_accessibility_resync(),
        Err(SurfaceError::Unsupported(_))
    ));
    let action = SurfaceAccessibilityActionRequest {
        action: accesskit::Action::Click,
        target_tree: SurfaceAccessibilityTreeId::ROOT,
        target_node: accesskit::NodeId(1),
        data: None,
    };
    assert!(matches!(
        producer.send_accessibility_action(action),
        Err(SurfaceError::Unsupported(_))
    ));
}

struct EventQueueSurface {
    events: VecDeque<WebSurfaceEvent>,
}

impl SurfaceProducer for EventQueueSurface {
    fn resize(&mut self, _: u32, _: u32) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn set_offset(&mut self, _: i32, _: i32) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn acquire_frame(&mut self) -> Result<Option<SurfaceFrame>, SurfaceError> {
        Ok(None)
    }
    fn send_mouse_input(&mut self, _: MouseEvent) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn send_pointer_input(&mut self, _: PointerEvent) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn send_keyboard_input(&mut self, _: KeyboardEvent) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn move_focus(&mut self, _: FocusReason) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
        None
    }
    fn apply_settings(&mut self, _: &SurfaceSettings) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn as_web_surface(&mut self) -> Option<&mut dyn WebSurface> {
        Some(self)
    }
}

impl WebSurface for EventQueueSurface {
    fn navigate_to_url(&mut self, _: &str) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn navigate_to_string(&mut self, _: &str) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn reload(&mut self) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn stop(&mut self) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn go_back(&mut self) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn go_forward(&mut self) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn can_go_back(&self) -> bool {
        false
    }
    fn can_go_forward(&self) -> bool {
        false
    }
    fn set_cookie(&mut self, _: &Cookie) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn request_cookies_for_url(&mut self, id: WebRequestId, _: &str) -> Result<(), SurfaceError> {
        self.events.push_back(WebSurfaceEvent::CookiesCompleted {
            id,
            result: Ok(Vec::new()),
        });
        Ok(())
    }
    fn request_script_result(&mut self, id: WebRequestId, _: &str) -> Result<(), SurfaceError> {
        self.events.push_back(WebSurfaceEvent::ScriptCompleted {
            id,
            result: Ok("42".into()),
        });
        Ok(())
    }
    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        self.events.pop_front()
    }
}

fn decision(id: &str) -> EngineRouteDecision {
    EngineRouteDecision {
        engine_id: id.to_string(),
        surface_contract: SurfaceContract {
            target: SurfaceTargetId::new("test:1"),
            mode: SurfaceContractMode::CompositedTexture,
        },
    }
}

fn stub_request() -> SurfaceSpawnRequest {
    SurfaceSpawnRequest {
        url: "https://example.com".into(),
        width: 800,
        height: 600,
        profile: EngineProfileBinding {
            user_data_dir: "/tmp/test-profile".into(),
        },
        fence_handle: None,
    }
}

#[test]
fn registry_contains_registered_engine() {
    let mut reg = SurfaceEngineRegistry::new();
    reg.register(Box::new(StubSurfaceEngine));
    assert!(reg.contains("test.surface"));
    assert!(!reg.contains("absent.engine"));
}

#[test]
fn registry_spawns_registered_engine() {
    let mut reg = SurfaceEngineRegistry::new();
    reg.register(Box::new(StubSurfaceEngine));
    // `Box<dyn SurfaceProducer>` doesn't implement Debug, so avoid .expect()
    assert!(
        reg.spawn(&decision("test.surface"), &stub_request())
            .is_ok()
    );
}

#[test]
fn registry_reports_missing_engine() {
    let reg = SurfaceEngineRegistry::new();
    let result = reg.spawn(&decision("absent.engine"), &stub_request());
    assert!(matches!(result, Err(SurfaceError::EngineNotFound(_))));
}

#[test]
fn surface_producers_default_to_unsupported_rehosting() {
    let mut producer = StubProducer;
    let result = unsafe {
        producer.rehost(NativeSurfaceHost::Win32 {
            hwnd: std::num::NonZeroIsize::new(1).unwrap(),
        })
    };
    assert!(
        matches!(result, Err(SurfaceError::Unsupported(reason)) if reason.contains("rehosting"))
    );
}

#[test]
fn mixed_web_events_are_observed_once_in_producer_order() {
    let expected = vec![
        WebSurfaceEvent::TitleChanged {
            title: "First".into(),
        },
        WebSurfaceEvent::Navigation(NavigationEvent::Committed {
            url: "https://example.com/next".into(),
        }),
        WebSurfaceEvent::WebMessage(WebMessage {
            tag: "receipt".into(),
            payload: "done".into(),
        }),
    ];
    let mut surface = EventQueueSurface {
        events: expected.clone().into(),
    };
    let mut observed = Vec::new();
    while let Some(event) = surface.poll_web_event() {
        observed.push(event);
    }
    assert_eq!(observed, expected);
}

#[test]
fn accepted_commands_complete_after_earlier_events_with_the_callers_ids() {
    let earlier = WebSurfaceEvent::Navigation(NavigationEvent::Committed {
        url: "https://example.com/ready".into(),
    });
    let script_id = WebRequestId::new(11);
    let cookie_id = WebRequestId::new(12);
    let mut surface = EventQueueSurface {
        events: VecDeque::from([earlier.clone()]),
    };

    surface.request_script_result(script_id, "6 * 7").unwrap();
    surface
        .request_cookies_for_url(cookie_id, "https://example.com")
        .unwrap();

    assert_eq!(surface.poll_web_event(), Some(earlier));
    assert!(matches!(
        surface.poll_web_event(),
        Some(WebSurfaceEvent::ScriptCompleted { id, result: Ok(value) })
            if id == script_id && value == "42"
    ));
    assert!(matches!(
        surface.poll_web_event(),
        Some(WebSurfaceEvent::CookiesCompleted { id, result: Ok(cookies) })
            if id == cookie_id && cookies.is_empty()
    ));
    assert_eq!(surface.poll_web_event(), None);
}

#[test]
fn web_surface_capture_defaults_to_unsupported() {
    let mut surface = EventQueueSurface {
        events: VecDeque::new(),
    };
    let request = PageCaptureRequest::viewport(PageCaptureRequestId::new(4));
    assert!(matches!(
        surface.request_page_capture(request),
        Err(SurfaceError::Unsupported(_))
    ));
}

#[test]
fn capture_completion_preserves_request_id_for_success_and_error() {
    let id = PageCaptureRequestId::new(7);
    let success = WebSurfaceEvent::PageCaptureCompleted {
        id,
        result: Ok(PageCaptureOutput {
            scope: PageCaptureScope::Viewport,
            image: PageCaptureImageArtifact::Png(vec![137, 80, 78, 71]),
            viewport: PageCaptureViewportFacts::unknown_css(2, 2),
            applied_page_scale: None,
        }),
    };
    let failure = WebSurfaceEvent::PageCaptureCompleted {
        id,
        result: Err(SurfaceError::Busy {
            operation: "page capture".into(),
        }),
    };

    assert!(matches!(
        success,
        WebSurfaceEvent::PageCaptureCompleted {
            id: actual,
            result: Ok(PageCaptureOutput {
                scope: PageCaptureScope::Viewport,
                ..
            })
        } if actual == id
    ));
    assert!(
        matches!(failure, WebSurfaceEvent::PageCaptureCompleted { id: actual, result: Err(SurfaceError::Busy { .. }) } if actual == id)
    );
}
