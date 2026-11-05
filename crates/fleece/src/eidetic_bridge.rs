/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Opt-in bridge from Fleece's caller-owned extraction output to Eidetic.
//!
//! The extractor does not establish a page's resolved URL or acquired bytes.
//! A host supplies both as [`CaptureIdentity`]; this module preserves
//! that evidence beside Fleece's canonical DOM text and a W3C Web Annotation
//! target. It never fetches, resolves URLs, or decides that a capture is
//! shareable. The typed save helper writes `LocalOnly`; promotion is a later
//! product action with its own authority and privacy decision.

use crate::{
    CanonicalTextRecordV1, CanonicalTextSelectorProjection, ExtractedDocument, TextAnchor,
};
use eidetic::{
    BlobManifest, BlobSource, Hash, ManifestId, ModerationState, PrivacyClass, ProvenanceOrigin,
    ProvenanceRecord, Result, SchemaRef, Timestamp, TrustEnvelope, TrustLevel, TypedPayload,
};
use serde::{Deserialize, Serialize};

const RFC_5147: &str = crate::RFC5147_CONFORMS_TO;

/// Stable schema identifier carried by a Moot `Shared` contribution that
/// points at a [`FleeceAnnotationRecord`].
/// The document-lanes spelling is retained so existing records keep their identity.
pub const FLEECE_ANNOTATION_SCHEMA_ID: &str = "mere.document-lanes.FleeceAnnotation/v1";

/// Canonical bytes of the schema codicil describing this bridge's typed payload.
const FLEECE_ANNOTATION_SCHEMA_PAYLOAD: &[u8] = br#"{"format":"mere-native","schema_id":"mere.document-lanes.FleeceAnnotation/v1","body":{"version":1,"description":"A caller-identified Fleece extraction with a W3C Web Annotation target over its canonical DOM text.","required":["extraction","target","annotation"],"fields":{"extraction":{"type":"object"},"target":{"type":"object"},"annotation":{"type":"object"}}}}"#;

/// The well-known Eidetic schema reference for [`FleeceAnnotationRecord`].
pub static FLEECE_ANNOTATION_SCHEMA_REF: std::sync::LazyLock<SchemaRef> =
    std::sync::LazyLock::new(|| {
        SchemaRef::from_id(ManifestId::from_hash(Hash::of(
            FLEECE_ANNOTATION_SCHEMA_PAYLOAD,
        )))
    });

/// Capture facts that Fleece cannot infer or verify.
///
/// `capture_hash` is typed caller-supplied evidence for the immutable acquired
/// representation. It is intentionally not inferred from Fleece text: the
/// text is a normalized projection, while the capture may be HTML, WACZ, or a
/// host-specific immutable resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureIdentity {
    /// The host-resolved canonical source URI, never an unresolved DOM href.
    pub canonical_source: String,
    /// Caller-supplied identity of the acquired immutable capture.
    pub capture_hash: Hash,
    /// Versioned caller-owned facts about the acquisition that Fleece cannot
    /// infer from its DOM input. `None` only represents a record serialized
    /// before capture evidence existed and is explicitly non-authoritative for
    /// the absent facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<CaptureEvidenceV1>,
    /// Binds the evidence fields together and into the annotation target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_hash: Option<Hash>,
}

impl CaptureIdentity {
    pub fn new(canonical_source: impl Into<String>, capture_hash: Hash) -> Result<Self> {
        Self::from_evidence(CaptureEvidenceV1::legacy(canonical_source, capture_hash)?)
    }

    /// Preserve acquisition evidence from the host response without importing
    /// its fetch policy, cache, or transport into Fleece.
    pub fn from_resource_response(
        response: &genet_host_api::ResourceResponse,
        captured_at: Timestamp,
        dom_mode: CaptureDomMode,
        raw_manifest: Option<ManifestId>,
        replay_manifest: Option<ManifestId>,
    ) -> Result<Self> {
        Self::from_evidence(CaptureEvidenceV1::from_resource_response(
            response,
            captured_at,
            dom_mode,
            raw_manifest,
            replay_manifest,
        )?)
    }

