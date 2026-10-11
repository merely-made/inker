// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Source-neutral selected-content intake and bounded, locally resolved export.
//! Caller approval remains separate from byte availability. No source URL is
//! fetched and no graph/session authority is manufactured by these helpers.

use eidetic::clip_profile::{
    ClipAnchorV1, ClipPartV1, ClipSourceV1, PortableClipV1, RetainedClipV1,
};
use eidetic::{
    Error, Hash, ManifestId, NoFetcher, PayloadSealer, PrivacyClass, ProvenanceRecord, Result,
    Store, Timestamp, TrustEnvelope, TypedPayload,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Budgets are supplied by the owning application, not silently chosen here.
#[derive(Clone, Copy, Debug)]
pub struct ClipLimits {
    pub max_parts: usize,
    pub max_bytes: u64,
    pub max_record_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct PartInput {
    pub name: String,
    pub role: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub dependencies: Vec<String>,
}

/// Host provenance is evidence, not an admission or disclosure grant.
pub struct CaptureContext {
    pub source: ClipSourceV1,
    pub capture_id: String,
    pub provenance: ProvenanceRecord,
    pub trust: TrustEnvelope,
    pub captured_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipPayloadV1 {
    pub id: ManifestId,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipPackageV1 {
    pub version: u32,
    pub clip: PortableClipV1,
    pub payloads: Vec<ClipPayloadV1>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(message)
}

fn check_record(clip: &PortableClipV1, limits: ClipLimits) -> Result<()> {
    if clip.version != 1
        || clip.clip_id.trim().is_empty()
        || clip.capture_id.trim().is_empty()
        || clip.source.entity.trim().is_empty()
        || clip.parts.is_empty()
        || clip.parts.len() > limits.max_parts
    {
        return Err(invalid("unsupported or invalid clip record"));
    }
    if serde_json::to_vec(clip)
        .map_err(|e| invalid(e.to_string()))?
        .len()
        > limits.max_record_bytes
    {
        return Err(invalid("clip record exceeds budget"));
    }
    let mut names = HashSet::new();
    for part in &clip.parts {
        if part.name.trim().is_empty()
            || part.role.trim().is_empty()
            || part.media_type.trim().is_empty()
            || part.schema.is_some()
            || !names.insert(&part.name)
        {
            return Err(invalid("invalid, duplicate or unsupported typed clip part"));
        }
    }
    for part in &clip.parts {
        if part.dependencies.iter().any(|name| !names.contains(name)) {
            return Err(invalid("clip dependency is outside disclosed inventory"));
        }
    }
    Ok(())
}

/// Retain selected material and available private backing using existing typed
/// storage/sealing. Inputs are checked before writing. A storage failure can
/// leave immutable payloads; no graph admission or atomicity is claimed.
#[allow(clippy::too_many_arguments)]
pub async fn retain_clip(
    store: &mut dyn Store,
    sealer: Option<&dyn PayloadSealer>,
    mut clip: PortableClipV1,
    selected: Vec<PartInput>,
    backing: Vec<PartInput>,
    context: &CaptureContext,
    limits: ClipLimits,
) -> Result<ManifestId> {
    if !clip.parts.is_empty()
        || selected.is_empty()
        || selected
            .len()
            .checked_add(backing.len())
            .is_none_or(|n| n > limits.max_parts)
    {
        return Err(invalid("invalid clip input inventory"));
    }
    let mut bytes = 0u64;
    let mut names = HashSet::new();
    for input in selected.iter().chain(&backing) {
        bytes = bytes
            .checked_add(input.bytes.len() as u64)
            .ok_or_else(|| invalid("byte budget overflow"))?;
        if bytes > limits.max_bytes
            || input.name.trim().is_empty()
            || !names.insert(input.name.clone())
        {
            return Err(invalid("clip input exceeds budget or repeats a part"));
        }
    }
    // A selected part cannot secretly require private backing.
    let selected_names: HashSet<_> = selected.iter().map(|p| p.name.as_str()).collect();
    if selected.iter().any(|p| {
        p.dependencies
            .iter()
            .any(|n| !selected_names.contains(n.as_str()))
    }) {
        return Err(invalid("selected content depends on undisclosed backing"));
    }
    clip.parts = selected.iter().map(input_part).collect();
    check_record(&clip, limits)?;
    let retained = RetainedClipV1 {
        clip,
        private_backing: backing.iter().map(input_part).collect(),
    };
    if serde_json::to_vec(&retained)
        .map_err(|e| invalid(e.to_string()))?
        .len()
        > limits.max_record_bytes
    {
        return Err(invalid("retained record exceeds budget"));
    }
    eidetic::clip_profile::bootstrap_clip_schemas(store).await?;
    for input in selected.into_iter().chain(backing) {
        // OpaqueBlob uses a raw byte serializer and the existing manifest path.
        deposit(
            store,
            sealer,
            &eidetic::models::OpaqueBlob(input.bytes),
            context,
        )
        .await?;
    }
    deposit(store, sealer, &retained, context).await
}

// Reuse identical objects without rewriting another custody/sealing manifest.
// A schema collision or unavailable existing object is a refusal, not repair.
async fn deposit<T: TypedPayload>(
    store: &mut dyn Store,
    sealer: Option<&dyn PayloadSealer>,
    value: &T,
    context: &CaptureContext,
) -> Result<ManifestId> {
    let bytes = value.serialize_to_bytes()?;
    let id = ManifestId::from_hash(Hash::of(&bytes));
    if let Some(manifest) = eidetic::manifest::load_manifest(store, id).await? {
        if manifest.byte_size != bytes.len() as u64 {
            return Err(invalid("existing object size differs"));
        }
    }
    if let Some(existing) =
        eidetic::load_typed_sealed::<T>(store, &mut NoFetcher, sealer, id).await?
    {
        if existing.serialize_to_bytes()? != bytes {
            return Err(invalid("existing object differs"));
        }
        return Ok(id);
    }
    eidetic::save_typed_sealed(
        store,
        sealer,
        value,
        vec![],
        PrivacyClass::LocalOnly,
        context.provenance.clone(),
        context.trust.clone(),
        context.captured_at,
    )
    .await
}

fn input_part(input: &PartInput) -> ClipPartV1 {
    ClipPartV1 {
        name: input.name.clone(),
        role: input.role.clone(),
        media_type: input.media_type.clone(),
        schema: None,
        manifest: ManifestId::from_hash(Hash::of(&input.bytes)),
        dependencies: input.dependencies.clone(),
    }
}

/// Caller-approved selection of declared parts; closure never walks the source
/// graph, external URLs or the retained record's private-backing inventory.
pub async fn export_selected_clip(
    store: &mut dyn Store,
    sealer: Option<&dyn PayloadSealer>,
    retained_id: ManifestId,
    approved_roots: &[String],
    limits: ClipLimits,
) -> Result<ClipPackageV1> {
    let manifest = eidetic::manifest::load_manifest(store, retained_id)
        .await?
        .ok_or_else(|| invalid("retained clip missing"))?;
    if manifest.byte_size > limits.max_record_bytes as u64 {
        return Err(invalid("retained record exceeds budget"));
    }
    let retained =
        eidetic::load_typed_sealed::<RetainedClipV1>(store, &mut NoFetcher, sealer, retained_id)
            .await?
            .ok_or_else(|| invalid("retained clip missing"))?;
    check_record(&retained.clip, limits)?;
    if approved_roots.is_empty() || approved_roots.len() > limits.max_parts {
        return Err(invalid("no approved roots or too many roots"));
    }
    let by_name: HashMap<_, _> = retained
        .clip
        .parts
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();
    let mut pending: Vec<_> = approved_roots.iter().map(String::as_str).collect();
    let mut reached = HashSet::new();
    while let Some(name) = pending.pop() {
        let part = by_name
            .get(name)
            .ok_or_else(|| invalid("approved part is not selected content"))?;
        if reached.insert(name.to_owned()) {
            if reached.len() > limits.max_parts {
                return Err(invalid("part traversal exceeds budget"));
            }
            pending.extend(part.dependencies.iter().map(String::as_str));
        }
    }
    let mut clip = retained.clip;
    clip.parts.retain(|p| reached.contains(p.name.as_str()));
    let mut payloads = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for part in &clip.parts {
        if !seen.insert(part.manifest) {
            continue;
        }
        let manifest = eidetic::manifest::load_manifest(store, part.manifest)
            .await?
            .ok_or_else(|| invalid("required part manifest missing"))?;
        if manifest.id != part.manifest
            || ManifestId::from_hash(manifest.content_hash) != part.manifest
            || manifest.schema != eidetic::models::OpaqueBlob::schema_ref()
        {
            return Err(invalid("required part identity or raw-byte schema differs"));
        }
        if manifest.byte_size > limits.max_bytes.saturating_sub(total) {
            return Err(invalid("export byte budget exceeded"));
        }
        let bytes = eidetic::resolve_sealed_blob(store, &mut NoFetcher, sealer, &manifest).await?;
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("byte budget overflow"))?;
        if bytes.len() as u64 != manifest.byte_size || total > limits.max_bytes {
            return Err(invalid("part byte size or budget mismatch"));
        }
        // Custody manifests, private provenance and seal markers stay local.
        payloads.push(ClipPayloadV1 {
            id: part.manifest,
            bytes,
        });
    }
    Ok(ClipPackageV1 {
        version: 1,
        clip,
        payloads,
    })
}

/// Verify the complete envelope before any writes, then deposit immutable
/// material. The application still admits the logical clip into its graph and
/// chooses custody. Store failures can leave verified immutable payloads.
pub async fn import_clip_package(
    store: &mut dyn Store,
    sealer: Option<&dyn PayloadSealer>,
    package: &ClipPackageV1,
    context: &CaptureContext,
    limits: ClipLimits,
) -> Result<ManifestId> {
    if package.version != 1 || package.payloads.len() > limits.max_parts {
        return Err(invalid("unsupported clip package or payload count"));
    }
    check_record(&package.clip, limits)?;
    let required: HashSet<_> = package.clip.parts.iter().map(|p| p.manifest).collect();
    let mut actual = HashSet::new();
    let mut total = 0u64;
    for payload in &package.payloads {
        let hash = Hash::of(&payload.bytes);
        total = total
            .checked_add(payload.bytes.len() as u64)
            .ok_or_else(|| invalid("byte budget overflow"))?;
        if !actual.insert(payload.id)
            || !required.contains(&payload.id)
            || payload.id != ManifestId::from_hash(hash)
            || total > limits.max_bytes
        {
            return Err(invalid(
                "unselected, duplicate, altered or oversized clip payload",
            ));
        }
    }
    if actual != required {
        return Err(invalid("required clip payload missing"));
    }
    let retained = RetainedClipV1 {
        clip: package.clip.clone(),
        private_backing: vec![],
    };
    if serde_json::to_vec(&retained)
        .map_err(|e| invalid(e.to_string()))?
        .len()
        > limits.max_record_bytes
    {
        return Err(invalid("retained record exceeds budget"));
    }
    eidetic::clip_profile::bootstrap_clip_schemas(store).await?;
    for payload in &package.payloads {
        deposit(
            store,
            sealer,
            &eidetic::models::OpaqueBlob(payload.bytes.clone()),
            context,
        )
        .await?;
    }
    deposit(store, sealer, &retained, context).await
}

/// Lower a native Knot selection export. Native labels are source facts with
/// unknown taggers, not newly asserted personal tags.
pub async fn retain_knot_export(
    store: &mut dyn Store,
    sealer: Option<&dyn PayloadSealer>,
    export: &serde_json::Value,
    source_backing: Option<Vec<u8>>,
    context: &CaptureContext,
    limits: ClipLimits,
) -> Result<ManifestId> {
    if export.get("schema").and_then(|s| s.as_str()) != Some("knot.selected-content/v1") {
        return Err(invalid("unsupported Knot export"));
    }
    let id = text(export, "id")?;
    let body = export
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid("Knot text missing"))?;
    let anchor = export.get("document_source").filter(|v| !v.is_null());
    if let Some(anchor) = anchor {
        let span = anchor
            .get("byte_span")
            .ok_or_else(|| invalid("Knot byte span missing"))?;
        let start = span
            .get("start")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| invalid("Knot start missing"))?;
        let end = span
            .get("end")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| invalid("Knot end missing"))?;
        if end.checked_sub(start) != Some(text(anchor, "exact_quote")?.len() as u64) {
            return Err(invalid("Knot selector length mismatch"));
        }
    }
    if let (Some(anchor), Some(bytes)) = (anchor, source_backing.as_ref()) {
        let expected = text(anchor, "source_hash")?;
        if Hash::of(bytes).to_hex() != expected {
            return Err(invalid("Knot backing hash mismatch"));
        }
        let span = anchor
            .get("byte_span")
            .ok_or_else(|| invalid("Knot byte span missing"))?;
        let start = span
            .get("start")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| invalid("Knot start missing"))?;
        let end = span
            .get("end")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| invalid("Knot end missing"))?;
        let source = std::str::from_utf8(bytes).map_err(|_| invalid("Knot source is not UTF-8"))?;
        let quote = source
            .get(
                usize::try_from(start).map_err(|_| invalid("Knot offset overflow"))?
                    ..usize::try_from(end).map_err(|_| invalid("Knot offset overflow"))?,
            )
            .ok_or_else(|| invalid("Knot selector splits UTF-8 or exceeds source"))?;
        if quote != text(anchor, "exact_quote")? {
            return Err(invalid("Knot source quote mismatch"));
        }
    } else if source_backing.is_some() {
        return Err(invalid("Knot backing requires a bound source anchor"));
    }
    let mut clip = empty_clip(id, text(export, "label")?, context);
    clip.source_evidence = serde_json::json!({"source_tags": export.get("tags").cloned().unwrap_or(serde_json::json!([])), "tag_asserter": null,
        "kind": export.get("kind"), "pack_sources": export.get("pack_sources")});
    if let Some(anchor) = anchor {
        clip.anchors.push(ClipAnchorV1 {
            source: text(anchor, "document_address")?.into(),
            representation: anchor
                .get("source_hash")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            units: "utf8-bytes".into(),
            selector: anchor.clone(),
            evidence: serde_json::Value::Null,
        });
    }
    let backing = source_backing
        .into_iter()
        .map(|bytes| PartInput {
            name: "source-backing".into(),
            role: "source-backing".into(),
            media_type: "text/plain;charset=utf-8".into(),
            bytes,
            dependencies: vec![],
        })
        .collect();
    retain_clip(
        store,
        sealer,
        clip,
        vec![PartInput {
            name: "body".into(),
            role: "selected-body".into(),
            media_type: "text/plain;charset=utf-8".into(),
            bytes: body.as_bytes().to_vec(),
            dependencies: vec![],
        }],
        backing,
        context,
        limits,
    )
    .await
}

