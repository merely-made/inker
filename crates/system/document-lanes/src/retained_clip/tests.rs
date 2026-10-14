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
        let exported = export_body(&mut store, None, id, limits()).await.unwrap();
        let package: fleece::portable_clip::ClipPackageV1 =
            serde_json::from_slice(&exported.bytes).unwrap();
        assert_eq!(package.clip, view.clip);
        assert_eq!(package.payloads.len(), 1);
        assert_eq!(package.payloads[0].bytes, voice);
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

#[test]
fn selected_export_roundtrips_exact_body_and_source_without_private_backing() {
    pollster::block_on(async {
        let mut sender = eidetic::MemoryBackend::new();
        let selected = serde_json::from_str(include_str!(
            "../../../../fleece/tests/fixtures/portable_clip/knot.json"
        ))
        .unwrap();
        let id = fleece::portable_clip::retain_knot_export(
            &mut sender,
            None,
            &selected,
            Some("private prefix; café; private suffix".as_bytes().to_vec()),
            &context(),
            limits(),
        )
        .await
        .unwrap();
        let original = read_store(&mut sender, None, id, limits()).await.unwrap();
        let output = export_body(&mut sender, None, id, limits()).await.unwrap();
        assert_eq!(
            output.filename,
            format!("selected-clip-{}.mereclip", id.0.to_hex())
        );
        let package: fleece::portable_clip::ClipPackageV1 =
            serde_json::from_slice(&output.bytes).unwrap();
        assert_eq!(package.clip, original.clip);
        assert_eq!(package.payloads.len(), 1);
        assert_eq!(package.payloads[0].bytes, "café".as_bytes());
        let wire = String::from_utf8(output.bytes).unwrap();
        assert!(!wire.contains("private_backing"));
        assert!(!wire.contains("provenance"));
        let mut receiver = eidetic::MemoryBackend::new();
        let received = fleece::portable_clip::import_clip_package(
            &mut receiver,
            None,
            &package,
            &context(),
            limits(),
        )
        .await
        .unwrap();
        let reopened = read_store(&mut receiver, None, received, limits())
            .await
            .unwrap();
        assert_eq!(reopened.clip, original.clip);
        assert!(matches!(reopened.body, ClipBody::Text(body) if body == "café"));
        let mut small = limits();
        small.max_bytes = 3;
        assert!(export_body(&mut sender, None, id, small).await.is_err());
        let absent = ManifestId::from_hash(Hash::of(b"foreign"));
        assert!(
            export_body(&mut sender, None, absent, limits())
                .await
                .is_err()
        );
        let mut overflow = limits();
        overflow.max_bytes = u64::MAX;
        assert!(export_body(&mut sender, None, id, overflow).await.is_err());
    });
}

#[test]
fn export_without_a_custody_adapter_refuses_a_sealed_clip() {
    // Marker wiring only. Cryptographic qualification belongs to the sealer.
    struct SealedFixture;
    impl eidetic::PayloadSealer for SealedFixture {
        fn seal(
            &self,
            _: &Hash,
            bytes: &[u8],
        ) -> eidetic::Result<(Vec<u8>, eidetic::SealedBlobRef)> {
            Ok((
                bytes.iter().map(|b| b ^ 0xaa).collect(),
                eidetic::SealedBlobRef {
                    epoch: eidetic::SealEpochId([1; 16]),
                    format: "export-refusal-fixture".into(),
                },
            ))
        }
        fn unseal(
            &self,
            _: &Hash,
            _: &eidetic::SealedBlobRef,
            _: &[u8],
        ) -> eidetic::Result<Vec<u8>> {
            Err(eidetic::Error::new("fixture key unavailable"))
        }
    }
    pollster::block_on(async {
        let mut store = eidetic::MemoryBackend::new();
        let selected = serde_json::from_str(include_str!(
            "../../../../fleece/tests/fixtures/portable_clip/knot.json"
        ))
        .unwrap();
        let id = fleece::portable_clip::retain_knot_export(
            &mut store,
            Some(&SealedFixture),
            &selected,
            None,
            &context(),
            limits(),
        )
        .await
        .unwrap();
        assert!(export_body(&mut store, None, id, limits()).await.is_err());
        assert!(
            export_body(&mut store, Some(&SealedFixture), id, limits())
                .await
                .is_err()
        );
    });
}
