// SPDX-License-Identifier: MPL-2.0
//! Retained sessions on the shared Webcue control plane. The host still owns
//! acquisition, history, routing and document-specific resource delivery.
use crate::{DocumentSession, SessionInput, SessionInputResult};
use std::{collections::VecDeque, time::Duration};
use webcue::{document::*, *};

/// One installed retained session. Replacing a session creates a new scope;
/// resizing it does not. A single host owns driving and draining this object.
pub struct SessionSurface<F> {
    session: Box<dyn DocumentSession<F>>,
    protocol: Protocol,
    report: CapabilityReport,
    viewport: Viewport,
    address: String,
    commands: VecDeque<(RequestKey, Command)>,
    publications: PublicationTracker,
    semantics: bool,
    visible: bool,
    revision: u64,
    retired_publication: Option<PublicationKey>,
    text_input_ready: bool,
    shared_stream_claimed: bool,
}
fn backend(e: impl std::fmt::Display) -> Error {
    Error::Backend(e.to_string())
}
fn support(value: CapabilityStatus) -> Support {
    match value {
        CapabilityStatus::Supported => Support::Supported,
        CapabilityStatus::Partial { detail } => Support::Partial(detail),
        CapabilityStatus::Unsupported { reason } => Support::Unsupported(reason),
    }
}
/// Some targets (notably wasm32-unknown-unknown) have no OS name. Webcue
/// still requires an honest, nonempty platform identity for those adapters.
pub(crate) fn platform_id() -> &'static str {
    platform_id_for(std::env::consts::OS, std::env::consts::ARCH)
}
fn platform_id_for<'a>(os: &'a str, arch: &'a str) -> &'a str {
    if os.is_empty() { arch } else { os }
}
#[cfg(test)]
#[test]
fn platform_identity_covers_targets_without_an_os_name() {
    assert_eq!(platform_id_for("", "wasm32"), "wasm32");
    assert_eq!(platform_id_for("macos", "aarch64"), "macos");
}
impl<F: 'static> SessionSurface<F> {
    pub fn new(
        session: Box<dyn DocumentSession<F>>,
        scope: Scope,
        engine: String,
        address: String,
        viewport: Viewport,
        semantic: bool,
    ) -> Result<Self> {
        let viewport = viewport.validate()?;
        let caps = session.document_capabilities();
        let report = CapabilityReport::new(
            BackendIdentity { engine, version: None, platform: platform_id().into(),
                build_features: vec!["retained-session".into()] },
            PresentationMode::HostDevice, Evidence::Constructed,
            if semantic { SemanticProtocol::DocumentSnapshot } else { SemanticProtocol::Unavailable },
            Feature::ALL.map(|f| (f, match f {
                Feature::Navigation | Feature::Reload => Support::Partial("host receives an acquisition request; routing and history stay with the host".into()),
                Feature::Find => match support(caps.find_in_page.clone()) {
                    Support::Unsupported(reason) => Support::Unsupported(reason),
                    _ => Support::Partial("new forward queries; stepping uses the document extension".into()),
                },
                Feature::Zoom => support(caps.page_zoom.clone()),
                Feature::Mouse | Feature::Keyboard | Feature::Text | Feature::Focus | Feature::Scroll =>
                    Support::Partial("canonical retained dispatch; each input result reports whether this session handled it".into()),
                Feature::Ime => Support::Partial("canonical retained composition; logical caret geometry when supplied by the session; acceptance is per input and the host applies window placement and device scale".into()),
                Feature::Semantics | Feature::SemanticActions if semantic => Support::Partial("current retained document projection and advertised actions; host owns native AT publication".into()),
                _ => Support::Unsupported("not exposed by this retained-session adapter".into()),
            })),
        )?;
        Ok(Self {
            session,
            protocol: Protocol::new(scope, 256)?,
            report,
            viewport,
            address,
            commands: VecDeque::new(),
            publications: PublicationTracker::new(scope),
            semantics: false,
            visible: true,
            revision: 0,
            retired_publication: None,
            text_input_ready: false,
            shared_stream_claimed: false,
        })
    }
    /// Call only after the receiving renderer has qualified a produced frame.
    pub fn observe_frame(&mut self) -> Result<()> {
        self.protocol.ensure_live()?;
        if self.report.evidence != Evidence::Observed {
            self.protocol.ensure_observations(1)?;
            self.report.evidence = Evidence::Observed;
            self.protocol
                .emit(Event::CapabilitiesChanged(self.report.clone()))?;
        }
        Ok(())
    }
    /// A frame revision belongs to the retained receiver, surviving renderer
    /// reconstruction on another host device. A new scope starts a new counter.
    pub fn next_frame_revision(&mut self) -> Result<u64> {
        self.protocol.ensure_live()?;
        self.revision = self.revision.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(self.revision)
    }
    /// Asynchronous controls or event polling claim the shared stream. Legacy
    /// synchronous document input and renderer-only operations do not.
    pub fn shared_stream_claimed(&self) -> bool {
        self.shared_stream_claimed
    }
    pub fn retirement_len(&self) -> usize {
        self.protocol.retirement_len()
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    /// The last observation published on this scope's shared event stream.
    pub fn text_input_state(&self) -> Option<&TextInputState> {
        self.protocol.text_input_state()
    }
    pub fn host_address(&mut self, address: String) {
        self.address = address;
    }
    fn invalidate(&mut self) {
        self.text_input_ready = false;
        // Webcue reserves this clearing observation when publishing Some, so
        // geometry can be revoked even if ordinary observations are full.
        let _ = self.protocol.emit(Event::TextInput(None));
        self.invalidate_semantics();
    }
    fn invalidate_after_input(&mut self, result: &SessionInputResult) {
        // Ignored pointer motion/key release need not schedule a frame. Keep
        // an unchanged observation only if the session still confirms that
        // its completed layout and focused editor are current. With no live
        // publication, preserve any retry deferred by mailbox backpressure.
        if matches!(&result.effect, crate::SessionEffect::Ignored) && self.text_input_ready
            && (self.protocol.text_input_state().is_none()
                || self.session.text_input_state().as_ref() == self.protocol.text_input_state())
        {
            self.invalidate_semantics();
        } else {
            self.invalidate();
        }
    }
    fn invalidate_semantics(&mut self) {
        if let Some(publication) = self.publications.retire() {
            self.retired_publication = Some(publication);
        }
    }
    fn flush_retirement(&mut self) -> Result<()> {
        if let Some(publication) = self.retired_publication {
            self.protocol.emit(Event::SemanticsRetired(publication))?;
            self.retired_publication = None;
        }
        Ok(())
    }
    /// Synchronous host receiving uses the same input dispatcher as Surface.
    /// The result is consumed directly by that host, rather than queued twice.
    pub fn dispatch(&mut self, input: SessionInput) -> Result<SessionInputResult> {
        self.protocol.ensure_live()?;
        self.ensure_visible()?;
        let result = dispatch_document_input(self.session.as_mut(), Input::Document(input))?;
        self.invalidate_after_input(&result);
        Ok(result)
    }
    fn ensure_visible(&self) -> Result<()> {
        if self.visible {
            Ok(())
        } else {
            Err(Error::Invalid("surface is hidden".into()))
        }
    }
    pub fn frame(&mut self, width: u32, height: u32) -> F {
        self.invalidate_semantics();
        let frame = self.session.frame(width, height);
        self.text_input_ready = true;
        let _ = self.publish_text_input();
        frame
    }
    pub fn try_frame(&mut self, width: u32, height: u32) -> std::result::Result<F, String> {
        self.protocol.ensure_live().map_err(|e| e.to_string())?;
        if self.semantics {
            self.protocol.ensure_observations(4).map_err(|e| e.to_string())?;
        }
        self.invalidate_semantics();
        let frame = match self.session.try_frame(width, height) {
            Ok(frame) => frame,
            Err(error) => { self.invalidate(); return Err(error); }
        };
        self.text_input_ready = true;
        match self.publish() {
            // Legacy document hosts may not consume the shared observation
            // stream. Text metadata must not stall their fallible frame seam.
            Err(Error::Backpressure) if !self.semantics => {},
            Err(error) => return Err(error.to_string()),
            Ok(()) => {},
        }
        Ok(frame)
    }
    fn publish_text_input(&mut self) -> Result<()> {
        self.protocol.ensure_live()?;
        let state = (self.visible && self.text_input_ready)
            .then(|| self.session.text_input_state()).flatten();
        if let Err(error) = self.protocol.emit(Event::TextInput(state)) {
            // The infallible frame seam may reach a changed layout while the
            // mailbox is full. Revoke old geometry using its reserved slot;
            // polling can retry the new observation after the host drains.
            let _ = self.protocol.emit(Event::TextInput(None));
            return Err(error);
        }
        Ok(())
    }
    fn publish(&mut self) -> Result<()> {
        self.flush_retirement()?;
        self.publish_text_input()?;
        if !self.semantics {
            return Ok(());
        }
        if let Some(projection) = self.session.accessibility_projection() {
            self.revision = self.revision.checked_add(1).ok_or(Error::Exhausted)?;
            let publication = self.publications.publish(self.revision)?;
            self.protocol.emit(Event::Semantics {
                publication,
                payload: SemanticPayload::Snapshot(projection),
            })?;
        }
        Ok(())
    }
    fn execute(&mut self, command: Command) -> Result<Reply> {
        match command {
            Command::Navigate { address } => {
                self.protocol.emit(Event::NavigationRequested { address })?;
                Ok(Reply::Dispatched)
            },
            Command::Reload => {
                self.protocol.emit(Event::NavigationRequested {
                    address: self.address.clone(),
                })?;
                Ok(Reply::Dispatched)
            },
            Command::Find {
                text,
                backwards: false,
            } => self
                .session
                .document_find(&DocumentFindQuery::new(text))
                .map(Reply::Find)
                .map_err(backend),
            Command::Zoom(factor) => self
                .session
                .set_page_zoom(factor)
                .map(Reply::Zoom)
                .map_err(backend),
            Command::SetSemantics(active) => {
                self.invalidate();
                self.semantics = active;
                if active {
                    self.publish()?;
                }
                Ok(Reply::Done)
            },
            _ => Err(Error::Unsupported(
                "command is not exposed by this retained adapter".into(),
            )),
        }
    }
}
impl<F: 'static> Surface for SessionSurface<F> {
    fn scope(&self) -> Scope {
        self.protocol.scope()
    }
    fn capabilities(&self) -> &CapabilityReport {
        &self.report
    }
    fn set_viewport(&mut self, viewport: Viewport) -> Result<()> {
        self.protocol.ensure_live()?;
        let viewport = viewport.validate()?;
        if viewport != self.viewport {
            self.invalidate();
            self.viewport = viewport;
        }
        Ok(())
    }
    fn set_visible(&mut self, visible: bool) -> Result<()> {
        self.protocol.ensure_live()?;
        if self.visible != visible {
            self.invalidate();
            self.visible = visible;
        }
        // A newly installed session may have been spawned hidden. Forward
        // the host's visibility even when the wrapper already has that value.
        self.session.set_hidden(!visible);
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<()> {
        self.shared_stream_claimed = true;
        self.protocol.ensure_live()?;
        self.ensure_visible()?;
        self.protocol.ensure_observations(4)?;
        let result = dispatch_document_input(self.session.as_mut(), input)?;
        self.invalidate_after_input(&result);
        self.flush_retirement()?;
        for event in document_input_events(result) {
            self.protocol.emit(event)?;
        }
        Ok(())
    }
    fn request(&mut self, id: RequestId, command: Command) -> Result<RequestKey> {
        self.shared_stream_claimed = true;
        self.protocol.ensure_live()?;
        if !self.report.feature(command.feature()).permits()
            || matches!(
                command,
                Command::Find {
                    backwards: true,
                    ..
                }
            )
        {
            return Err(Error::Unsupported(
                "command is not exposed by this retained adapter".into(),
            ));
        }
        if matches!(command, Command::Zoom(f) if !f.is_finite() || f <= 0.) {
            return Err(Error::Invalid("zoom must be finite and positive".into()));
        }
        if self.commands.len() >= 128 {
            return Err(Error::Backpressure);
        }
        let key = self.protocol.accept(id)?;
        self.commands.push_back((key, command));
        Ok(key)
    }
    fn semantic_action(&mut self, action: SemanticAction) -> Result<()> {
        self.shared_stream_claimed = true;
        self.protocol.ensure_live()?;
        self.publications.validate(action.publication())?;
        let SemanticAction::Document { action, .. } = action else {
            return Err(Error::Unsupported(
                "retained document semantics required".into(),
            ));
        };
        let projection = self
            .session
            .accessibility_projection()
            .ok_or(Error::Stale)?;
        if projection.revision() != action.revision
            || !projection
                .node(action.target)
                .is_some_and(|node| node.actions.contains(&action.action))
        {
            return Err(Error::Stale);
        }
        self.protocol.ensure_observations(7)?;
        if action.action == DocumentA11yAction::Click {
            let target = self
                .session
                .accessibility_click_target(action.target)
                .filter(|target| target.revision == action.revision)
                .ok_or(Error::Stale)?;
            self.invalidate();
            self.flush_retirement()?;
            for state in [SessionButtonState::Pressed, SessionButtonState::Released] {
                let result = self.dispatch(SessionInput::PointerButton {
                    x: target.point.x,
                    y: target.point.y,
                    button: SessionPointerButton::Primary,
                    state,
                    modifiers: SessionModifiers::default(),
                })?;
                for event in document_input_events(result) {
                    self.protocol.emit(event)?;
                }
            }
            return Ok(());
        }
        self.invalidate();
        self.flush_retirement()?;
        if self.session.dispatch_accessibility_action(&action) {
            Ok(())
        } else {
            Err(Error::Unsupported("session refused semantic action".into()))
        }
    }
    fn advance(&mut self, now: Duration) -> Result<()> {
        self.protocol.ensure_live()?;
        if !self.commands.is_empty() {
            self.protocol.ensure_observations(2)?;
            let (key, command) = self.commands.pop_front().unwrap();
            self.invalidate();
            self.flush_retirement()?;
            let result = self.execute(command);
            self.protocol.settle(key, result)?;
        }
        if !self.session.settled() {
            self.invalidate();
        }
        self.session.pump(now.as_secs_f64() * 1000.);
        Ok(())
    }
    fn poll_event(&mut self) -> Option<Envelope> {
        self.shared_stream_claimed = true;
        let _ = self.flush_retirement();
        // The infallible legacy frame entry can defer a publication when the
        // mailbox is full. Polling retries without another layout or mutation.
        let _ = self.publish_text_input();
        self.protocol.poll()
    }
    fn retire(&mut self) -> Result<()> {
        self.shared_stream_claimed = true;
        self.invalidate();
        self.commands.clear();
        self.protocol.retire()
    }
}

