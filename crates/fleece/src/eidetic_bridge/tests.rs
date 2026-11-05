/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use genet_static_dom::StaticDocument;
use muniment::FjallStore;
use oxjsonld::{JsonLdParser, JsonLdRemoteDocument};
use oxrdf::{BlankNode, Dataset, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Term};
use std::collections::{HashMap, HashSet};

// An offline copy of <http://www.w3.org/ns/anno.jsonld>, used under the W3C
// Software and Document License: <https://www.w3.org/copyright/software-license/>.
// The loader below accepts only this URI, so the conformance fixture cannot
// make a network request or silently substitute a host cache.
const W3C_ANNOTATION_CONTEXT: &str = r#"{
 "@context": {
    "oa":      "http://www.w3.org/ns/oa#",
    "dc":      "http://purl.org/dc/elements/1.1/",
    "dcterms": "http://purl.org/dc/terms/",
    "dctypes": "http://purl.org/dc/dcmitype/",
    "foaf":    "http://xmlns.com/foaf/0.1/",
    "rdf":     "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
    "rdfs":    "http://www.w3.org/2000/01/rdf-schema#",
    "skos":    "http://www.w3.org/2004/02/skos/core#",
    "xsd":     "http://www.w3.org/2001/XMLSchema#",
    "iana":    "http://www.iana.org/assignments/relation/",
    "owl":     "http://www.w3.org/2002/07/owl#",
    "as":      "http://www.w3.org/ns/activitystreams#",
    "schema":  "http://schema.org/",

    "id":      {"@type": "@id", "@id": "@id"},
    "type":    {"@type": "@id", "@id": "@type"},

    "Annotation":           "oa:Annotation",
    "Dataset":              "dctypes:Dataset",
    "Image":                "dctypes:StillImage",
    "Video":                "dctypes:MovingImage",
    "Audio":                "dctypes:Sound",
    "Text":                 "dctypes:Text",
    "TextualBody":          "oa:TextualBody",
    "ResourceSelection":    "oa:ResourceSelection",
    "SpecificResource":     "oa:SpecificResource",
    "FragmentSelector":     "oa:FragmentSelector",
    "CssSelector":          "oa:CssSelector",
    "XPathSelector":        "oa:XPathSelector",
    "TextQuoteSelector":    "oa:TextQuoteSelector",
    "TextPositionSelector": "oa:TextPositionSelector",
    "DataPositionSelector": "oa:DataPositionSelector",
    "SvgSelector":          "oa:SvgSelector",
    "RangeSelector":        "oa:RangeSelector",
    "TimeState":            "oa:TimeState",
    "HttpRequestState":     "oa:HttpRequestState",
    "CssStylesheet":        "oa:CssStyle",
    "Choice":               "oa:Choice",
    "Person":               "foaf:Person",
    "Software":             "as:Application",
    "Organization":         "foaf:Organization",
    "AnnotationCollection": "as:OrderedCollection",
    "AnnotationPage":       "as:OrderedCollectionPage",
    "Audience":             "schema:Audience",

    "Motivation":    "oa:Motivation",
    "bookmarking":   "oa:bookmarking",
    "classifying":   "oa:classifying",
    "commenting":    "oa:commenting",
    "describing":    "oa:describing",
    "editing":       "oa:editing",
    "highlighting":  "oa:highlighting",
    "identifying":   "oa:identifying",
    "linking":       "oa:linking",
    "moderating":    "oa:moderating",
    "questioning":   "oa:questioning",
    "replying":      "oa:replying",
    "reviewing":     "oa:reviewing",
    "assessing":     "oa:assessing",
    "tagging":       "oa:tagging",

    "auto":          "oa:autoDirection",
    "ltr":           "oa:ltrDirection",
    "rtl":           "oa:rtlDirection",

    "body":          {"@type": "@id", "@id": "oa:hasBody"},
    "target":        {"@type": "@id", "@id": "oa:hasTarget"},
    "source":        {"@type": "@id", "@id": "oa:hasSource"},
    "selector":      {"@type": "@id", "@id": "oa:hasSelector"},
    "state":         {"@type": "@id", "@id": "oa:hasState"},
    "scope":         {"@type": "@id", "@id": "oa:hasScope"},
    "refinedBy":     {"@type": "@id", "@id": "oa:refinedBy"},
    "startSelector": {"@type": "@id", "@id": "oa:hasStartSelector"},
    "endSelector":   {"@type": "@id", "@id": "oa:hasEndSelector"},
    "renderedVia":   {"@type": "@id", "@id": "oa:renderedVia"},
    "creator":       {"@type": "@id", "@id": "dcterms:creator"},
    "generator":     {"@type": "@id", "@id": "as:generator"},
    "rights":        {"@type": "@id", "@id": "dcterms:rights"},
    "homepage":      {"@type": "@id", "@id": "foaf:homepage"},
    "via":           {"@type": "@id", "@id": "oa:via"},
    "canonical":     {"@type": "@id", "@id": "oa:canonical"},
    "stylesheet":    {"@type": "@id", "@id": "oa:styledBy"},
    "cached":        {"@type": "@id", "@id": "oa:cachedSource"},
    "conformsTo":    {"@type": "@id", "@id": "dcterms:conformsTo"},
    "items":         {"@type": "@id", "@id": "as:items", "@container": "@list"},
    "partOf":        {"@type": "@id", "@id": "as:partOf"},
    "first":         {"@type": "@id", "@id": "as:first"},
    "last":          {"@type": "@id", "@id": "as:last"},
    "next":          {"@type": "@id", "@id": "as:next"},
    "prev":          {"@type": "@id", "@id": "as:prev"},
    "audience":      {"@type": "@id", "@id": "schema:audience"},
    "motivation":    {"@type": "@vocab", "@id": "oa:motivatedBy"},
    "purpose":       {"@type": "@vocab", "@id": "oa:hasPurpose"},
    "textDirection": {"@type": "@vocab", "@id": "oa:textDirection"},

    "accessibility": "schema:accessibilityFeature",
    "bodyValue":     "oa:bodyValue",
    "format":        "dc:format",
    "language":      "dc:language",
    "processingLanguage": "oa:processingLanguage",
    "value":         "rdf:value",
    "exact":         "oa:exact",
    "prefix":        "oa:prefix",
    "suffix":        "oa:suffix",
    "styleClass":    "oa:styleClass",
    "name":          "foaf:name",
    "email":         "foaf:mbox",
    "email_sha1":    "foaf:mbox_sha1sum",
    "nickname":      "foaf:nick",
    "label":         "rdfs:label",

    "created":       {"@id": "dcterms:created", "@type": "xsd:dateTime"},
    "modified":      {"@id": "dcterms:modified", "@type": "xsd:dateTime"},
    "generated":     {"@id": "dcterms:issued", "@type": "xsd:dateTime"},
    "sourceDate":    {"@id": "oa:sourceDate", "@type": "xsd:dateTime"},
    "sourceDateStart": {"@id": "oa:sourceDateStart", "@type": "xsd:dateTime"},
    "sourceDateEnd": {"@id": "oa:sourceDateEnd", "@type": "xsd:dateTime"},

    "start":         {"@id": "oa:start", "@type": "xsd:nonNegativeInteger"},
    "end":           {"@id": "oa:end", "@type": "xsd:nonNegativeInteger"},
    "total":         {"@id": "as:totalItems", "@type": "xsd:nonNegativeInteger"},
    "startIndex":    {"@id": "as:startIndex", "@type": "xsd:nonNegativeInteger"}
  }
}"#;

