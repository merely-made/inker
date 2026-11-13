use super::*;
use crate::*;
struct Page {
    events: VecDeque<WebSurfaceEvent>,
}
impl SurfaceProducer for Page {
    fn resize(&mut self, _: u32, _: u32) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn set_offset(&mut self, _: i32, _: i32) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn acquire_frame(&mut self) -> std::result::Result<Option<SurfaceFrame>, SurfaceError> {
        panic!("blocking/platform acquire must not be a fallback")
    }
    fn send_mouse_input(&mut self, _: MouseEvent) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn send_pointer_input(&mut self, _: PointerEvent) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn send_keyboard_input(&mut self, _: KeyboardEvent) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn move_focus(&mut self, _: FocusReason) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
        None
    }
    fn apply_settings(&mut self, _: &SurfaceSettings) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn as_web_surface(&mut self) -> Option<&mut dyn WebSurface> {
        Some(self)
    }
}
impl WebSurface for Page {
    fn capabilities(&self) -> WebSurfaceCapabilities {
        WebSurfaceCapabilities {
            backend_name: "test.browser".into(),
            document: DocumentCapabilities {
                navigation: CapabilityStatus::Supported,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn navigate_to_url(&mut self, url: &str) -> std::result::Result<(), SurfaceError> {
        self.events
            .push_back(WebSurfaceEvent::Navigation(NavigationEvent::Started {
                url: url.into(),
            }));
        Ok(())
    }
    fn navigate_to_string(&mut self, _: &str) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn reload(&mut self) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn stop(&mut self) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn go_back(&mut self) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn go_forward(&mut self) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn can_go_back(&self) -> bool {
        false
    }
    fn can_go_forward(&self) -> bool {
        false
    }
    fn set_cookie(&mut self, _: &Cookie) -> std::result::Result<(), SurfaceError> {
        Ok(())
    }
    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        self.events.pop_front()
    }
}
#[test]
fn common_browser_controls_and_frame_binding_use_one_scope_without_blocking_fallback() {
    let scope = Scope {
        owner: 123,
        generation: 4,
    };
    let mut surface = BrowserSurface::new(
        Box::new(Page {
            events: Default::default(),
        }),
        scope,
        Viewport {
            width: 1,
            height: 1,
            scale: 1.,
        },
        PresentationMode::CpuRgba,
        Box::new(|_, stamp| {
            Ok(Some(Frame::Cpu(CpuFrame {
                stamp,
                size: FrameSize {
                    width: 1,
                    height: 1,
                },
                stride: 4,
                rgba: vec![255, 0, 0, 255],
                alpha: Alpha::Opaque,
                encoding: Encoding::Srgb,
            })))
        }),
    )
    .unwrap();
    let key = surface
        .request(
            RequestId(1),
            Command::Navigate {
                address: "https://held.test/".into(),
            },
        )
        .unwrap();
    surface.advance(Duration::ZERO).unwrap();
    assert!(
        matches!(surface.poll_event().unwrap().event,Event::Completed{request,result:Ok(Reply::Dispatched)} if request==key)
    );
    assert!(matches!(
        surface.poll_event().unwrap().event,
        Event::NavigationStarted { .. }
    ));
    let Some(Frame::Cpu(frame)) = surface.try_acquire_frame().unwrap() else {
        panic!("owned frame")
    };
    assert_eq!(frame.stamp.scope, scope);
    assert!(matches!(
        surface.input(Input::Document(SessionInput::Ime(SessionIme::Enabled))),
        Err(Error::Unsupported(_))
    ));
    surface.retire().unwrap();
    assert!(matches!(surface.try_acquire_frame(), Err(Error::Retired)));
}
#[test]
fn callback_backlog_cannot_starve_an_accepted_host_command() {
    let events = (0..1000)
        .map(|index| WebSurfaceEvent::TitleChanged {
            title: index.to_string(),
        })
        .collect();
    let mut surface = BrowserSurface::new(
        Box::new(Page { events }),
        Scope {
            owner: 124,
            generation: 1,
        },
        Viewport {
            width: 1,
            height: 1,
            scale: 1.,
        },
        PresentationMode::NativeFrame,
        Box::new(|_, _| Ok(None)),
    )
    .unwrap();
    let key = surface.request(RequestId(1), Command::Reload).unwrap();
    surface.advance(Duration::ZERO).unwrap();
    let observed = std::iter::from_fn(|| surface.poll_event()).collect::<Vec<_>>();
    assert!(observed.iter().any(|event|matches!(event.event,Event::Completed{request,result:Ok(Reply::Dispatched)} if request==key)));
    let titles = observed
        .into_iter()
        .filter_map(|event| match event.event {
            Event::TitleChanged(title) => Some(title.parse::<usize>().unwrap()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(titles, (0..128).collect::<Vec<_>>());
}
#[test]
fn concrete_reports_keep_independent_refusals_and_gate_input() {
    let report = CapabilityReport::new(
        BackendIdentity {
            engine: "concrete.browser".into(),
            version: Some("1".into()),
            platform: "test".into(),
            build_features: vec!["native".into()],
        },
        PresentationMode::NativeFrame,
        Evidence::Constructed,
        SemanticProtocol::Unavailable,
        Feature::ALL.map(|f| {
            (
                f,
                if matches!(f, Feature::Navigation | Feature::Reload) {
                    Support::Supported
                } else {
                    Support::Unsupported(
                        "concrete binding does not implement this operation".into(),
                    )
                },
            )
        }),
    )
    .unwrap();
    let mut surface = BrowserSurface::new_with_report(
        Box::new(Page {
            events: Default::default(),
        }),
        Scope {
            owner: 125,
            generation: 1,
        },
        Viewport {
            width: 1,
            height: 1,
            scale: 1.,
        },
        report,
        Box::new(|_, _| Ok(None)),
    )
    .unwrap();
    assert_eq!(
        surface.capabilities().identity.version.as_deref(),
        Some("1")
    );
    assert!(matches!(
        surface.request(RequestId(1), Command::Stop),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        surface.request(
            RequestId(1),
            Command::LoadHtml {
                address: "test:".into(),
                html: String::new()
            }
        ),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        surface.input(Input::Document(SessionInput::Text("unadvertised".into()))),
        Err(Error::Unsupported(_))
    ));
    // Refused admissions do not spend a request id or produce completions.
    let key = surface.request(RequestId(1), Command::Reload).unwrap();
    surface.advance(Duration::ZERO).unwrap();
    assert!(
        matches!(surface.poll_event().unwrap().event, Event::Completed { request, result: Ok(Reply::Dispatched) } if request == key)
    );
    assert!(surface.poll_event().is_none());
}
#[test]
fn engine_turns_continue_while_hidden_and_stop_after_retirement() {
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = count.clone();
    let mut surface = BrowserSurface::new(
        Box::new(Page {
            events: Default::default(),
        }),
        Scope {
            owner: 126,
            generation: 1,
        },
        Viewport {
            width: 1,
            height: 1,
            scale: 1.,
        },
        PresentationMode::NativeFrame,
        Box::new(|_, _| panic!("hidden advancement must not acquire a frame")),
    )
    .unwrap()
    .with_engine_turn(move |now| {
        assert_eq!(now, Duration::from_millis(7));
        observed.set(observed.get() + 1);
        Ok(())
    });
    surface.set_visible(false).unwrap();
    let key = surface.request(RequestId(1), Command::Reload).unwrap();
    surface.advance(Duration::from_millis(7)).unwrap();
    assert_eq!(count.get(), 1);
    assert!(
        matches!(surface.poll_event().unwrap().event, Event::Completed { request, result: Ok(Reply::Dispatched) } if request == key)
    );
    assert!(surface.try_acquire_frame().unwrap().is_none());
    surface.retire().unwrap();
    assert!(matches!(
        surface.advance(Duration::from_millis(7)),
        Err(Error::Retired)
    ));
    assert_eq!(count.get(), 1);
}
