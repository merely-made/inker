// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Fleece gathers content through bounded traversal, clipping and capture.
//! The Genet extractor remains the lightweight document-analysis core. Source,
//! transport, engine and durable-record integrations are explicit features.

pub use fleece_extract::*;

#[cfg(feature = "clip")]
pub mod clip;
#[cfg(all(feature = "crawl", not(target_arch = "wasm32")))]
pub mod crawl;
#[cfg(feature = "eidetic-bridge")]
pub mod eidetic_bridge;
#[cfg(feature = "page-links")]
pub mod page_links;
#[cfg(feature = "portable-clip")]
pub mod portable_clip;
#[cfg(feature = "structured-data")]
pub mod structured_data;
pub mod traversal;

#[cfg(feature = "eidetic-bridge")]
pub use eidetic_bridge::{
    CaptureIdentity, ExternalWebResource, FLEECE_ANNOTATION_SCHEMA_REF, FleeceAnnotationRecord,
    FleeceExtractionRecord, WebAnnotationEnvelope, WebAnnotationTarget,
    bootstrap_fleece_annotation_schema, load_fleece_annotation, save_fleece_annotation,
};
