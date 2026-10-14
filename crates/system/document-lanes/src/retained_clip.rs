// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Shared retained-clip presentation. The caller admits bytes and owns playback.
use eidetic::{Hash, ManifestId, clip_profile::PortableClipV1};
pub use fleece::portable_clip::ClipLimits;
use inker::{Block, EngineDocument, InlineSpan};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const ENGINE_ID: &str = "fleece.retained-clip";
#[cfg(not(target_arch = "wasm32"))]
const STORE_DIR: &str = "retained-clips";
pub fn address(id: ManifestId) -> String {
    format!("mere://clip/{}", id.0.to_hex())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipRequest {
    Read(ManifestId),
    Play(ManifestId),
    Stop(ManifestId),
    Export(ManifestId),
}

pub fn request(address: &str) -> Result<Option<ClipRequest>, String> {
    let Ok(url) = url::Url::parse(address) else {
        return Ok(None);
    };
    if url.scheme() != "mere" || url.host_str() != Some("clip") {
        return Ok(None);
    }
    if url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("Invalid retained clip address".into());
    }
    let parts: Vec<_> = url
        .path_segments()
        .ok_or("Invalid retained clip address")?
        .collect();
    let Some(hex) = parts.first().filter(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) else {
        return Err("Invalid retained clip identity".into());
    };
    let id = ManifestId::from_hash(Hash::parse(hex).map_err(|e| e.to_string())?);
    match parts.as_slice() {
        [_] => Ok(Some(ClipRequest::Read(id))),
        [_, "play"] => Ok(Some(ClipRequest::Play(id))),
        [_, "stop"] => Ok(Some(ClipRequest::Stop(id))),
        [_, "export"] => Ok(Some(ClipRequest::Export(id))),
        _ => Err("Unsupported retained clip action".into()),
    }
}

pub struct ClipView {
    pub id: ManifestId,
    pub clip: PortableClipV1,
    pub body: ClipBody,
}
pub enum ClipBody {
    Text(String),
    Voice(Vec<u8>),
}

/// A selected-body copy, validated by Fleece from this caller's store. Custody
/// manifests, seal material and private backing are never serialized into it.
pub struct ClipExport {
    pub bytes: Vec<u8>,
    pub filename: String,
}
pub const PACKAGE_MEDIA_TYPE: &str = "application/vnd.mere.clip-package+json";

pub async fn export_body(
    store: &mut dyn eidetic::Store,
    sealer: Option<&dyn eidetic::seal::PayloadSealer>,
    id: ManifestId,
    policy: ClipLimits,
) -> Result<ClipExport, String> {
    let wire_limit = policy
        .max_bytes
        .checked_mul(16)
        .and_then(|n| n.checked_add(policy.max_record_bytes as u64))
        .ok_or("Clip wire budget overflow")?;
    let package =
        fleece::portable_clip::export_selected_clip(store, sealer, id, &["body".into()], policy)
            .await
            .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&package).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > wire_limit {
        return Err("Clip package exceeds its configured wire budget".into());
    }
    Ok(ClipExport {
        bytes,
        filename: format!("selected-clip-{}.mereclip", id.0.to_hex()),
    })
}

