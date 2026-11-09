// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The concrete Servo producer: wgpu-graft's `grafting-servo` library adapted
//! onto [`GraftSurface`] / [`GraftProducerFactory`], the way `scrying-engine`
//! adapts `scrying`. Hosts write no Servo code: they build a
//! [`ServoGraftFactory`] from a [`ServoProducerConfig`] and register
//! [`GraftEngine`](crate::GraftEngine) over it.
//!
//! Requires the host to enable `grafting-servo/servo` on its own dependency
//! row (see this crate's `servo` feature).

use std::{path::PathBuf, sync::Arc};

use grafting_servo::{
    ServoCursor, ServoEvent, ServoFactory, ServoFramePayload, ServoKeyEvent, ServoModifiers,
    ServoMouseButton, ServoMouseEvent, ServoMouseKind, ServoProducerError, ServoSurface,
    ServoViewRequest,
};
use inker::{
    CursorShape, DragEvent, DragOperationSet, FocusReason, KeyboardEvent, MouseButton, MouseEvent,
    MouseEventKind, NativeTextureHandle, NavigationEvent, OwnedSurfaceFrame, PhysicalPosition,
    PointerEvent, PointerPhase, PointerType, SurfaceAccessibilityActionRequest,
    SurfaceAccessibilityTreeId, SurfaceAccessibilityUpdate, SurfaceError, SurfaceSettings,
    SurfaceSpawnRequest, SurfaceSyncHandle, SurfaceTextureFormat, WebFeatureStatus,
    WebFrameTransportMode, WebMessage, WebSurfaceCapabilities, WebSurfaceEvent,
};

use crate::{GraftFrame, GraftProducerFactory, GraftSurface};

// The host names configuration and lifecycle through this crate, and reaches
// Servo only through the library's re-export: its revision stays in wgpu-graft.
pub use grafting_servo::{
    A11yFocusPolicy, ServoProducerConfig, active_views, grafting, parse_gpu_sync, pump, servo,
    shutdown,
};

/// Payload kind of [`GraftImportedFrame`] (macOS and Windows).
#[cfg(not(target_os = "linux"))]
pub const FRAME_KIND: &str = "graft.servo.imported-frame";
/// Payload kind of [`GraftDmaBufFrame`] (Linux).
#[cfg(target_os = "linux")]
pub const FRAME_KIND: &str = "graft.servo.native-dmabuf-frame";

/// Owned custody of a texture imported and normalized on the host device by
/// the pre-present hook. The host downcasts [`OwnedSurfaceFrame::into_any`] to
/// this and validates it against its own device and queue.
#[derive(Debug)]
pub struct GraftImportedFrame {
    pub imported: grafting::ImportedTexture,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub resource_epoch: u64,
}

