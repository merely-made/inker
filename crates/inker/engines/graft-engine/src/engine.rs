// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! `GraftEngine` — the `inker::SurfaceEngine` impl + the host-supplied
//! `GraftProducerFactory` that builds a concrete graft surface per spawn.

use std::sync::Arc;

use inker::{SurfaceEngine, SurfaceError, SurfaceProducer, SurfaceSpawnRequest};

use crate::producer::{GraftProducer, GraftSurface};

/// Engine ID this engine registers under.
///
/// Not yet in `inker::routing`'s constant set; add `ENGINE_GRAFT_SERVO =
/// "graft.servo"` there (kept out of the default policy, opt-in via pin /
/// override, exactly like `ENGINE_SCRYING_WEB`) when wiring meerkat. The
/// `engine_id_matches_routing_constant` assertion in `scrying-engine` is the
/// shape to mirror once the constant exists.
pub const GRAFT_SERVO_ENGINE_ID: &str = "graft.servo";

/// Host-supplied factory that builds a concrete [`GraftSurface`] for a resolved
/// [`SurfaceSpawnRequest`].
///
/// The engine can't build the surface itself: it needs the host's wgpu device,
/// an embedded `servo::Servo` instance + `WebView`, and the
/// `ServoWgpuInteropAdapter` bound to that device — none of which this crate
/// depends on. The host implements this trait once (behind its `engine-graft`
/// feature) and hands an `Arc` of it to [`GraftEngine::new`].
pub trait GraftProducerFactory: Send + Sync {
    /// Build a fresh graft surface for this request, or a
    /// [`SurfaceError::SpawnFailed`] describing why.
    fn build(&self, request: &SurfaceSpawnRequest) -> Result<Box<dyn GraftSurface>, SurfaceError>;
}

/// `inker::SurfaceEngine` impl backed by wgpu-graft.
pub struct GraftEngine {
    factory: Arc<dyn GraftProducerFactory>,
}

impl GraftEngine {
    /// Construct the engine. The host's `factory` knows how to build a concrete
    /// graft surface.
    pub fn new(factory: Arc<dyn GraftProducerFactory>) -> Self {
        Self { factory }
    }
}

impl SurfaceEngine for GraftEngine {
    fn engine_id(&self) -> &str {
        GRAFT_SERVO_ENGINE_ID
    }

    // Accessibility remains Opaque by default. The optional GraftSurface
    // semantic/control methods forward host exports without certifying them
    // or promoting a factory's native assistive-technology capability.