impl<F: 'static> AsRef<dyn DocumentSession<F>> for SessionSurface<F> {
    fn as_ref(&self) -> &(dyn DocumentSession<F> + 'static) {
        self.session.as_ref()
    }
}
impl<F: 'static> AsMut<dyn DocumentSession<F>> for SessionSurface<F> {
    fn as_mut(&mut self) -> &mut (dyn DocumentSession<F> + 'static) {
        self.invalidate();
        self.session.as_mut()
    }
}


#[cfg(test)]
mod text_input_tests {
    use super::*;
    struct Editor(f64);
    impl DocumentSession<()> for Editor {
        fn frame(&mut self, width: u32, _: u32) { self.0 = f64::from(width); }
        fn scroll_by(&mut self, _: f32, _: f32) -> bool { false }
        fn scroll_for_key(&mut self, _: crate::SessionScrollKey) -> bool { false }
        fn click_at(&mut self, _: f32, _: f32) -> crate::SessionClick { crate::SessionClick::Miss }
        fn links(&self) -> Vec<crate::SessionLink> { vec![] }
        fn editable_focus(&self) -> bool { true }
        fn text_input_state(&self) -> Option<TextInputState> {
            Some(TextInputState { element_kind: "input".into(), input_type: "text".into(),
                input_mode: String::new(), autocomplete: String::new(), multiline: false,
                password: false, selection_utf16: (0,0),
                caret: LogicalRect { x: self.0, y: 0.0, width: 1.0, height: 16.0 } })
        }
        fn as_any_ref(&self) -> &dyn std::any::Any { self }
        fn as_any(&mut self) -> &mut dyn std::any::Any { self }
    }
    #[test]
    fn ignored_input_keeps_unchanged_current_geometry_without_requiring_a_frame() {
        let mut surface = SessionSurface::new(Box::new(Editor(0.0)),
            Scope { owner: 89, generation: 1 }, "fixture".into(), "cue:fixture".into(),
            Viewport { width: 100, height: 100, scale: 1.0 }, false).unwrap();
        surface.frame(100, 100);
        let state = surface.text_input_state().unwrap().clone();
        while surface.poll_event().is_some() {}
        let result = surface.dispatch(SessionInput::PointerMoved {
            x: 20.0, y: 20.0, modifiers: Default::default(),
        }).unwrap();
        assert!(matches!(result.effect, crate::SessionEffect::Ignored));
        assert_eq!(surface.text_input_state(), Some(&state));
        assert!(surface.poll_event().is_none());
    }

