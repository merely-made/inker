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
                Feature::Ime => Support::Partial("canonical retained composition; acceptance is reported per input, native candidate-window placement stays with the host".into()),
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
    pub fn retirement_len(&self) -> usize {
        self.protocol.retirement_len()
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn host_address(&mut self, address: String) {
        self.address = address;
    }
    fn invalidate(&mut self) {
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
        self.invalidate();
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
        self.invalidate();
        self.session.frame(width, height)
    }
    pub fn try_frame(&mut self, width: u32, height: u32) -> std::result::Result<F, String> {
        self.protocol.ensure_live().map_err(|e| e.to_string())?;
        if self.semantics {
            self.protocol
                .ensure_observations(2)
                .map_err(|e| e.to_string())?;
        }
        self.invalidate();
        let frame = self.session.try_frame(width, height)?;
        self.publish().map_err(|e| e.to_string())?;
        Ok(frame)
    }
    fn publish(&mut self) -> Result<()> {
        self.flush_retirement()?;
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
        self.invalidate();
        self.visible = visible;
        self.session.set_hidden(!visible);
        Ok(())
    }
    fn input(&mut self, input: Input) -> Result<()> {
        self.protocol.ensure_live()?;
        self.ensure_visible()?;
        self.protocol.ensure_observations(4)?;
        let result = dispatch_document_input(self.session.as_mut(), input)?;
        self.invalidate();
        self.flush_retirement()?;
        for event in document_input_events(result) {
            self.protocol.emit(event)?;
        }
        Ok(())
    }
    fn request(&mut self, id: RequestId, command: Command) -> Result<RequestKey> {
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
        let _ = self.flush_retirement();
        self.protocol.poll()
    }
    fn retire(&mut self) -> Result<()> {
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