fn fixture() -> FleeceAnnotationRecord {
    let html = "<html><head><title>Bridge proof</title></head><body><main><h1>Bridge proof</h1><p>The durable passage is preserved with selectors.</p></main></body></html>";
    let document = crate::extract_document(&StaticDocument::parse(html));
    let exact = "durable passage";
    let start = document.page.text.find(exact).expect("fixture text") as u64;
    // Fleece positions are Unicode code points, not UTF-8 byte offsets.
    let start = document.page.text[..start as usize].chars().count() as u64;
    let end = start + exact.chars().count() as u64;
    let anchor = crate::anchor_for_range(
        &document.page.text,
        crate::TextPositionSelector { start, end },
        document.contract.quote_context,
    )
    .expect("fixture anchor");
    let response = genet_host_api::ResourceResponse::new(
        "https://example.test/story",
        html.as_bytes().to_vec(),
    )
    .with_content_type("text/html; charset=utf-8");
    FleeceAnnotationRecord::from_fleece(
        CaptureIdentity::from_resource_response(
            &response,
            Timestamp(1_700_000_000_000),
            CaptureDomMode::Source,
            Some(ManifestId::from_hash(Hash::of(html.as_bytes()))),
            Some(ManifestId::of_blob(b"replay fixture")),
        )
        .expect("capture identity"),
        &document,
        &anchor,
    )
    .expect("Fleece annotation record")
}