/// Lower the native note/body export. A voice body requires supplied bytes;
/// the referenced recording is not a required dependency of that body.
pub async fn retain_redshank_export(
    store: &mut dyn Store,
    sealer: Option<&dyn PayloadSealer>,
    export: &serde_json::Value,
    voice_body: Option<Vec<u8>>,
    context: &CaptureContext,
    limits: ClipLimits,
) -> Result<ManifestId> {
    if export.get("schema").and_then(|s| s.as_str()) != Some("redshank.selected-note/v1") {
        return Err(invalid("unsupported Redshank export"));
    }
    let note = export
        .get("annotation")
        .ok_or_else(|| invalid("Redshank annotation missing"))?;
    if note.get("privacy").and_then(|v| v.as_str()) != Some("shareable") {
        return Err(invalid("Redshank note is private"));
    }
    let body = note
        .get("body")
        .ok_or_else(|| invalid("Redshank body missing"))?;
    let (bytes, media_type) = match text(body, "kind")? {
        "text" if voice_body.is_none() => (
            body.get("plain_text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| invalid("Redshank text missing"))?
                .as_bytes()
                .to_vec(),
            "text/plain;charset=utf-8".into(),
        ),
        "audio" => (
            voice_body.ok_or_else(|| invalid("voice body bytes missing"))?,
            text(body, "media_type")?.to_owned(),
        ),
        _ => return Err(invalid("unsupported or mismatched note body")),
    };
    if text(body, "kind")? == "audio" && bytes.is_empty() {
        return Err(invalid("voice body bytes are empty"));
    }
    let mut clip = empty_clip(text(note, "id")?, "Redshank note", context);
    let target = note
        .get("target")
        .ok_or_else(|| invalid("Redshank target missing"))?;
    clip.anchors.push(ClipAnchorV1 { source: text(export, "source")?.into(), representation: None,
        units: "milliseconds".into(), selector: target.clone(), evidence: serde_json::json!({"body": body, "source_revision": null, "historical_author": null}) });
    retain_clip(
        store,
        sealer,
        clip,
        vec![PartInput {
            name: "body".into(),
            role: "note-body".into(),
            media_type,
            bytes,
            dependencies: vec![],
        }],
        vec![],
        context,
        limits,
    )
    .await
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid(format!("missing or empty {key}")))
}

fn empty_clip(id: &str, title: &str, context: &CaptureContext) -> PortableClipV1 {
    PortableClipV1 {
        version: 1,
        clip_id: id.into(),
        capture_id: context.capture_id.clone(),
        title: title.into(),
        source: context.source.clone(),
        source_evidence: serde_json::Value::Null,
        parts: vec![],
        anchors: vec![],
        tags: vec![],
    }
}
