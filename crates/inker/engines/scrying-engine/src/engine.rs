// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! `ScryingTileEngine` — the `inker::SurfaceEngine` impl + host-supplied
//! `ProducerFactory` for constructing concrete scrying producers.

use std::sync::Arc;

use inker::{
    A11yCapability, PresentationDeclaration, SurfaceEngine, SurfaceError, SurfaceProducer,
    SurfaceSpawnRequest,
};
use scrying::WebSurfaceProducer;

use crate::producer::ScryingProducer;

/// Engine ID this engine registers under. Mirrors `inker::routing::ENGINE_SCRYING_WEB`.
pub const SCRYING_WEB_ENGINE_ID: &str = "scrying.web";

/// Host-supplied factory that builds a concrete scrying producer given a
/// resolved [`SurfaceSpawnRequest`].
///
/// The engine itself can't construct platform producers because they need
/// host resources the engine doesn't own:
/// - parent HWND (Windows) / parent NSView (macOS) / GTK widget (Linux),
/// - the host's wgpu device (or fence share-handle for explicit GPU sync),
/// - per-platform composition controller plumbing.
///
/// The host implements this trait once (per platform) and hands an `Arc` of
/// it to [`ScryingTileEngine::new`]. The engine then plumbs the spawn
/// request through on each tile spawn.
pub trait ProducerFactory: Send + Sync {
    /// Build a fresh scrying producer for this request, or return a
    /// [`SurfaceError::SpawnFailed`] describing why.
    fn build(
        &self,
        request: &SurfaceSpawnRequest,
    ) -> Result<Box<dyn WebSurfaceProducer>, SurfaceError>;
}

/// `inker::SurfaceEngine` impl backed by scrying.
pub struct ScryingTileEngine {
    declaration: PresentationDeclaration,
    factory: Arc<dyn ProducerFactory>,
}

impl ScryingTileEngine {
    /// Construct the engine. The host's `factory` knows how to build a
    /// platform producer.
    pub fn new(factory: Arc<dyn ProducerFactory>) -> Self {
        Self {
            declaration: PresentationDeclaration::surface(SCRYING_WEB_ENGINE_ID),
            factory,
        }
    }
}

impl SurfaceEngine for ScryingTileEngine {
    fn engine_id(&self) -> &str {
        &self.declaration.id
    }

    fn a11y_capability(&self) -> A11yCapability {
        self.declaration.reaches.a11y
    }

    /// Composited frames from the system WebView; opaque to assistive technology, and
    /// presents no scheme or content type, so it is reached by pin or override.
    fn declaration(&self) -> PresentationDeclaration {
        self.declaration.clone()
    }

    #[tracing::instrument(level = "debug", skip(self, request), fields(url = %request.url))]
    fn spawn(
        &self,
        request: &SurfaceSpawnRequest,
    ) -> Result<Box<dyn SurfaceProducer>, SurfaceError> {
        let inner = self.factory.build(request).map_err(|err| {
            tracing::warn!(?err, "scrying producer factory failed");
            err
        })?;
        Ok(Box::new(ScryingProducer::new(inner, request.fence_handle)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inker::{
        EngineProfileBinding, SurfaceEngineRegistry, SurfaceFrame,
        routing::{EngineRouteDecision, SurfaceContract, SurfaceContractMode, SurfaceTargetId},
    };
    use scrying::SystemWebviewBackend;
    use scrying::{
        WebSurfaceCapabilities, WebSurfaceError, WebSurfaceFrame, WebSurfaceMode,
        native_frame::{CapabilityStatus, NativeFrameKind, UnsupportedReason},
    };

    /// Minimal producer stub — implements scrying's trait, returns OverlayOnly
    /// frames + Unsupported for everything else. Used to drive the spawn
    /// pipeline through the registry in tests.
    struct OverlayStub;

    impl WebSurfaceProducer for OverlayStub {
        fn capabilities(&self) -> WebSurfaceCapabilities {
            WebSurfaceCapabilities {
                backend: SystemWebviewBackend::Unknown,
                preferred_mode: WebSurfaceMode::NativeChildOverlay,
                imported_texture: CapabilityStatus::Unsupported(
                    UnsupportedReason::PlatformNotImplemented,
                ),
                native_child_overlay: CapabilityStatus::Supported,
                cpu_snapshot: CapabilityStatus::Unsupported(
                    UnsupportedReason::PlatformNotImplemented,
                ),
                supported_frames: vec![NativeFrameKind::Dx12SharedTexture],
                reason: "test stub",
                features: WebSurfaceCapabilities::probe(None).features,
            }
        }
        fn acquire_frame(&mut self) -> Result<WebSurfaceFrame, WebSurfaceError> {
            Ok(WebSurfaceFrame::OverlayOnly)
        }
    }

    struct StubFactory;
    impl ProducerFactory for StubFactory {
        fn build(
            &self,
            _: &SurfaceSpawnRequest,
        ) -> Result<Box<dyn WebSurfaceProducer>, SurfaceError> {
            Ok(Box::new(OverlayStub))
        }
    }

    struct FailFactory;
    impl ProducerFactory for FailFactory {
        fn build(
            &self,
            _: &SurfaceSpawnRequest,
        ) -> Result<Box<dyn WebSurfaceProducer>, SurfaceError> {
            Err(SurfaceError::SpawnFailed("no host context".into()))
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

    fn decision() -> EngineRouteDecision {
        EngineRouteDecision {
            engine_id: SCRYING_WEB_ENGINE_ID.to_string(),
            surface_contract: SurfaceContract {
                target: SurfaceTargetId::new("tile:1"),
                mode: SurfaceContractMode::CompositedTexture,
            },
        }
    }

    #[test]
    fn declaration_names_the_engine_and_presents_nothing_by_default() {
        let engine = ScryingTileEngine::new(Arc::new(StubFactory));
        let declaration = engine.declaration();
        assert_eq!(declaration.id, SCRYING_WEB_ENGINE_ID);
        assert_eq!(engine.engine_id(), declaration.id);
        assert_eq!(declaration.reaches.a11y, engine.a11y_capability());
        assert!(declaration.presents_nothing());
    }

    #[test]
    fn engine_id_matches_routing_constant() {
        assert_eq!(SCRYING_WEB_ENGINE_ID, inker::routing::ENGINE_SCRYING_WEB);
    }

    #[test]
    fn registers_and_spawns_through_registry() {
        let mut reg = SurfaceEngineRegistry::new();
        reg.register(Box::new(ScryingTileEngine::new(Arc::new(StubFactory))));
        assert!(reg.contains(SCRYING_WEB_ENGINE_ID));

        let mut producer = reg
            .spawn(&decision(), &stub_request())
            .ok()
            .expect("spawn ok");

        // OverlayStub → WebSurfaceFrame::OverlayOnly maps to None for v1
        // (no native texture available).
        match producer.acquire_frame() {
            Ok(opt) => assert!(matches!(opt, Option::<SurfaceFrame>::None)),
            Err(err) => panic!("unexpected acquire_frame err: {err:?}"),
        }
    }

    #[test]
    fn factory_failure_surfaces_as_spawn_failed() {
        let engine = ScryingTileEngine::new(Arc::new(FailFactory));
        let result = engine.spawn(&stub_request());
        assert!(matches!(result, Err(SurfaceError::SpawnFailed(_))));
    }
}