    #[test]
    fn legacy_fallible_frames_continue_without_an_observation_consumer() {
        let mut surface = SessionSurface::new(Box::new(Editor(0.0)),
            Scope { owner: 90, generation: 1 }, "fixture".into(), "cue:fixture".into(),
            Viewport { width: 100, height: 100, scale: 1.0 }, false).unwrap();
        let request = surface.request(RequestId(1), Command::Reload).unwrap();
        for width in 1..=300 { surface.try_frame(width, 100).unwrap(); }
        assert!(surface.text_input_state().is_none());
        while surface.poll_event().is_some() {}
        assert_eq!(surface.text_input_state().unwrap().caret.x, 300.0);
        surface.retire().unwrap();
        assert!(matches!(surface.poll_event().unwrap().event, Event::TextInput(None)));
        assert!(matches!(surface.poll_event().unwrap().event,
            Event::Completed { request: completed, result: Err(Error::Retired) } if completed == request));
        assert!(surface.try_frame(301, 100).is_err());
    }

    #[test]
    fn infallible_frame_backpressure_clears_old_geometry_and_poll_retries_current_layout() {
        let mut surface = SessionSurface::new(Box::new(Editor(0.0)),
            Scope { owner: 88, generation: 1 }, "fixture".into(), "cue:fixture".into(),
            Viewport { width: 100, height: 100, scale: 1.0 }, false).unwrap();
        // Repeated changed frames fill the mailbox without host draining.
        for width in 1..=300 { surface.frame(width, 100); }
        assert!(surface.text_input_state().is_none(), "a blocked new observation must not retain old geometry");
        // Ignored native traffic must not discard the pending publication.
        surface.dispatch(SessionInput::PointerMoved {
            x: 20.0, y: 20.0, modifiers: Default::default(),
        }).unwrap();
        let events: Vec<_> = std::iter::from_fn(|| surface.poll_event()).collect();
        assert!(events.iter().any(|e| matches!(e.event, Event::TextInput(None))));
        assert_eq!(surface.text_input_state().unwrap().caret.x, 300.0);
        assert!(events.windows(2).all(|e| e[0].sequence < e[1].sequence));
        surface.retire().unwrap();
        assert!(surface.text_input_state().is_none());
        assert!(matches!(surface.poll_event().unwrap().event, Event::TextInput(None)));
    }
}
