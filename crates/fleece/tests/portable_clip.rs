// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

#![cfg(all(feature = "portable-clip", not(target_arch = "wasm32")))]

use eidetic::clip_profile::{ClipSourceV1, ClipTagV1, PortableClipV1, RetainedClipV1};
use eidetic::{
    Hash, ManifestId, MemoryBackend, NoFetcher, ProvenanceRecord, Store, Timestamp, TrustEnvelope,
};
use fleece::portable_clip::*;
use serde_json::{Value, json};

fn limits() -> ClipLimits {
    ClipLimits {
        max_parts: 16,
        max_bytes: 1_000_000,
        max_record_bytes: 64_000,
    }
}
fn context(owner: &str) -> CaptureContext {
    CaptureContext {
        source: ClipSourceV1 {
            owner: Some(owner.into()),
            entity: "durable-owner-entity".into(),
            revision: None,
        },
        capture_id: "encounter-1".into(),
        provenance: ProvenanceRecord::self_imported("PRIVATE custody evidence"),
        trust: TrustEnvelope::self_asserted(),
        captured_at: Timestamp(7),
    }
}
fn clip() -> PortableClipV1 {
    PortableClipV1 {
        version: 1,
        clip_id: "clip-1".into(),
        capture_id: "encounter-1".into(),
        title: "Selected content".into(),
        source: context("test").source,
        source_evidence: Value::Null,
        parts: vec![],
        anchors: vec![],
        tags: vec![],
    }
}
fn part(name: &str, bytes: &[u8], dependencies: &[&str]) -> PartInput {
    PartInput {
        name: name.into(),
        role: "selected-body".into(),
        media_type: "text/plain".into(),
        bytes: bytes.into(),
        dependencies: dependencies.iter().map(|s| (*s).into()).collect(),
    }
}
async fn load(store: &mut dyn Store, id: ManifestId) -> RetainedClipV1 {
    eidetic::load_typed::<RetainedClipV1>(store, &mut NoFetcher, id)
        .await
        .unwrap()
        .unwrap()
}