    /// Construct an identity from all caller-owned acquisition facts.
    pub fn from_evidence(evidence: CaptureEvidenceV1) -> Result<Self> {
        evidence.validate_integrity()?;
        let evidence_hash = evidence.integrity_hash()?;
        Ok(Self {
            canonical_source: evidence.final_source.clone(),
            capture_hash: evidence.capture_hash,
            evidence: Some(evidence),
            evidence_hash: Some(evidence_hash),
        })
    }

    fn validate_integrity(&self) -> Result<()> {
        validate_canonical_source(&self.canonical_source)?;
        match (&self.evidence, self.evidence_hash) {
            (None, None) => Ok(()),
            (Some(evidence), Some(evidence_hash)) => {
                evidence.validate_integrity()?;
                if evidence.integrity_hash()? != evidence_hash
                    || evidence.final_source != self.canonical_source
                    || evidence.capture_hash != self.capture_hash
                {
                    return Err(eidetic::Error::new(
                        "capture identity does not bind its retained evidence",
                    ));
                }
                Ok(())
            },
            _ => Err(eidetic::Error::new(
                "capture evidence and its integrity hash must be retained together",
            )),
        }
    }
}

/// Which DOM representation Fleece received from its caller.
///
/// This records an extraction input, not a policy for producing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureDomMode {
    /// Fleece received a DOM made directly from the acquired source bytes.
    Source,
    /// Fleece received a DOM after a caller-owned rendering or scripting pass.
    Rendered,
    /// A caller supplied a DOM whose construction is outside this bridge.
    CallerSupplied,
    /// Legacy `CaptureIdentity::new` data has no DOM-mode observation.
    Unspecified,
}

/// Versioned caller-owned facts about one acquired document representation.
///
/// The optional manifest identities name durable raw and replay material when
/// a caller has saved it. They never direct fetching, replay, or cache policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureEvidenceV1 {
    pub version: u8,
    pub final_source: String,
    pub capture_hash: Hash,
    pub captured_at: Timestamp,
    pub response_content_type: Option<String>,
    pub dom_mode: CaptureDomMode,
    pub raw_manifest: Option<ManifestId>,
    pub replay_manifest: Option<ManifestId>,
}

/// The durable subset of one Fleece extraction needed to reopen an annotation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleeceExtractionRecord {
    /// Caller-owned resolved source and capture evidence.
    pub capture: CaptureIdentity,
    /// Eidetic's BLAKE3 hash of Fleece's preserved canonical text, checked
    /// before an annotation is accepted.
    ///
    /// This is Eidetic's BLAKE3 content hash. It is deliberately distinct from
    /// Fleece's SHA-256 `canonical_text_iri` in [`Self::canonical_text_record`].
    pub canonical_text_hash: Hash,
    /// Fleece's stable preservation contract, including the canonical-text
    /// resource identity, profile, normalization, language/direction evidence,
    /// and reader anchors.
    pub canonical_text_record: CanonicalTextRecordV1,
}

impl FleeceExtractionRecord {
    /// Preserve the Fleece result together with facts Fleece intentionally does
    /// not own. The Fleece preservation record supplies the extraction profile,
    /// normalization, and canonical-text resource identity.
    pub fn from_fleece(capture: CaptureIdentity, document: &ExtractedDocument) -> Result<Self> {
        let canonical_text_record = CanonicalTextRecordV1::from_document(document);
        canonical_text_record.validate().map_err(|error| {
            eidetic::Error::new(format!("invalid Fleece extraction record: {error}"))
        })?;
        let record = Self {
            capture,
            canonical_text_hash: Hash::of(canonical_text_record.canonical_text.as_bytes()),
            canonical_text_record,
        };
        record.validate_integrity()?;
        Ok(record)
    }

