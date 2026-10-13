// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
use super::*;
use inker::SessionEngine;

fn limits() -> ClipLimits {
    ClipLimits {
        max_parts: 16,
        max_bytes: 1024 * 1024,
        max_record_bytes: 128 * 1024,
    }
}
fn context() -> fleece::portable_clip::CaptureContext {
    fleece::portable_clip::CaptureContext {
        source: eidetic::clip_profile::ClipSourceV1 {
            owner: Some("shared-content-test".into()),
            entity: "selected".into(),
            revision: None,
        },
        capture_id: "shared-content-test".into(),
        provenance: eidetic::ProvenanceRecord::self_imported("explicit selected content"),
        trust: eidetic::TrustEnvelope::self_asserted(),
        captured_at: eidetic::Timestamp(7),
    }
}
#[test]
fn shared_text_clip_has_original_evidence_and_a_real_pelt_session() {
    pollster::block_on(async {
        let mut store = eidetic::MemoryBackend::new();
        let selected = serde_json::from_str(include_str!(
            "../../../../fleece/tests/fixtures/portable_clip/knot.json"
        ))
        .unwrap();
        let id = fleece::portable_clip::retain_knot_export(
            &mut store,
            None,
            &selected,
            Some("private prefix; café; private suffix".as_bytes().to_vec()),
            &context(),
            limits(),
        )
        .await
        .unwrap();
        let view = read_store(&mut store, None, id, limits()).await.unwrap();
        assert!(matches!(&view.body, ClipBody::Text(text) if text == "café"));
        let document = view.document();
        assert_eq!(document.address, address(id));
        assert!(document.blocks.iter().any(
            |b| matches!(b, Block::MetadataRow { label, .. } if label.starts_with("Source anchor"))
        ));
        let request = inker::SessionSpawnRequest::new(address(id))
            .with_body(serde_json::to_string(&document).unwrap())
            .with_viewport(600, 400);
        let mut session = ClipSessionEngine.spawn(&request).unwrap();
        session.frame(600, 400);
        assert!(session.accessibility_projection().is_some());
        let mut wrong = request.clone();
        wrong.address = "mere://clip/foreign".into();
        assert!(ClipSessionEngine.spawn(&wrong).is_err());
    });
}
#[test]
fn shared_voice_clip_keeps_exact_decoder_bytes_and_typed_controls() {
    pollster::block_on(async {
        let mut store = eidetic::MemoryBackend::new();
        let selected = serde_json::from_str(include_str!(
            "../../../../fleece/tests/fixtures/portable_clip/redshank.json"
        ))
        .unwrap();
        let voice = include_bytes!("../../../../fleece/tests/fixtures/portable_clip/voice.wav");
        let id = fleece::portable_clip::retain_redshank_export(
            &mut store,
            None,
            &selected,
            Some(voice.to_vec()),
            &context(),
            limits(),
        )
        .await
        .unwrap();
        let view = read_store(&mut store, None, id, limits()).await.unwrap();
        assert!(matches!(&view.body, ClipBody::Voice(bytes) if bytes == voice));
        assert_eq!(
            request(&format!("{}/play", address(id))).unwrap(),
            Some(ClipRequest::Play(id))
        );
        assert!(request(&format!("{}?redirect=other", address(id))).is_err());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let root = tempfile::tempdir().unwrap();
            let path = view.playback_path(root.path()).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), voice);
            std::fs::write(&path, b"corrupted cache").unwrap();
            assert!(view.playback_path(root.path()).is_err());
        }
    });
}
