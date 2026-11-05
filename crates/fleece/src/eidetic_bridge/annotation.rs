/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl FleeceAnnotationRecord {
    pub fn from_fleece(
        capture: CaptureIdentity,
        document: &ExtractedDocument,
        anchor: &TextAnchor,
    ) -> Result<Self> {
        let extraction = FleeceExtractionRecord::from_fleece(capture, document)?;
        let target = WebAnnotationTarget::from_projection(
            &extraction,
            document.selector_projection(anchor),
        )?;
        let annotation = WebAnnotationEnvelope::from_target(target.clone());
        let record = Self {
            extraction,
            target,
            annotation,
        };
        record.validate_integrity()?;
        Ok(record)
    }

    /// Validate the retained canonical text, both selector forms, capture
    /// bindings, and the serialized interchange envelope before use.
    pub fn validate_integrity(&self) -> Result<()> {
        self.extraction.validate_integrity()?;
        if self.target.target_type != "SpecificResource"
            || self.target.source.id != self.extraction.canonical_text_record.canonical_text_iri
            || self.target.source.format != self.extraction.canonical_text_record.media_type
            || self.target.scope != self.extraction.capture.canonical_source
            || self.target.capture_hash != self.extraction.capture.capture_hash
            || self.target.capture_evidence_hash != self.extraction.capture.evidence_hash
            || self.target.canonical_text_hash != self.extraction.canonical_text_hash
        {
            return Err(eidetic::Error::new(
                "annotation target does not bind the stored extraction",
            ));
        }
        let anchor = anchor_from_target(&self.target)?;
        let projection =
            crate::CanonicalTextSelectorProjection::from_anchor(&self.target.source.id, &anchor);
        if !projection.resolves_against(&self.extraction.canonical_text_record.canonical_text) {
            return Err(eidetic::Error::new(
                "annotation selectors do not resolve against stored canonical text",
            ));
        }
        let expected = WebAnnotationEnvelope::from_target(self.target.clone());
        if self.annotation != expected {
            return Err(eidetic::Error::new(
                "annotation JSON-LD envelope does not match the stored target",
            ));
        }
        Ok(())
    }

    /// Serialize the complete W3C Web Annotation JSON-LD envelope.
    pub fn annotation_json_ld(&self) -> Result<Vec<u8>> {
        self.validate_integrity()?;
        serde_json::to_vec(&self.annotation)
            .map_err(|error| eidetic::Error::new(format!("annotation JSON-LD serialize: {error}")))
    }
}