    pub fn validate_integrity(&self) -> Result<()> {
        self.capture.validate_integrity()?;
        if self.canonical_text_record.canonical_text.is_empty() {
            return Err(eidetic::Error::new(
                "canonical Fleece text must not be empty",
            ));
        }
        self.canonical_text_record.validate().map_err(|error| {
            eidetic::Error::new(format!(
                "invalid preserved Fleece extraction record: {error}"
            ))
        })?;
        let actual = Hash::of(self.canonical_text_record.canonical_text.as_bytes());
        if actual != self.canonical_text_hash {
            return Err(eidetic::Error::new(format!(
                "canonical Fleece text hash mismatch: expected {}, got {}",
                self.canonical_text_hash, actual
            )));
        }
        Ok(())
    }
}

fn validate_canonical_source(source: &str) -> Result<()> {
    url::Url::parse(source).map(|_| ()).map_err(|error| {
        eidetic::Error::new(format!("canonical source must be an absolute IRI: {error}"))
    })
}

/// W3C Web Annotation's `TextPositionSelector` projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPositionSelector {
    #[serde(rename = "type")]
    pub selector_type: String,
    pub start: u64,
    pub end: u64,
}

/// W3C Web Annotation's RFC 5147 character-range selector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentSelector {
    #[serde(rename = "type")]
    pub selector_type: String,
    pub value: String,
    #[serde(rename = "conformsTo")]
    pub conforms_to: String,
}

/// W3C Web Annotation's `TextQuoteSelector` projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextQuoteSelector {
    #[serde(rename = "type")]
    pub selector_type: String,
    pub exact: String,
    pub prefix: String,
    pub suffix: String,
}

/// A W3C `SpecificResource` target with Fleece's sibling selectors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebAnnotationTarget {
    #[serde(rename = "type")]
    pub target_type: String,
    /// The immutable canonical text resource, so Fleece's code-point offsets
    /// have one unambiguous source stream.
    pub source: ExternalWebResource,
    /// The acquired page that supplied the canonical text resource.
    pub scope: String,
    pub selector: Vec<serde_json::Value>,
    /// Eidetic extension: capture identity is distinct from the source URI.
    #[serde(rename = "captureHash")]
    pub capture_hash: Hash,
    /// Eidetic extension: binds versioned caller capture evidence when the
    /// record retained it. Absent only for legacy records with no such facts.
    #[serde(
        rename = "captureEvidenceHash",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub capture_evidence_hash: Option<Hash>,
    /// Eidetic extension: declares the exact text the selectors are measured on.
    #[serde(rename = "canonicalTextHash")]
    pub canonical_text_hash: Hash,
}

/// A W3C External Web Resource describing the selector source stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalWebResource {
    #[serde(rename = "id")]
    pub id: String,
    pub format: String,
}

impl WebAnnotationTarget {
    fn from_projection(
        extraction: &FleeceExtractionRecord,
        projection: CanonicalTextSelectorProjection,
    ) -> Result<Self> {
        if !projection.resolves_against(&extraction.canonical_text_record.canonical_text) {
            return Err(eidetic::Error::new(
                "Fleece selector projection does not resolve against canonical text",
            ));
        }
        if projection.resource_iri != extraction.canonical_text_record.canonical_text_iri {
            return Err(eidetic::Error::new(
                "Fleece selector projection names a different canonical-text resource",
            ));
        }
        let fragment = FragmentSelector {
            selector_type: "FragmentSelector".to_owned(),
            value: projection.fragment.value(),
            conforms_to: RFC_5147.to_owned(),
        };
        let position = TextPositionSelector {
            selector_type: "TextPositionSelector".to_owned(),
            start: projection.position.start,
            end: projection.position.end,
        };
        let quote = TextQuoteSelector {
            selector_type: "TextQuoteSelector".to_owned(),
            exact: projection.quote.exact,
            prefix: projection.quote.prefix,
            suffix: projection.quote.suffix,
        };
        Ok(Self {
            target_type: "SpecificResource".to_owned(),
            source: ExternalWebResource {
                id: projection.resource_iri,
                format: extraction.canonical_text_record.media_type.clone(),
            },
            scope: extraction.capture.canonical_source.clone(),
            selector: vec![
                serde_json::to_value(position).map_err(|error| {
                    eidetic::Error::new(format!("position selector JSON: {error}"))
                })?,
                serde_json::to_value(quote).map_err(|error| {
                    eidetic::Error::new(format!("quote selector JSON: {error}"))
                })?,
                serde_json::to_value(fragment).map_err(|error| {
                    eidetic::Error::new(format!("fragment selector JSON: {error}"))
                })?,
            ],
            capture_hash: extraction.capture.capture_hash,
            capture_evidence_hash: extraction.capture.evidence_hash,
            canonical_text_hash: extraction.canonical_text_hash,
        })
    }
}

