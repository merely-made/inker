/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl CaptureEvidenceV1 {
    pub const VERSION: u8 = 1;

    /// Derive the source and capture identity from Genet's bounded host
    /// response surface. The caller supplies time, DOM provenance, and any
    /// durable manifest identities because those are outside the fetch seam.
    pub fn from_resource_response(
        response: &genet_host_api::ResourceResponse,
        captured_at: Timestamp,
        dom_mode: CaptureDomMode,
        raw_manifest: Option<ManifestId>,
        replay_manifest: Option<ManifestId>,
    ) -> Result<Self> {
        Self::new(
            response.final_url.clone(),
            Hash::of(&response.bytes),
            captured_at,
            response.content_type.clone(),
            dom_mode,
            raw_manifest,
            replay_manifest,
        )
    }

    pub fn new(
        final_source: impl Into<String>,
        capture_hash: Hash,
        captured_at: Timestamp,
        response_content_type: Option<String>,
        dom_mode: CaptureDomMode,
        raw_manifest: Option<ManifestId>,
        replay_manifest: Option<ManifestId>,
    ) -> Result<Self> {
        let evidence = Self {
            version: Self::VERSION,
            final_source: final_source.into(),
            capture_hash,
            captured_at,
            response_content_type,
            dom_mode,
            raw_manifest,
            replay_manifest,
        };
        evidence.validate_integrity()?;
        Ok(evidence)
    }

    pub(super) fn legacy(canonical_source: impl Into<String>, capture_hash: Hash) -> Result<Self> {
        Self::new(
            canonical_source,
            capture_hash,
            Timestamp::ZERO,
            None,
            CaptureDomMode::Unspecified,
            None,
            None,
        )
    }

    /// Whether every non-derived V1 fact was observed by the caller. Legacy
    /// constructor output retains source and hash only; its timestamp and DOM
    /// mode must not be interpreted as a capture observation.
    pub fn has_authoritative_acquisition_context(&self) -> bool {
        self.captured_at != Timestamp::ZERO && self.dom_mode != CaptureDomMode::Unspecified
    }

    pub(super) fn validate_integrity(&self) -> Result<()> {
        if self.version != Self::VERSION {
            return Err(eidetic::Error::new(format!(
                "unsupported capture evidence version: {}",
                self.version
            )));
        }
        validate_canonical_source(&self.final_source)?;
        if self
            .response_content_type
            .as_deref()
            .is_some_and(|content_type| content_type.trim().is_empty())
        {
            return Err(eidetic::Error::new(
                "capture response content type must not be empty when retained",
            ));
        }
        if self
            .raw_manifest
            .is_some_and(|raw_manifest| raw_manifest != ManifestId::from_hash(self.capture_hash))
        {
            return Err(eidetic::Error::new(
                "raw capture manifest must identify the retained capture bytes",
            ));
        }
        Ok(())
    }

    pub(super) fn integrity_hash(&self) -> Result<Hash> {
        serde_json::to_vec(self)
            .map(|bytes| Hash::of(&bytes))
            .map_err(|error| eidetic::Error::new(format!("capture evidence serialize: {error}")))
    }
}