#[test]
fn native_owner_exports_survive_receiver_only_disk_reopen() {
    pollster::block_on(async {
        let knot: Value =
            serde_json::from_str(include_str!("fixtures/portable_clip/knot.json")).unwrap();
        let redshank: Value =
            serde_json::from_str(include_str!("fixtures/portable_clip/redshank.json")).unwrap();
        let mut sender = MemoryBackend::new();
        let backing = b"private prefix; caf\xc3\xa9; private suffix".to_vec();
        let knot_id = retain_knot_export(
            &mut sender,
            None,
            &knot,
            Some(backing),
            &context("knot"),
            limits(),
        )
        .await
        .unwrap();
        let held = load(&mut sender, knot_id).await;
        assert_eq!(held.private_backing.len(), 1);
        assert!(
            export_selected_clip(
                &mut sender,
                None,
                knot_id,
                &["source-backing".into()],
                limits()
            )
            .await
            .is_err()
        );
        let knot_package =
            export_selected_clip(&mut sender, None, knot_id, &["body".into()], limits())
                .await
                .unwrap();
        let voice = include_bytes!("fixtures/portable_clip/voice.wav").to_vec();
        assert!(
            retain_redshank_export(
                &mut sender,
                None,
                &redshank,
                None,
                &context("redshank"),
                limits()
            )
            .await
            .is_err()
        );
        let note_id = retain_redshank_export(
            &mut sender,
            None,
            &redshank,
            Some(voice.clone()),
            &context("redshank"),
            limits(),
        )
        .await
        .unwrap();
        let note_package =
            export_selected_clip(&mut sender, None, note_id, &["body".into()], limits())
                .await
                .unwrap();
        let wire = serde_json::to_vec(&(knot_package, note_package)).unwrap();
        let text = String::from_utf8(wire.clone()).unwrap();
        assert!(!text.contains("private prefix"));
        assert!(!text.contains("PRIVATE custody"));
        assert!(!text.contains("private_backing"));
        drop(sender);

        let (knot_package, note_package): (ClipPackageV1, ClipPackageV1) =
            serde_json::from_slice(&wire).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let mut receiver = muniment::FjallStore::open(directory.path()).unwrap();
        let received_knot = import_clip_package(
            &mut receiver,
            None,
            &knot_package,
            &context("receiver"),
            limits(),
        )
        .await
        .unwrap();
        let received_note = import_clip_package(
            &mut receiver,
            None,
            &note_package,
            &context("receiver"),
            limits(),
        )
        .await
        .unwrap();
        drop(receiver);
        let mut reopened = muniment::FjallStore::open(directory.path()).unwrap();
        let knot_copy = load(&mut reopened, received_knot).await;
        let note_copy = load(&mut reopened, received_note).await;
        assert!(knot_copy.private_backing.is_empty());
        assert!(note_copy.private_backing.is_empty());
        assert_eq!(knot_copy.clip.anchors[0].units, "utf8-bytes");
        assert_eq!(knot_copy.clip.source_evidence["source_tags"][0], "coffee");
        assert!(knot_copy.clip.tags.is_empty());
        assert_eq!(note_copy.clip.anchors[0].units, "milliseconds");
        assert_eq!(
            note_copy.clip.anchors[0].selector["pressed_offset_ms"],
            1400
        );
        assert_eq!(
            note_copy.clip.anchors[0].selector["representation"]["etag"],
            "\"original\""
        );
        for (copy, expected) in [(knot_copy, "café".as_bytes().to_vec()), (note_copy, voice)] {
            let body = eidetic::load_typed::<eidetic::models::OpaqueBlob>(
                &mut reopened,
                &mut NoFetcher,
                copy.clip.parts[0].manifest,
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(body.0, expected);
        }
        // This is a playable voice body, while its recording remains a reference.
        assert_eq!(&note_package.payloads[0].bytes[..4], b"RIFF");
        assert_eq!(note_package.payloads.len(), 1);
    });
}

#[test]
fn invalid_packages_are_refused_before_receiver_writes() {
    pollster::block_on(async {
        let mut sender = MemoryBackend::new();
        let id = retain_clip(
            &mut sender,
            None,
            clip(),
            vec![part("body", b"selected", &[])],
            vec![part("backing", b"SECRET", &[])],
            &context("test"),
            limits(),
        )
        .await
        .unwrap();
        let package = export_selected_clip(&mut sender, None, id, &["body".into()], limits())
            .await
            .unwrap();
        let mut foreign_record = serde_json::to_value(&package).unwrap();
        foreign_record["clip"]["future_domain_fact"] = json!("must not be silently discarded");
        assert!(serde_json::from_value::<ClipPackageV1>(foreign_record).is_err());
        let mut missing = package.clone();
        missing.payloads.clear();
        let mut changed = package.clone();
        changed.payloads[0].bytes[0] ^= 1;
        let mut extra = package.clone();
        extra.payloads.push(ClipPayloadV1 {
            id: ManifestId::from_hash(Hash::of(b"SECRET")),
            bytes: b"SECRET".to_vec(),
        });
        let mut duplicate = package.clone();
        duplicate.payloads.push(duplicate.payloads[0].clone());
        let mut dangling = package.clone();
        dangling.clip.parts[0].dependencies.push("backing".into());
        for bad in [missing, changed, extra, duplicate, dangling] {
            let mut receiver = MemoryBackend::new();
            assert!(
                import_clip_package(&mut receiver, None, &bad, &context("receiver"), limits())
                    .await
                    .is_err()
            );
            assert!(receiver.list("").await.unwrap().is_empty());
        }
        let mut receiver = MemoryBackend::new();
        assert!(
            import_clip_package(
                &mut receiver,
                None,
                &package,
                &context("receiver"),
                ClipLimits {
                    max_bytes: 1,
                    ..limits()
                }
            )
            .await
            .is_err()
        );
        assert!(receiver.list("").await.unwrap().is_empty());
    });
}

#[test]
fn selected_dependency_closure_is_bounded_and_does_not_traverse_backing() {
    pollster::block_on(async {
        let mut store = MemoryBackend::new();
        let id = retain_clip(
            &mut store,
            None,
            clip(),
            vec![
                part("body", b"body", &["asset"]),
                part("asset", b"asset", &["body"]),
                part("other", b"unselected", &[]),
            ],
            vec![part("backing", b"SECRET", &[])],
            &context("test"),
            limits(),
        )
        .await
        .unwrap();
        let package = export_selected_clip(&mut store, None, id, &["body".into()], limits())
            .await
            .unwrap();
        assert_eq!(package.clip.parts.len(), 2);
        assert_eq!(package.payloads.len(), 2);
        assert!(
            !serde_json::to_string(&package)
                .unwrap()
                .contains("unselected")
        );
        assert!(
            export_selected_clip(
                &mut store,
                None,
                id,
                &["body".into()],
                ClipLimits {
                    max_parts: 1,
                    ..limits()
                }
            )
            .await
            .is_err()
        );
        assert!(
            retain_clip(
                &mut store,
                None,
                clip(),
                vec![part("body", b"body", &["backing"])],
                vec![part("backing", b"SECRET", &[])],
                &context("test"),
                limits()
            )
            .await
            .is_err()
        );
    });
}

#[test]
fn tag_attribution_and_native_anchor_refusals_survive_adapters() {
    pollster::block_on(async {
        let mut store = MemoryBackend::new();
        let mut record = clip();
        record.tags = ["author-a", "author-b"]
            .into_iter()
            .map(|owner| ClipTagV1 {
                concept: format!("{owner}/coffee"),
                owner: owner.into(),
                label: "coffee".into(),
                target: "clip-1".into(),
                statement_id: format!("{owner}/statement"),
                asserter: Some(owner.into()),
                scope: json!({"kind":"source"}),
                asserted_at_ms: Some(7),
            })
            .collect();
        let tags = record.tags.clone();
        let id = retain_clip(
            &mut store,
            None,
            record,
            vec![part("body", b"body", &[])],
            vec![],
            &context("test"),
            limits(),
        )
        .await
        .unwrap();
        let package = export_selected_clip(&mut store, None, id, &["body".into()], limits())
            .await
            .unwrap();
        let mut receiver = MemoryBackend::new();
        let received = import_clip_package(
            &mut receiver,
            None,
            &package,
            &context("receiver"),
            limits(),
        )
        .await
        .unwrap();
        assert_eq!(load(&mut receiver, received).await.clip.tags, tags);
        let mut knot: Value =
            serde_json::from_str(include_str!("fixtures/portable_clip/knot.json")).unwrap();
        let backing = b"private prefix; caf\xc3\xa9; private suffix".to_vec();
        assert!(
            retain_knot_export(
                &mut store,
                None,
                &knot,
                Some(b"different".to_vec()),
                &context("test"),
                limits()
            )
            .await
            .is_err()
        );
        knot["document_source"]["byte_span"]["start"] = json!(20);
        knot["document_source"]["byte_span"]["end"] = json!(25);
        assert!(
            retain_knot_export(
                &mut store,
                None,
                &knot,
                Some(backing),
                &context("test"),
                limits()
            )
            .await
            .is_err()
        );
        let mut redshank: Value =
            serde_json::from_str(include_str!("fixtures/portable_clip/redshank.json")).unwrap();
        assert!(
            retain_redshank_export(
                &mut store,
                None,
                &redshank,
                Some(vec![]),
                &context("test"),
                limits()
            )
            .await
            .is_err()
        );
        redshank["annotation"]["body"] = json!({"kind":"text", "plain_text":"  personal note  "});
        let text_note = retain_redshank_export(
            &mut store,
            None,
            &redshank,
            None,
            &context("test"),
            limits(),
        )
        .await
        .unwrap();
        let text_package =
            export_selected_clip(&mut store, None, text_note, &["body".into()], limits())
                .await
                .unwrap();
        assert_eq!(text_package.payloads[0].bytes, b"  personal note  ");
        redshank["annotation"]["privacy"] = json!("private");
        assert!(
            retain_redshank_export(
                &mut store,
                None,
                &redshank,
                Some(vec![1]),
                &context("test"),
                limits()
            )
            .await
            .is_err()
        );
    });
}

// Wiring double only. AEAD and epoch rotation remain qualified by Eidetic and
// Pandect's existing sealing tests, not by this reversible transformation.
struct WiringSealer;
impl eidetic::PayloadSealer for WiringSealer {
    fn seal(&self, _: &Hash, bytes: &[u8]) -> eidetic::Result<(Vec<u8>, eidetic::SealedBlobRef)> {
        Ok((
            bytes.iter().map(|b| b ^ 0xaa).collect(),
            eidetic::SealedBlobRef {
                epoch: eidetic::SealEpochId([1; 16]),
                format: "clip-wiring-test".into(),
            },
        ))
    }
    fn unseal(
        &self,
        _: &Hash,
        _: &eidetic::SealedBlobRef,
        bytes: &[u8],
    ) -> eidetic::Result<Vec<u8>> {
        Ok(bytes.iter().map(|b| b ^ 0xaa).collect())
    }
}

#[test]
fn retained_and_portable_schemas_are_available_and_validate_offline() {
    pollster::block_on(async {
        use eidetic::TypedPayload;
        let mut store = MemoryBackend::new();
        let id = retain_clip(
            &mut store,
            None,
            clip(),
            vec![part("body", b"body", &[])],
            vec![],
            &context("test"),
            limits(),
        )
        .await
        .unwrap();
        let retained = load(&mut store, id).await;
        for (schema, bytes) in [
            (
                RetainedClipV1::schema_ref(),
                retained.serialize_to_bytes().unwrap(),
            ),
            (
                PortableClipV1::schema_ref(),
                retained.clip.serialize_to_bytes().unwrap(),
            ),
        ] {
            assert!(
                eidetic::load_schema(&mut store, &mut NoFetcher, schema.0)
                    .await
                    .unwrap()
                    .is_some()
            );
            eidetic::validate_against_schema(&mut store, &mut NoFetcher, schema, &bytes)
                .await
                .unwrap();
            assert!(
                eidetic::validate_against_schema(&mut store, &mut NoFetcher, schema, b"{}")
                    .await
                    .is_err()
            );
        }
    });
}

#[test]
fn sealed_selected_transfer_preserves_existing_receiver_custody() {
    pollster::block_on(async {
        let mut sender = MemoryBackend::new();
        let id = retain_clip(
            &mut sender,
            Some(&WiringSealer),
            clip(),
            vec![part("body", b"selected", &[])],
            vec![part("backing", b"SECRET", &[])],
            &context("sender"),
            limits(),
        )
        .await
        .unwrap();
        assert!(
            export_selected_clip(&mut sender, None, id, &["body".into()], limits())
                .await
                .is_err()
        );
        let package = export_selected_clip(
            &mut sender,
            Some(&WiringSealer),
            id,
            &["body".into()],
            limits(),
        )
        .await
        .unwrap();
        assert_eq!(package.payloads[0].bytes, b"selected");
        let mut receiver = MemoryBackend::new();
        let body_id = eidetic::save_typed_sealed(
            &mut receiver,
            Some(&WiringSealer),
            &eidetic::models::OpaqueBlob(b"selected".to_vec()),
            vec![],
            eidetic::PrivacyClass::TrustedPeersOnly,
            ProvenanceRecord::self_imported("receiver-owned evidence"),
            TrustEnvelope::self_asserted(),
            Timestamp(1),
        )
        .await
        .unwrap();
        let original = eidetic::manifest::load_manifest(&mut receiver, body_id)
            .await
            .unwrap()
            .unwrap();
        import_clip_package(
            &mut receiver,
            None,
            &package,
            &context("receiver"),
            limits(),
        )
        .await
        .unwrap_err();
        import_clip_package(
            &mut receiver,
            Some(&WiringSealer),
            &package,
            &context("receiver"),
            limits(),
        )
        .await
        .unwrap();
        assert_eq!(
            eidetic::manifest::load_manifest(&mut receiver, body_id)
                .await
                .unwrap()
                .unwrap(),
            original
        );
        let stored = receiver
            .get(&format!("blob:{}", Hash::of(b"selected").to_hex()))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(stored, b"selected");
    });
}
