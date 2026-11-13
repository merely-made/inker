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
        buttons: PointerButtons::PRIMARY | PointerButtons::SECONDARY | PointerButtons::AUXILIARY,
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
            key.modifiers.shift && key.modifiers.ctrl && key.modifiers.alt && key.modifiers.meta
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
    assert!(!log.mice[3].modifiers.left_mouse_button && !log.mice[3].modifiers.right_mouse_button);
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