#[test]
fn fleece_annotation_round_trips_after_fjall_reopen() {
    pollster::block_on(async {
        let directory = tempfile::tempdir().expect("temporary Fjall directory");
        let record = fixture();
        let id = {
            let mut store = FjallStore::open(directory.path()).expect("open Fjall");
            bootstrap_fleece_annotation_schema(&mut store)
                .await
                .expect("seed schema");
            save_fleece_annotation(&mut store, &record, 1_700_000_000_000)
                .await
                .expect("save typed Fleece annotation")
        };

        let mut reopened = FjallStore::open(directory.path()).expect("reopen Fjall");
        let loaded = load_fleece_annotation(&mut reopened, id)
            .await
            .expect("load typed Fleece annotation")
            .expect("record exists after reopen");
        assert_eq!(loaded, record);
        let captures = eidetic::ResourceCaptureStore::new(&reopened)
            .for_url(&record.extraction.capture.canonical_source)
            .await
            .expect("capture association survives reopen");
        assert_eq!(captures.len(), 1);
        assert_eq!(
            captures[0].content_hash,
            record.extraction.capture.capture_hash
        );
        assert_eq!(captures[0].content, eidetic::CaptureContent::Acquired);
        assert_ne!(
            captures[0].content_hash,
            record.extraction.canonical_text_hash
        );
        assert!(
            eidetic::ResourceCaptureStore::new(&reopened)
                .for_url("https://other.test/story")
                .await
                .unwrap()
                .is_empty()
        );

        let json: serde_json::Value =
            serde_json::from_slice(&loaded.annotation_json_ld().expect("serialize JSON-LD"))
                .expect("parse JSON-LD");
        assert_eq!(json["type"], "Annotation");
        assert_eq!(json["target"]["type"], "SpecificResource");
        assert_eq!(
            json["target"]["source"]["id"],
            loaded.extraction.canonical_text_record.canonical_text_iri
        );
        assert!(
            loaded
                .extraction
                .canonical_text_record
                .canonical_text_iri
                .starts_with("urn:sha256:")
        );
        assert_ne!(
            loaded.extraction.canonical_text_record.canonical_text_iri,
            loaded.extraction.canonical_text_hash.to_string()
        );
        assert_eq!(
            json["target"]["source"]["format"],
            loaded.extraction.canonical_text_record.media_type
        );
        assert_eq!(json["target"]["scope"], "https://example.test/story");
        assert_eq!(
            json["target"]["canonicalTextHash"],
            loaded.extraction.canonical_text_hash.to_string()
        );
        assert_eq!(
            json["target"]["captureHash"],
            loaded.extraction.capture.capture_hash.to_string()
        );
        assert_eq!(
            json["target"]["selector"][0]["type"],
            "TextPositionSelector"
        );
        assert_eq!(
            json["target"]["selector"][2]["value"],
            format!(
                "char={},{}",
                target_position(&loaded.target).0,
                target_position(&loaded.target).1,
            )
        );
        assert_eq!(json["target"]["selector"][1]["type"], "TextQuoteSelector");
        assert_eq!(json["target"]["selector"][2]["type"], "FragmentSelector");
        assert_eq!(json["target"]["selector"][2]["conformsTo"], RFC_5147);
    });
}

#[test]
fn capture_identity_rejects_relative_or_malformed_sources() {
    let capture_hash = Hash::of(b"capture");
    assert!(CaptureIdentity::new("relative/path", capture_hash).is_err());
    assert!(CaptureIdentity::new("not a URL", capture_hash).is_err());
    let legacy = CaptureIdentity::new("gemini://example.test/page", capture_hash)
        .expect("legacy capture identity");
    assert_eq!(legacy.canonical_source, "gemini://example.test/page");
    assert_eq!(legacy.capture_hash, capture_hash);
    assert!(
        !legacy
            .evidence
            .as_ref()
            .expect("legacy constructor delegates to V1 evidence")
            .has_authoritative_acquisition_context()
    );

    let mut record = fixture();
    record.extraction.capture.canonical_source = "relative/path".to_owned();
    assert!(record.validate_integrity().is_err());
}

