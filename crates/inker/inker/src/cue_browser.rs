// SPDX-License-Identifier: MPL-2.0
//! Inker's platform browser bindings on the shared surface protocol. Native
//! frame conversion is supplied by the engine/host pair that owns its handles;
//! this layer never promotes borrowed handles into owned shared frames.
use crate::{
    KeyboardEvent, KeyboardModifiers, MouseButton, MouseEvent, MouseEventKind, NavigationEvent,
    PhysicalPosition, SurfaceProducer, WebSurfaceEvent,
};
use std::{collections::VecDeque, time::Duration};
use webcue::*;

/// The engine/host binding must return a non-blocking, move-owned frame with
/// this stamp. It retains the native resource and synchronization until import.
pub type FrameReader =
    Box<dyn FnMut(&mut dyn SurfaceProducer, FrameStamp) -> Result<Option<Frame>>>;
pub struct BrowserSurface {
    producer: Box<dyn SurfaceProducer>,
    protocol: Protocol,
    report: CapabilityReport,
    viewport: Viewport,
    visible: bool,
    commands: VecDeque<(RequestKey, Command)>,
    frames: FrameReader,
    revision: u64,
    turn: Option<Box<dyn FnMut(Duration) -> Result<()>>>,
}
fn backend(e: impl std::fmt::Display) -> Error {
    Error::Backend(e.to_string())
}
fn support(s: crate::WebFeatureStatus) -> Support {
    match s {
        crate::WebFeatureStatus::Supported => Support::Supported,
        crate::WebFeatureStatus::Partial { detail } => Support::Partial(detail),
        crate::WebFeatureStatus::Unsupported { reason } => Support::Unsupported(reason),
    }
}
impl BrowserSurface {
    pub fn new(
        mut producer: Box<dyn SurfaceProducer>,
        scope: Scope,
        viewport: Viewport,
        presentation: PresentationMode,
        frames: FrameReader,
    ) -> Result<Self> {
        let caps = producer
            .as_web_surface()
            .ok_or_else(|| Error::Unsupported("browser control binding required".into()))?
            .capabilities();
        let report=CapabilityReport::new(BackendIdentity{engine:caps.backend_name.clone(),version:caps.backend_version.clone(),platform:crate::cue::platform_id().into(),build_features:vec!["inker-platform-binding".into()]},
            presentation,Evidence::Constructed,SemanticProtocol::Unavailable,Feature::ALL.map(|f|(f,match f {
                Feature::Navigation|Feature::Reload|Feature::Stop|Feature::History=>support(caps.document.navigation.clone()),
                Feature::InlineHtml=>Support::Partial("inline HTML dispatch; base address remains engine-owned".into()),
                Feature::Zoom=>support(caps.document.page_zoom.clone()),
                Feature::Mouse=>support(caps.pointer.mouse.clone()),
                Feature::Scroll|Feature::Focus=>Support::Partial("dispatch through the installed platform binding; explicit refusal is preserved".into()),
                Feature::Keyboard|Feature::NativeKeyboard|Feature::Text=>Support::Partial("virtual key and composed text fields; repeat and independent native character fields are unavailable in the platform extension".into()),
                _=>Support::Unsupported("use the explicit platform extension; this shared adapter does not expose this operation".into()),
            })))?;
        Self::new_with_report(producer, scope, viewport, report, frames)
    }
    /// Bind a concrete producer with its complete, effective capability report.
    /// The report describes this installed control/frame binding, including
    /// refusals that the older platform vocabulary cannot name independently.
    pub fn new_with_report(
        mut producer: Box<dyn SurfaceProducer>,
        scope: Scope,
        viewport: Viewport,
        report: CapabilityReport,
        frames: FrameReader,
    ) -> Result<Self> {
        let viewport = viewport.validate()?;
        if report.presentation == PresentationMode::HostDevice
            || report.semantics != SemanticProtocol::Unavailable
            || report.features().any(|(feature, support)| {
                support.permits()
                    && !matches!(
                        feature,
                        Feature::Navigation
                            | Feature::Reload
                            | Feature::Stop
                            | Feature::History
                            | Feature::InlineHtml
                            | Feature::Zoom
                            | Feature::Mouse
                            | Feature::Scroll
                            | Feature::Focus
                            | Feature::Keyboard
                            | Feature::NativeKeyboard
                            | Feature::Text
                    )
            })
        {
            return Err(Error::Invalid("report advertises an operation or frame direction absent from this browser binding".into()));
        }
        if producer.as_web_surface().is_none() {
            return Err(Error::Unsupported(
                "browser control binding required".into(),
            ));
        }
        producer
            .resize(viewport.width, viewport.height)
            .map_err(backend)?;
        Ok(Self {
            producer,
            protocol: Protocol::new(scope, 256)?,
            report,
            viewport,
            visible: true,
            commands: VecDeque::new(),
            frames,
            revision: 0,
            turn: None,
        })
    }
    /// Install the concrete engine's bounded UI/process turn. It runs through
    /// `Surface::advance` even when frame acquisition is hidden or suspended.
    pub fn with_engine_turn(mut self, turn: impl FnMut(Duration) -> Result<()> + 'static) -> Self {
        self.turn = Some(Box::new(turn));
        self
    }
    /// Advanced browser/native embedding extensions retain their original owner.
    /// The host must not independently drain their event queue.
    pub fn platform(&self) -> &(dyn SurfaceProducer + 'static) {
        self.producer.as_ref()
    }
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }
    pub fn platform_mut(&mut self) -> &mut (dyn SurfaceProducer + 'static) {
        self.producer.as_mut()
    }
    fn position(&self, x: f32, y: f32) -> PhysicalPosition {
        PhysicalPosition {
            x: x * self.viewport.scale,
            y: y * self.viewport.scale,
        }
    }
    fn key(
        &mut self,
        key: u32,
        scan: u32,
        pressed: bool,
        text: Option<String>,
        modifiers: SessionModifiers,
    ) -> Result<()> {
        self.producer
            .send_keyboard_input(KeyboardEvent {
                key_code: key,
                scan_code: scan,
                pressed,
                text,
                modifiers: KeyboardModifiers {
                    shift: modifiers.shift,
                    ctrl: modifiers.control,
                    alt: modifiers.alt,
                    meta: modifiers.meta,
                },
            })
            .map_err(backend)
    }
    fn drain_callbacks(&mut self) -> Result<bool> {
        // Reserve before consuming a callback. A full mailbox leaves it with
        // its producer, preserving supplier order instead of dropping events.
        for _ in 0..64 {
            self.protocol.ensure_observations(2)?;
            let event = self
                .producer
                .as_web_surface()
                .and_then(|web| web.poll_web_event());
            let Some(event) = event else { return Ok(true) };
            match event {
                WebSurfaceEvent::Navigation(NavigationEvent::Started { url }) => self
                    .protocol
                    .emit(Event::NavigationStarted { address: url })?,
                WebSurfaceEvent::Navigation(NavigationEvent::Committed { url })
                | WebSurfaceEvent::AddressChanged { url } => {
                    self.protocol.emit(Event::AddressChanged(url))?
                },
                WebSurfaceEvent::Navigation(NavigationEvent::Finished { url, title }) => {
                    if let Some(title) = title {
                        self.protocol.emit(Event::TitleChanged(title))?;
                    }
                    self.protocol.emit(Event::NavigationFinished {
                        address: url,
                        success: true,
                    })?;
                },
                WebSurfaceEvent::Navigation(NavigationEvent::Failed { url, .. }) => {
                    self.protocol.emit(Event::NavigationFinished {
                        address: url,
                        success: false,
                    })?
                },
                WebSurfaceEvent::TitleChanged { title } => {
                    self.protocol.emit(Event::TitleChanged(title))?
                },
                WebSurfaceEvent::WebMessage(message) => self.protocol.emit(Event::Message(
                    serde_json::to_string(&message).map_err(backend)?,
                ))?,
                other => self.protocol.emit(Event::Message(
                    serde_json::to_string(&other).map_err(backend)?,
                ))?,
            }
        }
        Ok(false)
    }
    fn execute(&mut self, command: Command) -> Result<Reply> {
        if let Command::Zoom(factor) = command {
            return self
                .producer
                .set_page_zoom(factor)
                .map(Reply::Zoom)
                .map_err(backend);
        }
        let web = self
            .producer
            .as_web_surface()
            .ok_or_else(|| Error::Unsupported("browser control binding unavailable".into()))?;
        match command {
            Command::Navigate { address } => web.navigate_to_url(&address),
            Command::LoadHtml { html, .. } => web.navigate_to_string(&html),
            Command::Reload => web.reload(),
            Command::Stop => web.stop(),
            Command::Back => web.go_back(),
            Command::Forward => web.go_forward(),
            _ => {
                return Err(Error::Unsupported(
                    "shared operation not exposed by this binding".into(),
                ));
            },
        }
        .map_err(backend)?;
        Ok(Reply::Dispatched)
    }
}
impl Surface for BrowserSurface {
    fn scope(&self) -> Scope {
        self.protocol.scope()
    }
    fn capabilities(&self) -> &CapabilityReport {
        &self.report
    }
    fn set_viewport(&mut self, v: Viewport) -> Result<()> {
        self.protocol.ensure_live()?;
        let v = v.validate()?;
        if self.viewport != v {
            self.producer.resize(v.width, v.height).map_err(backend)?;
            self.viewport = v;
        }
        Ok(())
    }
    fn set_visible(&mut self, visible: bool) -> Result<()> {
        self.protocol.ensure_live()?;
        self.visible = visible;
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<()> {
        self.protocol.ensure_live()?;
        if !self.visible {
            return Err(Error::Invalid("surface is hidden".into()));
        }
        let feature = match &input {
            Input::Scroll { .. } => Feature::Scroll,
            Input::NativeKey(_) => Feature::NativeKeyboard,
            Input::Document(input) => match input {
                SessionInput::PointerMoved { .. } | SessionInput::PointerButton { .. } => {
                    Feature::Mouse
                },
                SessionInput::Text(_) | SessionInput::KeyboardText(_) => Feature::Text,
                SessionInput::Key { .. } => Feature::Keyboard,
                SessionInput::Ime(_) => Feature::Ime,
                SessionInput::Focus(_) | SessionInput::FocusMove(_) | SessionInput::Cancel => {
                    Feature::Focus
                },
            },
        };
        self.report.require(feature)?;
        match input {
            Input::Scroll {
                x,
                y,
                dx,
                dy,
                modifiers,
            } => self
                .producer
                .send_wheel_input(crate::SurfaceWheelEvent {
                    position: self.position(x, y),
                    delta: (dx * self.viewport.scale, dy * self.viewport.scale),
                    modifiers: KeyboardModifiers {
                        shift: modifiers.shift,
                        ctrl: modifiers.control,
                        alt: modifiers.alt,
                        meta: modifiers.meta,
                    },
                })
                .map_err(backend),
            Input::NativeKey(k) => {
                if k.repeat || k.characters_ignoring_modifiers != k.characters {
                    return Err(Error::Unsupported("native repeat/alternate character fields are not represented by the platform extension".into()));
                }
                self.key(
                    k.key_code,
                    k.scan_code,
                    k.pressed,
                    Some(k.characters),
                    k.modifiers,
                )
            },
            Input::Document(input) => match input {
                SessionInput::PointerMoved { x, y, modifiers }
                    if modifiers == SessionModifiers::default() =>
                {
                    self.producer
                        .send_mouse_input(MouseEvent {
                            position: self.position(x, y),
                            button: None,
                            kind: MouseEventKind::Moved,
                        })
                        .map_err(backend)
                },
                SessionInput::PointerButton {
                    x,
                    y,
                    button,
                    state,
                    modifiers,
                } if modifiers == SessionModifiers::default() => self
                    .producer
                    .send_mouse_input(MouseEvent {
                        position: self.position(x, y),
                        button: Some(match button {
                            SessionPointerButton::Primary => MouseButton::Left,
                            SessionPointerButton::Secondary => MouseButton::Right,
                            SessionPointerButton::Auxiliary => MouseButton::Middle,
                        }),
                        kind: if state == SessionButtonState::Pressed {
                            MouseEventKind::Pressed
                        } else {
                            MouseEventKind::Released
                        },
                    })
                    .map_err(backend),
                SessionInput::Focus(focused) => self.producer.set_focus(focused).map_err(backend),
                SessionInput::Text(text) | SessionInput::KeyboardText(text) => {
                    self.key(0, 0, true, Some(text), SessionModifiers::default())
                },
                SessionInput::Key {
                    key,
                    state,
                    modifiers,
                    repeat: false,
                } => {
                    let code = match key {
                        SessionKey::Enter => 13,
                        SessionKey::Tab => 9,
                        SessionKey::Backspace => 8,
                        SessionKey::Delete => 46,
                        SessionKey::Escape => 27,
                        SessionKey::Space => 32,
                        SessionKey::ArrowLeft => 37,
                        SessionKey::ArrowUp => 38,
                        SessionKey::ArrowRight => 39,
                        SessionKey::ArrowDown => 40,
                        SessionKey::Home => 36,
                        SessionKey::End => 35,
                        SessionKey::PageUp => 33,
                        SessionKey::PageDown => 34,
                        _ => {
                            return Err(Error::Unsupported(
                                "use NativeKey for platform character keys".into(),
                            ));
                        },
                    };
                    self.key(
                        code,
                        0,
                        state == SessionButtonState::Pressed,
                        None,
                        modifiers,
                    )
                },
                _ => Err(Error::Unsupported(
                    "input fields are not losslessly represented by this platform binding".into(),
                )),
            },
        }
    }
    fn request(&mut self, id: RequestId, command: Command) -> Result<RequestKey> {
        self.protocol.ensure_live()?;
        self.report.require(command.feature())?;
        if let Command::LoadHtml { address, .. } = &command
            && !address.is_empty()
        {
            return Err(Error::Unsupported(
                "inline base address is not represented by this platform binding".into(),
            ));
        }
        if matches!(command,Command::Zoom(f) if !f.is_finite()||f<=0.) {
            return Err(Error::Invalid("invalid zoom".into()));
        }
        if self.commands.len() >= 128 {
            return Err(Error::Backpressure);
        }
        let key = self.protocol.accept(id)?;
        self.commands.push_back((key, command));
        Ok(key)
    }
    fn semantic_action(&mut self, _: SemanticAction) -> Result<()> {
        self.protocol.ensure_live()?;
        Err(Error::Unsupported(
            "use the native semantic platform extension".into(),
        ))
    }
    fn advance(&mut self, now: Duration) -> Result<()> {
        self.protocol.ensure_live()?;
        // Preserve callback order within a bounded batch, but do not let a
        // continuously busy page starve host navigation or stop dispatch.
        self.drain_callbacks()?;
        if let Some((key, command)) = self.commands.pop_front() {
            let result = self.execute(command);
            self.protocol.settle(key, result)?;
        }
        if let Some(turn) = &mut self.turn {
            turn(now)?;
        }
        self.drain_callbacks()?;
        self.protocol.ensure_observations(1)?;
        if let Some(cursor) = self.producer.poll_cursor_shape() {
            use crate::CursorShape::*;
            let cursor = match cursor {
                Default => "default",
                Text => "text",
                Pointer => "pointer",
                Grab => "grab",
                Grabbing => "grabbing",
                Crosshair => "crosshair",
                Move => "move",
                ResizeNs => "ns-resize",
                ResizeEw => "ew-resize",
                ResizeNesw => "nesw-resize",
                ResizeNwse => "nwse-resize",
                NotAllowed => "not-allowed",
                Hidden => "none",
            };
            self.protocol.emit(Event::CursorChanged(cursor.into()))?;
        }
        Ok(())
    }
    fn poll_event(&mut self) -> Option<Envelope> {
        self.protocol.poll()
    }
    fn retire(&mut self) -> Result<()> {
        self.commands.clear();
        self.visible = false;
        self.protocol.retire()
    }
}
impl FrameSource for BrowserSurface {
    fn try_acquire_frame(&mut self) -> Result<Option<Frame>> {
        self.protocol.ensure_live()?;
        if !self.visible {
            return Ok(None);
        }
        let revision = self.revision.checked_add(1).ok_or(Error::Exhausted)?;
        let stamp = FrameStamp {
            scope: self.scope(),
            allocation_epoch: revision,
            content_revision: revision,
        };
        let frame = (self.frames)(self.producer.as_mut(), stamp)?;
        if let Some(frame) = &frame {
            let actual = match frame {
                Frame::Native(frame)
                    if self.report.presentation == PresentationMode::NativeFrame =>
                {
                    frame.stamp
                },
                Frame::Cpu(frame) if self.report.presentation == PresentationMode::CpuRgba => {
                    frame.validate()?;
                    if frame.size.width != self.viewport.width
                        || frame.size.height != self.viewport.height
                    {
                        return Err(Error::Invalid("frame extent differs from viewport".into()));
                    }
                    frame.stamp
                },
                Frame::Overlay if self.report.presentation == PresentationMode::NativeOverlay => {
                    stamp
                },
                _ => {
                    return Err(Error::Invalid(
                        "frame binding disagrees with declared presentation".into(),
                    ));
                },
            };
            if actual.scope != stamp.scope || actual.content_revision != stamp.content_revision {
                return Err(Error::Stale);
            }
            self.revision = revision;
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
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
        fn send_keyboard_input(
            &mut self,
            _: KeyboardEvent,
        ) -> std::result::Result<(), SurfaceError> {
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
}