#[cfg(not(target_os = "linux"))]
impl OwnedSurfaceFrame for GraftImportedFrame {
    fn payload_kind(&self) -> &'static str {
        FRAME_KIND
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

/// Owned custody of a completed DMA-BUF snapshot (Linux).
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct GraftDmaBufFrame {
    pub frame: grafting::NeutralNativeFrame,
}

#[cfg(target_os = "linux")]
impl OwnedSurfaceFrame for GraftDmaBufFrame {
    fn payload_kind(&self) -> &'static str {
        FRAME_KIND
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

pub(crate) fn map_error(error: ServoProducerError) -> SurfaceError {
    match error {
        ServoProducerError::Spawn(d) | ServoProducerError::Config(d) => {
            SurfaceError::SpawnFailed(d)
        },
        ServoProducerError::Busy(operation) => SurfaceError::Busy { operation },
        ServoProducerError::Unsupported(d) => unsupported(&d),
        ServoProducerError::Frame(d) => SurfaceError::FrameAcquisitionFailed(d),
        ServoProducerError::Navigation(d) => SurfaceError::NavigationFailed(d),
        ServoProducerError::Input(d) => SurfaceError::InputFailed(d),
    }
}

fn unsupported(operation: &str) -> SurfaceError {
    SurfaceError::Unsupported(format!("Servo host does not support {operation}"))
}

fn map_cursor(cursor: ServoCursor) -> CursorShape {
    match cursor {
        ServoCursor::Default => CursorShape::Default,
        ServoCursor::Pointer => CursorShape::Pointer,
        ServoCursor::Text => CursorShape::Text,
        ServoCursor::Grab => CursorShape::Grab,
        ServoCursor::Grabbing => CursorShape::Grabbing,
        ServoCursor::Crosshair => CursorShape::Crosshair,
        ServoCursor::Move => CursorShape::Move,
        ServoCursor::ResizeNs => CursorShape::ResizeNs,
        ServoCursor::ResizeEw => CursorShape::ResizeEw,
        ServoCursor::ResizeNesw => CursorShape::ResizeNesw,
        ServoCursor::ResizeNwse => CursorShape::ResizeNwse,
        ServoCursor::NotAllowed => CursorShape::NotAllowed,
        ServoCursor::Hidden => CursorShape::Hidden,
    }
}

fn map_event(event: ServoEvent) -> WebSurfaceEvent {
    match event {
        ServoEvent::AddressChanged { url } => WebSurfaceEvent::AddressChanged { url },
        ServoEvent::TitleChanged { title } => WebSurfaceEvent::TitleChanged { title },
        ServoEvent::NavigationStarted { url } => {
            WebSurfaceEvent::Navigation(NavigationEvent::Started { url })
        },
        ServoEvent::NavigationCommitted { url } => {
            WebSurfaceEvent::Navigation(NavigationEvent::Committed { url })
        },
        ServoEvent::NavigationFinished { url, title } => {
            WebSurfaceEvent::Navigation(NavigationEvent::Finished { url, title })
        },
        ServoEvent::NavigationFailed { url, reason } => {
            WebSurfaceEvent::Navigation(NavigationEvent::Failed { url, reason })
        },
    }
}

/// Convert the library's serialized AccessKit update into the surface
/// contract's AccessKit version, unchanged.
///
/// # Errors
/// The value is not a valid update.
pub fn surface_update_from_value(
    value: serde_json::Value,
) -> Result<SurfaceAccessibilityUpdate, serde_json::Error> {
    serde_json::from_value(value)
}

/// Builds Servo-backed [`GraftSurface`]s on the library's process root.
pub struct ServoGraftFactory {
    inner: ServoFactory,
}

impl ServoGraftFactory {
    /// Construct exactly once per process on the UI thread; `wake` is called
    /// from any thread when Servo has work for the next pump.
    ///
    /// # Errors
    /// See [`ServoFactory::new`].
    pub fn new(
        config: ServoProducerConfig,
        device: wgpu::Device,
        queue: wgpu::Queue,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, SurfaceError> {
        Ok(Self {
            inner: ServoFactory::new(config, device, queue, wake).map_err(map_error)?,
        })
    }

    #[must_use]
    pub fn profile_name(&self) -> &str {
        self.inner.profile_name()
    }

    #[must_use]
    pub fn profile_dir(&self) -> &std::path::Path {
        self.inner.profile_dir()
    }

    #[must_use]
    pub fn a11y_focus_policy(&self) -> &'static str {
        self.inner.a11y_focus_policy().name()
    }

    /// The host device this factory imports onto; frames are paired to it.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        self.inner.device()
    }

    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        self.inner.queue()
    }
}

impl GraftProducerFactory for ServoGraftFactory {
    fn build(&self, request: &SurfaceSpawnRequest) -> Result<Box<dyn GraftSurface>, SurfaceError> {
        // The shell resolves Servo requests to the explicitly selected process
        // profile. Refuse accidentally forwarding a per-node binding.
        if PathBuf::from(&request.profile.user_data_dir) != self.inner.profile_dir() {
            return Err(SurfaceError::SpawnFailed(
                "Servo view requested a different process profile".into(),
            ));
        }
        if request.fence_handle.is_some() {
            return Err(SurfaceError::SpawnFailed(
                "Servo import owns synchronization; raw spawn fences are unsupported".into(),
            ));
        }
        let inner = self
            .inner
            .build(&ServoViewRequest {
                url: request.url.clone(),
                width: request.width,
                height: request.height,
            })
            .map_err(map_error)?;
        Ok(Box::new(ServoGraftSurface { inner }))
    }
}