#[test]
fn capture_evidence_binds_every_retained_fact() {
    let record = fixture();
    assert!(
        record
            .extraction
            .capture
            .evidence
            .as_ref()
            .expect("fixture retains V1 capture evidence")
            .has_authoritative_acquisition_context()
    );
    assert!(
        CaptureEvidenceV1::new(
            "https://example.test/story",
            Hash::of(b"capture"),
            Timestamp(1_700_000_000_000),
            None,
            CaptureDomMode::Source,
            Some(ManifestId::of_blob(b"other raw bytes")),
            None,
        )
        .is_err(),
        "a raw manifest must name the same bytes as the capture hash"
    );

    let mut changed = record.clone();
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .final_source = "https://example.test/changed".to_owned();
    assert!(changed.validate_integrity().is_err());

    let mut changed = record.clone();
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .capture_hash = Hash::of(b"changed capture");
    assert!(changed.validate_integrity().is_err());

    let mut changed = record.clone();
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .captured_at = Timestamp(1_700_000_000_001);
    assert!(changed.validate_integrity().is_err());

    let mut changed = record.clone();
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .response_content_type = Some("application/xhtml+xml".to_owned());
    assert!(changed.validate_integrity().is_err());

    let mut changed = record.clone();
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .dom_mode = CaptureDomMode::Rendered;
    assert!(changed.validate_integrity().is_err());

    let mut changed = record.clone();
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .raw_manifest = Some(ManifestId::of_blob(b"changed raw fixture"));
    assert!(changed.validate_integrity().is_err());

    let mut changed = record;
    changed
        .extraction
        .capture
        .evidence
        .as_mut()
        .expect("fixture evidence")
        .replay_manifest = Some(ManifestId::of_blob(b"changed replay fixture"));
    assert!(changed.validate_integrity().is_err());
}

#[test]
fn record_serialized_before_capture_evidence_remains_readable() {
    let mut json = serde_json::to_value(fixture()).expect("serialize current record");
    let capture = json["extraction"]["capture"]
        .as_object_mut()
        .expect("capture object");
    capture.remove("evidence");
    capture.remove("evidence_hash");
    json["target"]
        .as_object_mut()
        .expect("target object")
        .remove("captureEvidenceHash");
    json["annotation"]["target"]
        .as_object_mut()
        .expect("annotation target object")
        .remove("captureEvidenceHash");

    let legacy: FleeceAnnotationRecord =
        serde_json::from_value(json).expect("decode pre-evidence record");
    assert!(legacy.extraction.capture.evidence.is_none());
    assert!(legacy.extraction.capture.evidence_hash.is_none());
    legacy
        .validate_integrity()
        .expect("pre-evidence record remains valid");
}

#[test]
fn changed_text_or_selector_is_rejected_before_reopen_accepts_it() {
    pollster::block_on(async {
        let mut record = fixture();
        record
            .extraction
            .canonical_text_record
            .canonical_text
            .push('!');
        let mut store = eidetic::MemoryBackend::default();
        let id = eidetic::save_typed(
            &mut store,
            &record,
            Vec::new(),
            PrivacyClass::LocalOnly,
            ProvenanceRecord {
                origin: ProvenanceOrigin::Generated,
                upstream: Vec::new(),
                tooling: None,
                generated_at: Timestamp::ZERO,
            },
            TrustEnvelope {
                level: TrustLevel::SelfAsserted,
                signatures: Vec::new(),
                moderation_state: ModerationState::Unreviewed,
            },
            Timestamp::ZERO,
        )
        .await
        .expect("write deliberately invalid typed payload");
        assert!(load_fleece_annotation(&mut store, id).await.is_err());

        let mut record = fixture();
        record.target.selector[1]["exact"] = serde_json::Value::String("wrong".to_owned());
        let id = eidetic::save_typed(
            &mut store,
            &record,
            Vec::new(),
            PrivacyClass::LocalOnly,
            ProvenanceRecord {
                origin: ProvenanceOrigin::Generated,
                upstream: Vec::new(),
                tooling: None,
                generated_at: Timestamp::ZERO,
            },
            TrustEnvelope {
                level: TrustLevel::SelfAsserted,
                signatures: Vec::new(),
                moderation_state: ModerationState::Unreviewed,
            },
            Timestamp::ZERO,
        )
        .await
        .expect("write deliberately invalid typed payload");
        assert!(load_fleece_annotation(&mut store, id).await.is_err());

        let mut record = fixture();
        record.target.selector[2]["value"] = serde_json::Value::String("char=0,1".to_owned());
        assert!(record.validate_integrity().is_err());

        let mut record = fixture();
        record.extraction.capture.capture_hash = Hash::of(b"changed capture");
        assert!(record.validate_integrity().is_err());
    });
}

#[path = "tests/json_ld.rs"]
mod json_ld;