/// A complete JSON-LD Web Annotation envelope, including the capture bindings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebAnnotationEnvelope {
    #[serde(rename = "@context")]
    pub context: Vec<serde_json::Value>,
    pub id: String,
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub target: WebAnnotationTarget,
}

impl WebAnnotationEnvelope {
    fn from_target(target: WebAnnotationTarget) -> Self {
        let id = format!(
            "urn:eidetic:annotation:{}:{}-{}",
            target.capture_hash.to_hex(),
            target_position(&target).0,
            target_position(&target).1,
        );
        Self {
            context: vec![
                serde_json::Value::String("http://www.w3.org/ns/anno.jsonld".to_owned()),
                serde_json::json!({
                    "eidetic": "https://merely-made.org/ns/eidetic#",
                    "captureHash": "eidetic:captureHash",
                    "captureEvidenceHash": "eidetic:captureEvidenceHash",
                    "canonicalTextHash": "eidetic:canonicalTextHash"
                }),
            ],
            id,
            annotation_type: "Annotation".to_owned(),
            target,
        }
    }
}

/// The typed Eidetic payload: extraction evidence, W3C target, and its exact
/// JSON-LD envelope travel together so reopening can validate all three.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FleeceAnnotationRecord {
    pub extraction: FleeceExtractionRecord,
    pub target: WebAnnotationTarget,
    pub annotation: WebAnnotationEnvelope,
}

impl TypedPayload for FleeceAnnotationRecord {
    fn schema_ref() -> SchemaRef {
        *FLEECE_ANNOTATION_SCHEMA_REF
    }
}

/// Seed the bridge schema codicil. Idempotent, like Eidetic's browsing schema.
pub async fn bootstrap_fleece_annotation_schema(store: &mut dyn eidetic::Store) -> Result<()> {
    let id = FLEECE_ANNOTATION_SCHEMA_REF.0;
    if eidetic::manifest::load_manifest(store, id).await?.is_some() {
        return Ok(());
    }
    let content_hash = Hash::of(FLEECE_ANNOTATION_SCHEMA_PAYLOAD);
    let local_key = format!("blob:{}", content_hash.to_hex());
    store
        .put(&local_key, FLEECE_ANNOTATION_SCHEMA_PAYLOAD)
        .await?;
    eidetic::manifest::save_manifest(
        store,
        &BlobManifest {
            id,
            schema: *eidetic::META_SCHEMA_REF,
            content_hash,
            byte_size: FLEECE_ANNOTATION_SCHEMA_PAYLOAD.len() as u64,
            created_at: Timestamp::ZERO,
            last_accessed: None,
            sources: vec![BlobSource::Local { key: local_key }],
            privacy: PrivacyClass::PublicPortable,
            provenance: ProvenanceRecord {
                origin: ProvenanceOrigin::Generated,
                upstream: Vec::new(),
                tooling: Some(format!("mere-document-lanes/{}", env!("CARGO_PKG_VERSION"))),
                generated_at: Timestamp::ZERO,
            },
            trust: TrustEnvelope {
                level: TrustLevel::CheckpointAccepted,
                signatures: Vec::new(),
                moderation_state: ModerationState::Accepted,
            },
            schema_metadata: serde_json::Value::Null,
            manifest_version: BlobManifest::CURRENT_VERSION,
        },
    )
    .await
}