/// [`GraftSurface`] over one library [`ServoSurface`].
pub struct ServoGraftSurface {
    inner: ServoSurface,
}

impl GraftSurface for ServoGraftSurface {
    fn set_accessibility_active(
        &mut self,
        active: bool,
    ) -> Result<Option<SurfaceAccessibilityTreeId>, SurfaceError> {
        self.inner
            .set_accessibility_active(active)
            .map(|tree| tree.map(tree_id))
            .map_err(map_error)
    }

    fn poll_accessibility_update(&mut self) -> Option<SurfaceAccessibilityUpdate> {
        let value = self.inner.poll_accessibility_update()?;
        match surface_update_from_value(value) {
            Ok(update) => Some(update),
            Err(error) => {
                // A dropped delta would leave the host applying later deltas
                // to the wrong tree. Retire the forest; resync re-admits it.
                tracing::error!(target: "graft_engine::servo::a11y",
                    %error, "Servo accessibility update did not convert");
                let _ = self.inner.set_accessibility_active(false);
                None
            },
        }
    }

    fn request_accessibility_resync(&mut self) -> Result<SurfaceAccessibilityTreeId, SurfaceError> {
        self.inner
            .request_accessibility_resync()
            .map(tree_id)
            .map_err(map_error)
    }

    fn send_accessibility_action(
        &mut self,
        _request: SurfaceAccessibilityActionRequest,
    ) -> Result<(), SurfaceError> {
        // AAC's DOM bridge implements Click, but its node producer advertises
        // no actions. Forwarding Click without that mask would invent support;
        // Focus, value/edit, selection and scrolling lack DOM handlers too.
        Err(unsupported("unadvertised accessibility actions"))
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), SurfaceError> {
        self.inner.resize(width, height);
        Ok(())
    }

    fn acquire_frame(&mut self) -> Result<Option<GraftFrame>, SurfaceError> {
        let Some(frame) = self.inner.acquire_frame().map_err(map_error)? else {
            return Ok(None);
        };
        let (width, height, resource_epoch) = (frame.width, frame.height, frame.resource_epoch);
        let payload: Box<dyn OwnedSurfaceFrame> = match frame.payload {
            #[cfg(not(target_os = "linux"))]
            ServoFramePayload::Imported(f) => Box::new(GraftImportedFrame {
                imported: f.imported,
                device: f.device,
                queue: f.queue,
                resource_epoch: f.resource_epoch,
            }),
            #[cfg(target_os = "linux")]
            ServoFramePayload::DmaBuf(frame) => Box::new(GraftDmaBufFrame { frame }),
            #[allow(unreachable_patterns)]
            _ => {
                return Err(SurfaceError::FrameAcquisitionFailed(
                    "Servo frame payload is not supported on this platform path".into(),
                ));
            },
        };
        Ok(Some(GraftFrame {
            texture: NativeTextureHandle::OwnedPayload(payload),
            sync: SurfaceSyncHandle::None,
            width,
            height,
            format: SurfaceTextureFormat::Rgba8Unorm,
            resource_epoch,
        }))
    }

    fn load_url(&mut self, url: &str) -> Result<(), SurfaceError> {
        self.inner.load_url(url).map_err(map_error)
    }
    fn load_html(&mut self, _: &str) -> Result<(), SurfaceError> {
        Err(unsupported("inline HTML loading"))
    }
    fn reload(&mut self) -> Result<(), SurfaceError> {
        self.inner.reload();
        Ok(())
    }
    fn stop(&mut self) -> Result<(), SurfaceError> {
        Err(unsupported("stop loading"))
    }
    fn go_back(&mut self) -> Result<(), SurfaceError> {
        self.inner.go_back();
        Ok(())
    }
    fn go_forward(&mut self) -> Result<(), SurfaceError> {
        self.inner.go_forward();
        Ok(())
    }
    fn can_go_back(&self) -> bool {
        self.inner.can_go_back()
    }
    fn can_go_forward(&self) -> bool {
        self.inner.can_go_forward()
    }