    #[tracing::instrument(level = "debug", skip(self, request), fields(url = %request.url))]
    fn spawn(
        &self,
        request: &SurfaceSpawnRequest,
    ) -> Result<Box<dyn SurfaceProducer>, SurfaceError> {
        let surface = self.factory.build(request).map_err(|err| {
            tracing::warn!(?err, "graft producer factory failed");
            err
        })?;
        Ok(Box::new(GraftProducer::new(surface)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inker::{
        CapabilityStatus, EngineProfileBinding, SurfaceEngineRegistry,
        routing::{EngineRouteDecision, SurfaceContract, SurfaceContractMode, SurfaceTargetId},
    };

    use crate::producer::GraftFrame;
    use inker::{
        CursorShape, DragEvent, DragOperationSet, FocusReason, KeyboardEvent, MouseEvent,
        NavigationEvent, PhysicalPosition, PointerEvent, SurfaceSettings, WebMessage,
    };

    /// Minimal surface stub: navigable, no frames, no events. Drives the spawn
    /// pipeline through the registry without a Servo instance.
    #[derive(Default)]
    struct StubSurface {
        ordered_events: Option<std::collections::VecDeque<inker::WebSurfaceEvent>>,
    }

    impl GraftSurface for StubSurface {
        fn resize(&mut self, _: u32, _: u32) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn acquire_frame(&mut self) -> Result<Option<GraftFrame>, SurfaceError> {
            Ok(None)
        }
        fn load_url(&mut self, _: &str) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn load_html(&mut self, _: &str) -> Result<(), SurfaceError> {
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
        fn notify_mouse(&mut self, _: MouseEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn notify_pointer(&mut self, _: PointerEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn notify_drag(&mut self, _: DragEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn finish_drag_source(
            &mut self,
            _: PhysicalPosition,
            _: DragOperationSet,
        ) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn notify_keyboard(&mut self, _: KeyboardEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn focus(&mut self, _: FocusReason) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn poll_navigation_event(&mut self) -> Option<NavigationEvent> {
            assert!(
                self.ordered_events.is_none(),
                "ordered host must not be split into legacy queues"
            );
            None
        }
        fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
            None
        }
        fn poll_web_message(&mut self) -> Option<WebMessage> {
            assert!(
                self.ordered_events.is_none(),
                "ordered host must not be split into legacy queues"
            );
            None
        }
        fn poll_web_event(&mut self) -> Option<inker::WebSurfaceEvent> {
            self.ordered_events
                .as_mut()
                .and_then(|events| events.pop_front())
        }
        fn apply_settings(&mut self, _: &SurfaceSettings) -> Result<(), SurfaceError> {
            Ok(())
        }
    }

    struct StubFactory;
    impl GraftProducerFactory for StubFactory {
        fn build(&self, _: &SurfaceSpawnRequest) -> Result<Box<dyn GraftSurface>, SurfaceError> {
            Ok(Box::new(StubSurface::default()))
        }
    }

    #[derive(Default)]
    struct SemanticWitness {
        updates: std::collections::VecDeque<inker::SurfaceAccessibilityUpdate>,
        controls: Vec<&'static str>,
        actions: Vec<inker::SurfaceAccessibilityActionRequest>,
    }

    struct SemanticFactory {
        witness: Arc<std::sync::Mutex<SemanticWitness>>,
        root: accesskit::TreeId,
        replacement: accesskit::TreeId,
    }

    impl GraftProducerFactory for SemanticFactory {
        fn build(&self, _: &SurfaceSpawnRequest) -> Result<Box<dyn GraftSurface>, SurfaceError> {
            Ok(Box::new(SemanticSurface {
                witness: self.witness.clone(),
                root: self.root,
                replacement: self.replacement,
            }))
        }
    }

    struct SemanticSurface {
        witness: Arc<std::sync::Mutex<SemanticWitness>>,
        root: accesskit::TreeId,
        replacement: accesskit::TreeId,
    }

    impl GraftSurface for SemanticSurface {
        fn resize(&mut self, _: u32, _: u32) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn acquire_frame(&mut self) -> Result<Option<GraftFrame>, SurfaceError> {
            panic!("semantic control/delivery must not acquire a GPU frame")
        }
        fn load_url(&mut self, _: &str) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn load_html(&mut self, _: &str) -> Result<(), SurfaceError> {
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
        fn notify_mouse(&mut self, _: MouseEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn notify_pointer(&mut self, _: PointerEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn notify_drag(&mut self, _: DragEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn finish_drag_source(
            &mut self,
            _: PhysicalPosition,
            _: DragOperationSet,
        ) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn notify_keyboard(&mut self, _: KeyboardEvent) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn focus(&mut self, _: FocusReason) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn poll_navigation_event(&mut self) -> Option<NavigationEvent> {
            None
        }
        fn poll_cursor_shape(&mut self) -> Option<CursorShape> {
            None
        }
        fn poll_web_message(&mut self) -> Option<WebMessage> {
            None
        }
        fn apply_settings(&mut self, _: &SurfaceSettings) -> Result<(), SurfaceError> {
            Ok(())
        }

        fn set_accessibility_active(
            &mut self,
            active: bool,
        ) -> Result<Option<inker::SurfaceAccessibilityTreeId>, SurfaceError> {
            let mut witness = self.witness.lock().unwrap();
            witness
                .controls
                .push(if active { "activate" } else { "deactivate" });
            if active {
                Ok(Some(self.root))
            } else {
                witness.updates.clear();
                Ok(None)
            }
        }
        fn poll_accessibility_update(&mut self) -> Option<inker::SurfaceAccessibilityUpdate> {
            self.witness.lock().unwrap().updates.pop_front()
        }
        fn request_accessibility_resync(
            &mut self,
        ) -> Result<inker::SurfaceAccessibilityTreeId, SurfaceError> {
            let mut witness = self.witness.lock().unwrap();
            witness.controls.push("resync");
            witness.updates.clear();
            witness
                .updates
                .push_back(semantic_update(self.replacement, None));
            self.root = self.replacement;
            Ok(self.root)
        }
        fn send_accessibility_action(
            &mut self,
            request: inker::SurfaceAccessibilityActionRequest,
        ) -> Result<(), SurfaceError> {
            let refused = request.action == accesskit::Action::Focus;
            self.witness.lock().unwrap().actions.push(request);
            if refused {
                Err(SurfaceError::Unsupported("witness focus refusal".into()))
            } else {
                Ok(())
            }
        }
    }

    fn semantic_update(
        tree_id: accesskit::TreeId,
        nested: Option<accesskit::TreeId>,
    ) -> inker::SurfaceAccessibilityUpdate {
        use accesskit::{Action, Node, NodeId, Rect, Role, Tree, TreeUpdate};
        let mut node = Node::new(if nested.is_some() {
            Role::Document
        } else {
            Role::TextInput
        });
        node.set_label("semantic witness");
        node.set_bounds(Rect::new(1.0, 2.0, 30.0, 40.0));
        if let Some(nested) = nested {
            node.set_tree_id(nested);
        } else {
            node.add_action(Action::SetValue);
        }
        TreeUpdate {
            nodes: vec![(NodeId(1), node)],
            tree: Some(Tree::new(NodeId(1))),
            tree_id,
            focus: NodeId(1),
        }
    }

    #[test]
    fn legacy_graft_accessibility_defaults_remain_opaque_and_refuse_controls() {
        let engine = GraftEngine::new(Arc::new(StubFactory));
        assert_eq!(engine.a11y_capability(), inker::A11yCapability::Opaque);
        let mut producer = engine.spawn(&stub_request()).unwrap();
        assert!(producer.poll_accessibility_update().is_none());
        assert!(matches!(
            producer.set_accessibility_active(true),
            Err(SurfaceError::Unsupported(_))
        ));
        assert!(matches!(
            producer.request_accessibility_resync(),
            Err(SurfaceError::Unsupported(_))
        ));
        assert!(matches!(
            producer.send_accessibility_action(accesskit::ActionRequest {
                action: accesskit::Action::Click,
                target_tree: accesskit::TreeId::ROOT,
                target_node: accesskit::NodeId(1),
                data: None,
            }),
            Err(SurfaceError::Unsupported(_))
        ));
        assert!(matches!(
            producer
                .as_web_surface()
                .unwrap()
                .capabilities()
                .accessibility,
            inker::WebFeatureStatus::Unsupported { .. }
        ));
    }

    #[test]
    fn spawned_graft_preserves_semantic_fifo_nested_ids_and_typed_action_data() {
        use accesskit::{Action, ActionData, ActionRequest, NodeId, TreeId, Uuid};
        let root = TreeId(Uuid::from_u128(11));
        let document = TreeId(Uuid::from_u128(12));
        let replacement = TreeId(Uuid::from_u128(13));
        let updates = [
            semantic_update(root, Some(document)),
            semantic_update(document, None),
        ];
        let witness = Arc::new(std::sync::Mutex::new(SemanticWitness {
            updates: updates.clone().into(),
            ..SemanticWitness::default()
        }));
        let engine = GraftEngine::new(Arc::new(SemanticFactory {
            witness: witness.clone(),
            root,
            replacement,
        }));
        // Exercise the public erased spawn path, not concrete producer calls.
        let mut producer = engine.spawn(&stub_request()).unwrap();
        assert_eq!(producer.set_accessibility_active(true).unwrap(), Some(root));
        for expected in updates {
            assert_eq!(producer.poll_accessibility_update(), Some(expected));
        }
        assert!(producer.poll_accessibility_update().is_none());
        let action = ActionRequest {
            action: Action::SetValue,
            target_tree: document,
            target_node: NodeId(1),
            data: Some(ActionData::Value("retained typed data".into())),
        };
        producer.send_accessibility_action(action.clone()).unwrap();
        let refused = ActionRequest {
            action: Action::Focus,
            data: None,
            ..action.clone()
        };
        assert_eq!(
            producer.send_accessibility_action(refused.clone()),
            Err(SurfaceError::Unsupported("witness focus refusal".into()))
        );
        assert_eq!(witness.lock().unwrap().actions, vec![action, refused]);
        // A stale queued delta is discarded by the producer's resync policy;
        // the shared adapter must return the new root and fresh queue unchanged.
        witness
            .lock()
            .unwrap()
            .updates
            .push_back(semantic_update(document, None));
        assert_eq!(
            producer.request_accessibility_resync().unwrap(),
            replacement
        );
        assert_eq!(
            producer.poll_accessibility_update(),
            Some(semantic_update(replacement, None))
        );
        assert!(producer.poll_accessibility_update().is_none());
        witness
            .lock()
            .unwrap()
            .updates
            .push_back(semantic_update(replacement, None));
        assert_eq!(producer.set_accessibility_active(false).unwrap(), None);
        assert!(producer.poll_accessibility_update().is_none());
        assert_eq!(
            witness.lock().unwrap().controls,
            vec!["activate", "resync", "deactivate"]
        );
    }

    struct FailFactory;
    impl GraftProducerFactory for FailFactory {
        fn build(&self, _: &SurfaceSpawnRequest) -> Result<Box<dyn GraftSurface>, SurfaceError> {
            Err(SurfaceError::SpawnFailed("no host context".into()))
        }
    }

    fn stub_request() -> SurfaceSpawnRequest {
        SurfaceSpawnRequest {
            url: "https://servo.org".into(),
            width: 800,
            height: 600,
            profile: EngineProfileBinding {
                user_data_dir: "/tmp/test-profile".into(),
            },
            fence_handle: None,
        }
    }

    fn decision() -> EngineRouteDecision {
        EngineRouteDecision {
            engine_id: GRAFT_SERVO_ENGINE_ID.to_string(),
            surface_contract: SurfaceContract {
                target: SurfaceTargetId::new("tile:1"),
                mode: SurfaceContractMode::CompositedTexture,
            },
        }
    }

    #[test]
    fn registers_and_spawns_through_registry() {
        let mut reg = SurfaceEngineRegistry::new();
        reg.register(Box::new(GraftEngine::new(Arc::new(StubFactory))));
        assert!(reg.contains(GRAFT_SERVO_ENGINE_ID));

        let mut producer = reg
            .spawn(&decision(), &stub_request())
            .ok()
            .expect("spawn ok");
        // StubSurface yields no frame.
        match producer.acquire_frame() {
            Ok(opt) => assert!(opt.is_none()),
            Err(err) => panic!("unexpected acquire_frame err: {err:?}"),
        }
    }

    #[test]
    fn graft_capture_stays_unsupported_until_the_protocol_is_wired() {
        let caps = StubSurface::default().web_capabilities().document;
        assert!(matches!(
            caps.find_in_page,
            CapabilityStatus::Unsupported { .. }
        ));
        assert!(matches!(caps.page_zoom, CapabilityStatus::Partial { .. }));
        assert!(matches!(
            caps.page_capture,
            CapabilityStatus::Unsupported { .. }
        ));
        assert!(matches!(caps.navigation, CapabilityStatus::Partial { .. }));
    }

    #[test]
    fn factory_failure_surfaces_as_spawn_failed() {
        let engine = GraftEngine::new(Arc::new(FailFactory));
        let result = engine.spawn(&stub_request());
        assert!(matches!(result, Err(SurfaceError::SpawnFailed(_))));
    }

    #[test]
    fn producer_retains_host_callback_order_across_event_kinds() {
        use inker::{WebRequestId, WebSurface, WebSurfaceEvent};
        let events = [
            WebSurfaceEvent::TitleChanged {
                title: "first".into(),
            },
            WebSurfaceEvent::ScriptCompleted {
                id: WebRequestId::new(7),
                result: Ok("second".into()),
            },
            WebSurfaceEvent::Navigation(NavigationEvent::Finished {
                url: "https://servo.org/".into(),
                title: Some("third".into()),
            }),
            WebSurfaceEvent::AddressChanged {
                url: "https://servo.org/#fourth".into(),
            },
        ];
        let mut producer = GraftProducer::new(Box::new(StubSurface {
            ordered_events: Some(events.clone().into()),
        }));
        for expected in events {
            assert_eq!(producer.poll_web_event(), Some(expected));
        }
        assert_eq!(producer.poll_web_event(), None);
    }
}