/// Save an internally validated annotation record as an Eidetic `LocalOnly`
/// typed payload. Broader publication must be explicit at a higher layer.
pub async fn save_fleece_annotation(
    store: &mut dyn eidetic::Store,
    record: &FleeceAnnotationRecord,
    now_ms: u64,
) -> Result<ManifestId> {
    record.validate_integrity()?;
    let id = eidetic::save_typed(
        store,
        record,
        Vec::new(),
        PrivacyClass::LocalOnly,
        ProvenanceRecord {
            origin: ProvenanceOrigin::Generated,
            upstream: Vec::new(),
            tooling: Some(format!(
                "mere-document-lanes-fleece/{}",
                env!("CARGO_PKG_VERSION")
            )),
            generated_at: Timestamp(now_ms),
        },
        TrustEnvelope {
            level: TrustLevel::SelfAsserted,
            signatures: Vec::new(),
            moderation_state: ModerationState::Unreviewed,
        },
        Timestamp(now_ms),
    )
    .await?;
    eidetic::ResourceCaptureStore::new(&*store)
        .record(
            &record.extraction.capture.canonical_source,
            record.extraction.capture.capture_hash,
            eidetic::CaptureContent::Acquired,
            now_ms,
        )
        .await?;
    Ok(id)
}

/// Load a typed annotation record and reject tampered text, selector, target,
/// or JSON-LD envelope facts before returning it to the caller.
pub async fn load_fleece_annotation(
    store: &mut dyn eidetic::Store,
    id: ManifestId,
) -> Result<Option<FleeceAnnotationRecord>> {
    let mut fetcher = eidetic::NoFetcher;
    let Some(record) =
        eidetic::load_typed::<FleeceAnnotationRecord>(store, &mut fetcher, id).await?
    else {
        return Ok(None);
    };
    record.validate_integrity()?;
    Ok(Some(record))
}

fn anchor_from_target(target: &WebAnnotationTarget) -> Result<TextAnchor> {
    if target.selector.len() != 3 {
        return Err(eidetic::Error::new(
            "annotation target must retain fragment, position, and quote selectors",
        ));
    }
    let position: TextPositionSelector = serde_json::from_value(target.selector[0].clone())
        .map_err(|error| eidetic::Error::new(format!("position selector decode: {error}")))?;
    let quote: TextQuoteSelector = serde_json::from_value(target.selector[1].clone())
        .map_err(|error| eidetic::Error::new(format!("quote selector decode: {error}")))?;
    let fragment: FragmentSelector = serde_json::from_value(target.selector[2].clone())
        .map_err(|error| eidetic::Error::new(format!("fragment selector decode: {error}")))?;
    if fragment.selector_type != "FragmentSelector"
        || fragment.conforms_to != RFC_5147
        || position.selector_type != "TextPositionSelector"
        || quote.selector_type != "TextQuoteSelector"
    {
        return Err(eidetic::Error::new(
            "annotation target has unsupported selector types",
        ));
    }
    let fragment = crate::FragmentSelector::parse(&fragment.value)
        .ok_or_else(|| eidetic::Error::new("fragment selector must use an RFC 5147 char range"))?;
    if fragment.start != position.start || fragment.end != position.end {
        return Err(eidetic::Error::new(
            "fragment and text-position selectors disagree",
        ));
    }
    Ok(TextAnchor {
        position: crate::TextPositionSelector {
            start: position.start,
            end: position.end,
        },
        quote: crate::TextQuoteSelector {
            exact: quote.exact,
            prefix: quote.prefix,
            suffix: quote.suffix,
        },
    })
}

fn target_position(target: &WebAnnotationTarget) -> (u64, u64) {
    target
        .selector
        .first()
        .and_then(|value| serde_json::from_value::<TextPositionSelector>(value.clone()).ok())
        .map(|position| (position.start, position.end))
        .unwrap_or((0, 0))
}

#[cfg(test)]
#[path = "eidetic_bridge/tests.rs"]
mod tests;

#[path = "eidetic_bridge/evidence.rs"]
mod evidence;

#[path = "eidetic_bridge/annotation.rs"]
mod annotation;
