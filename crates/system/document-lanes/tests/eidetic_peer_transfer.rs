/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![cfg(feature = "eidetic-bridge")]

//! Receipt for selected page evidence crossing an authenticated peer lane and
//! surviving receiver-only disk reopen. Commons separately proves LogSync
//! operation convergence; this test owns the document bytes and manifests.

use eidetic::models::OpaqueBlob;
use eidetic::{
    BlobManifest, Hash, ManifestId, ModerationState, NoFetcher, PrivacyClass, ProvenanceOrigin,
    ProvenanceRecord, Store, Timestamp, TrustEnvelope, TrustLevel, TypedPayload,
};
use genet_static_dom::StaticDocument;
use mere_document_lanes::eidetic_bridge::{
    CaptureDomMode, CaptureIdentity, FLEECE_ANNOTATION_SCHEMA_REF, FleeceAnnotationRecord,
    bootstrap_fleece_annotation_schema, load_fleece_annotation,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use transport::{Alpn, P2pandaTransport, Transport, TransportKind};

const RAW_PAGE: &str = "<html><head><title>Peer proof</title></head><body><main><h1>Peer proof</h1><p>The shared passage survives transfer and reopen.</p></main></body></html>";

#[derive(Debug, Serialize, Deserialize)]
struct TransferEnvelope {
    annotation_manifest: BlobManifest,
    annotation_bytes: Vec<u8>,
    capture_manifest: BlobManifest,
    capture_bytes: Vec<u8>,
}

fn provenance(now: Timestamp) -> ProvenanceRecord {
    ProvenanceRecord {
        origin: ProvenanceOrigin::Generated,
        upstream: Vec::new(),
        tooling: Some("mere-document-lanes-peer-receipt/v1".to_owned()),
        generated_at: now,
    }
}

fn trust() -> TrustEnvelope {
    TrustEnvelope {
        level: TrustLevel::SelfAsserted,
        signatures: Vec::new(),
        moderation_state: ModerationState::Unreviewed,
    }
}

fn record_for(raw_manifest: ManifestId) -> FleeceAnnotationRecord {
    let response = genet_host_api::ResourceResponse::new(
        "https://example.test/shared-story",
        RAW_PAGE.as_bytes().to_vec(),
    )
    .with_content_type("text/html; charset=utf-8");
    let document = fleece::extract_document(&StaticDocument::parse(RAW_PAGE));
    let exact = "shared passage";
    let byte_start = document.page.text.find(exact).expect("fixture passage");
    let start = document.page.text[..byte_start].chars().count() as u64;
    let end = start + exact.chars().count() as u64;
    let anchor = fleece::anchor_for_range(
        &document.page.text,
        fleece::TextPositionSelector { start, end },
        document.contract.quote_context,
    )
    .expect("fixture anchor");
    let capture = CaptureIdentity::from_resource_response(
        &response,
        Timestamp(1_700_000_000_000),
        CaptureDomMode::Source,
        Some(raw_manifest),
        None,
    )
    .expect("host capture evidence");
    FleeceAnnotationRecord::from_fleece(capture, &document, &anchor)
        .expect("validated Fleece annotation")
}

fn validate_received_manifest(manifest: &BlobManifest, bytes: &[u8]) {
    assert_eq!(manifest.privacy, PrivacyClass::TrustedPeersOnly);
    assert!(manifest.is_transferable());
    assert_eq!(manifest.byte_size, bytes.len() as u64);
    assert_eq!(manifest.content_hash, Hash::of(bytes));
    assert_eq!(manifest.id, ManifestId::from_hash(manifest.content_hash));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn selected_page_survives_authenticated_peer_transfer_and_receiver_reopen() {
    let now = Timestamp(1_700_000_000_000);
    let mut source = eidetic::MemoryBackend::default();

    let capture_id = eidetic::save_typed(
        &mut source,
        &OpaqueBlob(RAW_PAGE.as_bytes().to_vec()),
        Vec::new(),
        PrivacyClass::TrustedPeersOnly,
        provenance(now),
        trust(),
        now,
    )
    .await
    .expect("save raw capture");
    assert_eq!(capture_id, ManifestId::of_blob(RAW_PAGE.as_bytes()));

    let record = record_for(capture_id);
    let annotation_id = eidetic::save_typed(
        &mut source,
        &record,
        Vec::new(),
        PrivacyClass::TrustedPeersOnly,
        provenance(now),
        trust(),
        now,
    )
    .await
    .expect("save shareable annotation");

    let annotation_manifest = eidetic::manifest::load_manifest(&mut source, annotation_id)
        .await
        .expect("load annotation manifest")
        .expect("annotation manifest exists");
    let capture_manifest = eidetic::manifest::load_manifest(&mut source, capture_id)
        .await
        .expect("load capture manifest")
        .expect("capture manifest exists");
    let annotation_bytes =
        eidetic::manifest::resolve_blob(&mut source, &mut NoFetcher, &annotation_manifest)
            .await
            .expect("resolve annotation bytes");
    let capture_bytes =
        eidetic::manifest::resolve_blob(&mut source, &mut NoFetcher, &capture_manifest)
            .await
            .expect("resolve capture bytes");
    let wire = serde_json::to_vec(&TransferEnvelope {
        annotation_manifest,
        annotation_bytes,
        capture_manifest,
        capture_bytes,
    })
    .expect("serialize transfer envelope");

    let alpn = Alpn::new("mere/document-capture-transfer/v1");
    let alice = P2pandaTransport::bind_seed([0x31; 32], vec![alpn.clone()])
        .await
        .expect("bind alice");
    let bob = P2pandaTransport::bind_seed([0x32; 32], vec![alpn.clone()])
        .await
        .expect("bind bob");
    let alice_id = alice.local_peer_id();
    let bob_id = bob.local_peer_id();
    alice
        .add_peer(bob.endpoint_addr().await.expect("bob endpoint"))
        .await
        .expect("alice adds bob");
    bob.add_peer(alice.endpoint_addr().await.expect("alice endpoint"))
        .await
        .expect("bob adds alice");

    let sender_alpn = alpn.clone();
    let sender = tokio::spawn(async move {
        let mut stream = alice
            .connect(bob_id, sender_alpn)
            .await
            .expect("connect to bob");
        stream.write_all(&wire).await.expect("send envelope");
        stream.flush().await.expect("flush envelope");
        stream.shutdown().await.expect("finish envelope");
    });

    let accepted = bob.accept(alpn.clone()).await.expect("accept from alice");
    assert_eq!(accepted.peer, Some(alice_id));
    assert!(accepted.is_transport_authenticated());
    assert_eq!(accepted.protocol, alpn);
    assert_eq!(accepted.ingress.transport, TransportKind::P2panda);
    let mut received_wire = Vec::new();
    accepted
        .into_stream()
        .read_to_end(&mut received_wire)
        .await
        .expect("receive complete envelope");
    sender.await.expect("sender task");

    let received: TransferEnvelope =
        serde_json::from_slice(&received_wire).expect("decode transfer envelope");
    validate_received_manifest(&received.annotation_manifest, &received.annotation_bytes);
    validate_received_manifest(&received.capture_manifest, &received.capture_bytes);
    assert_eq!(
        received.annotation_manifest.schema,
        *FLEECE_ANNOTATION_SCHEMA_REF
    );
    assert_eq!(
        received.capture_manifest.schema,
        <OpaqueBlob as TypedPayload>::schema_ref()
    );
    let received_record = <FleeceAnnotationRecord as TypedPayload>::deserialize_from_bytes(
        &received.annotation_bytes,
    )
    .expect("decode received annotation");
    received_record
        .validate_integrity()
        .expect("validate received annotation");
    assert_eq!(
        received_record
            .extraction
            .capture
            .evidence
            .as_ref()
            .expect("capture evidence")
            .raw_manifest,
        Some(received.capture_manifest.id)
    );
    assert_eq!(
        received_record.extraction.capture.capture_hash,
        received.capture_manifest.content_hash
    );

    let directory = tempfile::tempdir().expect("temporary receiver store");
    {
        let mut receiver =
            eidetic_fjall::FjallStore::open(directory.path()).expect("open receiver store");
        bootstrap_fleece_annotation_schema(&mut receiver)
            .await
            .expect("seed known schema");
        for (manifest, bytes) in [
            (&received.annotation_manifest, &received.annotation_bytes),
            (&received.capture_manifest, &received.capture_bytes),
        ] {
            receiver
                .put(&format!("blob:{}", manifest.content_hash.to_hex()), bytes)
                .await
                .expect("store received bytes");
            eidetic::manifest::save_manifest(&mut receiver, manifest)
                .await
                .expect("store received manifest");
        }
    }

    let mut reopened =
        eidetic_fjall::FjallStore::open(directory.path()).expect("reopen receiver store");
    let reopened_record = load_fleece_annotation(&mut reopened, received.annotation_manifest.id)
        .await
        .expect("load received annotation")
        .expect("received annotation exists after reopen");
    assert_eq!(reopened_record, record);
    let reopened_capture =
        eidetic::manifest::load_manifest(&mut reopened, received.capture_manifest.id)
            .await
            .expect("load received capture manifest")
            .expect("capture manifest exists after reopen");
    let reopened_bytes =
        eidetic::manifest::resolve_blob(&mut reopened, &mut NoFetcher, &reopened_capture)
            .await
            .expect("resolve received capture after reopen");
    assert_eq!(reopened_bytes, RAW_PAGE.as_bytes());
}