    fn notify_mouse(&mut self, event: MouseEvent) -> Result<(), SurfaceError> {
        let kind = match event.kind {
            MouseEventKind::Moved => ServoMouseKind::Moved,
            MouseEventKind::Pressed | MouseEventKind::Released => {
                let button = match event.button.ok_or_else(|| {
                    SurfaceError::InputFailed("Servo mouse button is absent".into())
                })? {
                    MouseButton::Left => ServoMouseButton::Left,
                    MouseButton::Middle => ServoMouseButton::Middle,
                    MouseButton::Right => ServoMouseButton::Right,
                    MouseButton::Back => ServoMouseButton::Back,
                    MouseButton::Forward => ServoMouseButton::Forward,
                };
                ServoMouseKind::Button {
                    button,
                    pressed: matches!(event.kind, MouseEventKind::Pressed),
                }
            },
            MouseEventKind::ScrollPixels { delta_x, delta_y } => ServoMouseKind::ScrollPixels {
                dx: delta_x,
                dy: delta_y,
            },
            MouseEventKind::ScrollLines { delta_x, delta_y } => ServoMouseKind::ScrollLines {
                dx: delta_x,
                dy: delta_y,
            },
        };
        self.inner.send_mouse(ServoMouseEvent {
            x: event.position.x,
            y: event.position.y,
            kind,
        });
        Ok(())
    }

    fn notify_pointer(&mut self, event: PointerEvent) -> Result<(), SurfaceError> {
        if event.pointer_type != PointerType::Mouse {
            return Err(unsupported("pen/touch pointer projection"));
        }
        let kind = match event.phase {
            PointerPhase::Down => MouseEventKind::Pressed,
            PointerPhase::Move => MouseEventKind::Moved,
            PointerPhase::Up => MouseEventKind::Released,
            // Upstream AAC exposes leave/out, but those events do not clear
            // pressed mouse buttons or dispatch pointercancel. A synthetic
            // release could activate a control, so report the actual limit.
            PointerPhase::Cancel => return Err(unsupported("mouse cancellation")),
        };
        self.notify_mouse(MouseEvent {
            position: event.position,
            button: event.button,
            kind,
        })
    }

    fn notify_drag(&mut self, _: DragEvent) -> Result<(), SurfaceError> {
        Err(unsupported("HTML drag"))
    }

    fn finish_drag_source(
        &mut self,
        _: PhysicalPosition,
        _: DragOperationSet,
    ) -> Result<(), SurfaceError> {
        Err(unsupported("HTML drag source completion"))
    }

    fn notify_keyboard(&mut self, event: KeyboardEvent) -> Result<(), SurfaceError> {
        self.inner.send_key(&ServoKeyEvent {
            key_code: event.key_code,
            modifiers: ServoModifiers {
                shift: event.modifiers.shift,
                ctrl: event.modifiers.ctrl,
                alt: event.modifiers.alt,
                meta: event.modifiers.meta,
            },
            pressed: event.pressed,
            text: event.text,
        });
        Ok(())
    }

    fn focus(&mut self, _: FocusReason) -> Result<(), SurfaceError> {
        self.inner.set_focused(true);
        Ok(())
    }
    fn set_focus(&mut self, focused: bool) -> Result<(), SurfaceError> {
        self.inner.set_focused(focused);
        Ok(())
    }