pub async fn read_store(
    store: &mut dyn eidetic::Store,
    sealer: Option<&dyn eidetic::seal::PayloadSealer>,
    id: ManifestId,
    policy: ClipLimits,
) -> Result<ClipView, String> {
    let package =
        fleece::portable_clip::export_selected_clip(store, sealer, id, &["body".into()], policy)
            .await
            .map_err(|e| e.to_string())?;
    if package.clip.parts.len() != 1 {
        return Err("This clip needs a receiving adapter for its additional parts".into());
    }
    let part = &package.clip.parts[0];
    let bytes = package
        .payloads
        .into_iter()
        .find(|p| p.id == part.manifest)
        .ok_or("Retained clip body is missing")?
        .bytes;
    let body = match part.media_type.as_str() {
        "text/plain" | "text/plain;charset=utf-8" | "text/plain; charset=utf-8" => {
            ClipBody::Text(String::from_utf8(bytes).map_err(|_| "Clip text is not UTF-8")?)
        },
        "audio/wav" | "audio/x-wav"
            if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") =>
        {
            ClipBody::Voice(bytes)
        },
        _ => {
            return Err(format!(
                "This host cannot present clip media {}",
                part.media_type
            ));
        },
    };
    Ok(ClipView {
        id,
        clip: package.clip,
        body,
    })
}
impl ClipView {
    pub fn document(&self) -> EngineDocument {
        let mut blocks = vec![Block::Heading {
            level: 1,
            spans: vec![InlineSpan::Text(self.clip.title.clone())],
        }];
        match &self.body {
            ClipBody::Text(body) => blocks.push(Block::Paragraph {
                spans: vec![InlineSpan::Text(body.clone())],
            }),
            ClipBody::Voice(_) => {
                for (suffix, label) in [("play", "Play voice note"), ("stop", "Stop voice note")] {
                    blocks.push(Block::Paragraph {
                        spans: vec![InlineSpan::Link {
                            url: format!("{}/{suffix}", address(self.id)),
                            title: None,
                            spans: vec![InlineSpan::Text(label.into())],
                            predicate: None,
                        }],
                    });
                }
                blocks.push(Block::MetadataRow {
                    label: "Voice note playback".into(),
                    value: "Ready".into(),
                });
            },
        }
        if self.clip.anchors.iter().any(|a| a.units == "milliseconds") {
            blocks.push(Block::Badge {
                text: "Source recording unavailable in this clip".into(),
            });
        }
        for anchor in &self.clip.anchors {
            blocks.push(Block::MetadataRow {
                label: format!("Source anchor ({})", anchor.units),
                value: anchor.source.clone(),
            });
            if anchor.units == "milliseconds" {
                if let Some(offset) = anchor.selector.get("offset_ms").and_then(|v| v.as_u64()) {
                    blocks.push(Block::MetadataRow {
                        label: "Original note position".into(),
                        value: format!("{}.{:03} seconds", offset / 1000, offset % 1000),
                    });
                }
            } else if anchor.units == "utf8-bytes" {
                if let Some(quote) = anchor.selector.get("exact_quote").and_then(|v| v.as_str()) {
                    blocks.push(Block::MetadataRow {
                        label: "Original selection".into(),
                        value: quote.into(),
                    });
                }
            } else {
                blocks.push(Block::Badge {
                    text: "Source selector preserved; this view does not interpret its format"
                        .into(),
                });
            }
        }
        for tag in &self.clip.tags {
            blocks.push(Block::MetadataRow {
                label: format!("Tag: {}", tag.label),
                value: format!(
                    "{}; {}",
                    tag.owner,
                    tag.asserter.as_deref().unwrap_or("author unknown")
                ),
            });
        }
        if let Some(tags) = self
            .clip
            .source_evidence
            .get("source_tags")
            .and_then(|v| v.as_array())
        {
            let labels: Vec<_> = tags.iter().filter_map(|v| v.as_str()).collect();
            if !labels.is_empty() {
                blocks.push(Block::MetadataRow {
                    label: "Source tags (author unknown)".into(),
                    value: labels.join(", "),
                });
            }
        }
        blocks.push(Block::Paragraph {
            spans: vec![InlineSpan::Link {
                url: format!("{}/export", address(self.id)),
                title: None,
                spans: vec![InlineSpan::Text("Export selected clip".into())],
                predicate: None,
            }],
        });
        EngineDocument {
            address: address(self.id),
            title: Some(self.clip.title.clone()),
            content_type: "application/vnd.mere.clip+json".into(),
            lang: None,
            provenance: inker::DocumentProvenance::for_engine(ENGINE_ID, &address(self.id)),
            trust: inker::DocumentTrustState::Unknown,
            diagnostics: vec![],
            navigation: Default::default(),
            blocks,
        }
    }

    /// A verified, rebuildable decoder cache. Durable authority stays with the
    /// retained bytes; incoming packages cannot choose a filesystem path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn playback_path(&self, root: &Path) -> Result<PathBuf, String> {
        let ClipBody::Voice(bytes) = &self.body else {
            return Err("This clip has no voice-note body".into());
        };
        let folder = root.join(STORE_DIR).join("playback");
        let path = folder.join(format!("{}.wav", Hash::of(bytes).to_hex()));
        if path.exists() {
            let mut held = Vec::new();
            File::open(&path)
                .map_err(|e| e.to_string())?
                .take(bytes.len() as u64 + 1)
                .read_to_end(&mut held)
                .map_err(|e| e.to_string())?;
            if held != *bytes {
                return Err(
                    "Retained audio decoder cache differs from its content identity".into(),
                );
            }
        } else {
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let mut temporary =
                tempfile::NamedTempFile::new_in(&folder).map_err(|e| e.to_string())?;
            temporary.write_all(bytes).map_err(|e| e.to_string())?;
            temporary.as_file().sync_all().map_err(|e| e.to_string())?;
            temporary
                .persist_noclobber(&path)
                .map_err(|e| e.to_string())?;
        }
        Ok(path)
    }
}

pub struct ClipSessionEngine;
impl inker::SessionEngine<netrender::Scene> for ClipSessionEngine {
    fn engine_id(&self) -> &str {
        ENGINE_ID
    }
    fn spawn(
        &self,
        request: &inker::SessionSpawnRequest,
    ) -> Result<Box<dyn inker::DocumentSession<netrender::Scene>>, inker::SessionError> {
        let body = request.body.as_deref().ok_or_else(|| {
            inker::SessionError::Unsupported(
                "A retained clip requires host-resolved local content".into(),
            )
        })?;
        let document: EngineDocument = serde_json::from_str(body)
            .map_err(|e| inker::SessionError::SpawnFailed(e.to_string()))?;
        if document.address != request.address
            || document.provenance.source_kind.as_deref() != Some(ENGINE_ID)
        {
            return Err(inker::SessionError::SpawnFailed(
                "Retained clip presentation identity differs".into(),
            ));
        }
        let doc =
            crate::SmolwebDocument::from_document_with_theme(document, crate::SmolwebTheme::System);
        Ok(Box::new(crate::SmolwebDocumentSession::new(
            doc,
            request.viewport,
        )))
    }
}

#[cfg(test)]
mod tests;