    fn poll_navigation_event(&mut self) -> Option<NavigationEvent> {
        None
    }
    fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
        self.inner.poll_cursor().map(map_cursor)
    }
    fn poll_web_message(&mut self) -> Option<WebMessage> {
        None
    }
    fn poll_web_event(&mut self) -> Option<WebSurfaceEvent> {
        self.inner.poll_event().map(map_event)
    }

    fn web_capabilities(&self) -> WebSurfaceCapabilities {
        let mut caps = WebSurfaceCapabilities {
            backend_name: "graft.servo".into(),
            frame_transport: WebFrameTransportMode::ImportedTexture,
            ..WebSurfaceCapabilities::default()
        };
        caps.document.navigation = WebFeatureStatus::Partial {
            detail: "native load/reload/history; upstream has no stop API".into(),
        };
        caps.document.page_zoom = WebFeatureStatus::Partial {
            detail: "requested native zoom; effective readback projection is not wired".into(),
        };
        caps.pointer.mouse = WebFeatureStatus::Partial {
            detail:
                "basic down/move/up/cancel; pointer modifiers and rich fields are not projected"
                    .into(),
        };
        caps.degradation_reasons.push("first Servo slice: physical key codes, rich pointer fields, scripts, cookies, permissions and browser tools remain unavailable".into());
        caps
    }

    fn set_page_zoom(&mut self, factor: f32) -> Result<inker::DocumentZoomState, SurfaceError> {
        if !factor.is_finite() || factor <= 0. {
            return Err(unsupported("non-finite or nonpositive page zoom"));
        }
        let applied = self.inner.set_page_zoom(factor);
        Ok(inker::DocumentZoomState {
            requested: factor,
            applied,
            min: 0.1,
            max: 10.,
        })
    }

    fn apply_settings(&mut self, settings: &SurfaceSettings) -> Result<(), SurfaceError> {
        if settings.dev_tools {
            return Err(unsupported("DevTools"));
        }
        if settings.background_color != SurfaceSettings::default().background_color {
            return Err(unsupported("host background override"));
        }
        self.inner.set_page_zoom(settings.zoom_factor as f32);
        Ok(())
    }
}

fn tree_id(raw: u128) -> SurfaceAccessibilityTreeId {
    SurfaceAccessibilityTreeId(accesskit::Uuid::from_u128(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Action, Node, NodeId, Rect, Role, TreeId, TreeInfo, Uuid};

    /// The library hands accessibility over as the AccessKit serde form; the
    /// surface contract's AccessKit version must read it unchanged.
    #[test]
    fn serialized_servo_updates_convert_to_the_surface_contract_unchanged() {
        let mut root = Node::new(Role::RootWebArea);
        root.set_children(vec![NodeId(2)]);
        root.set_tree_id(TreeId(Uuid::from_u64_pair(0, 110)));
        let mut child = Node::new(Role::Button);
        child.set_label("supplier node");
        child.add_action(Action::Click);
        child.set_bounds(Rect::new(1.0, 2.0, 30.0, 40.0));
        let expected = SurfaceAccessibilityUpdate {
            nodes: vec![(NodeId(1), root), (NodeId(2), child)],
            tree: Some(TreeInfo::new(NodeId(1))),
            tree_id: TreeId(Uuid::from_u64_pair(0, 109)),
            focus: NodeId(2),
        };
        let value = serde_json::to_value(&expected).unwrap();
        assert_eq!(surface_update_from_value(value).unwrap(), expected);
    }

    #[test]
    fn library_errors_map_onto_surface_errors() {
        assert!(matches!(
            map_error(ServoProducerError::Busy("x".into())),
            SurfaceError::Busy { .. }
        ));
        assert!(matches!(
            map_error(ServoProducerError::Frame("x".into())),
            SurfaceError::FrameAcquisitionFailed(_)
        ));
        assert!(matches!(
            map_error(ServoProducerError::Config("x".into())),
            SurfaceError::SpawnFailed(_)
        ));
    }

    #[test]
    fn tree_ids_round_trip_their_uuid() {
        let raw = Uuid::from_u64_pair(7, 109).as_u128();
        assert_eq!(tree_id(raw), TreeId(Uuid::from_u64_pair(7, 109)));
    }
}
