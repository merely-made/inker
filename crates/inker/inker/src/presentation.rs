// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The declaration of a presentation, a reader or a surface producer that a
//! node's resource opens into: what it presents, how it reaches the screen,
//! what it accepts back, and which operations it exposes.
//!
//! One serde type for what routing rules, a surface engine's id and
//! accessibility, and a mod manifest's `contributes` entry used to hold
//! apart. Built-ins construct it in code; a manifest carries it as TOML.
//! Trust is not part of it: that stays with the mod's capabilities and the
//! pack's requested scopes. Leaves and clips stay graph facts that an
//! operation acts on, not fields here.

use serde::{Deserialize, Serialize};

use crate::a11y::A11yCapability;
use crate::routing::SurfaceContractMode;
use crate::surface_engine::WebSurfaceCapabilities;

mod builtin;
pub use builtin::{default_declarations, external_protocol_declaration};

/// One presentation's declaration, keyed by the engine id routing hands out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationDeclaration {
    pub id: String,
    #[serde(default)]
    pub presents: Presents,
    pub reaches: Reaches,
    #[serde(default)]
    pub accepts: Accepts,
    #[serde(default)]
    pub operations: Vec<Operation>,
}

/// What a presentation presents: the addresses and content it opens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presents {
    #[serde(default)]
    pub schemes: Vec<String>,
    #[serde(default)]
    pub content_types: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// How a presentation reaches the screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaches {
    pub mode: SurfaceContractMode,
    pub a11y: A11yCapability,
    /// Producer capabilities, for a surface whose backend is known up front.
    /// A backend that varies by instance reports them at run time instead.
    /// Boxed so a declaration stays small beside the other contribution kinds.
    #[serde(default)]
    pub producer: Option<Box<WebSurfaceCapabilities>>,
}

/// What a presentation accepts back from the host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepts {
    /// Takes source-edit proposals (`crates/system/source-edit`).
    #[serde(default)]
    pub source_edits: bool,
    /// Command ids it accepts.
    #[serde(default)]
    pub commands: Vec<String>,
}

/// The operations an adapter can expose on a clip or live entity, each
/// separate and subject to its actual capabilities and grants (composition
/// brief, clip ruling 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Inspect,
    Annotate,
    Reclip,
    Open,
    Play,
    Edit,
    Apply,
}

impl PresentationDeclaration {
    /// A reader: parses content into the document model, whose blocks are the
    /// semantic tree, so accessibility is `Full` unless the reader degrades.
    pub fn reader(id: impl Into<String>) -> Self {
        Self::with_reach(id, A11yCapability::Full)
    }

    /// A surface producer: streams frames, `Opaque` to assistive technology
    /// until it bridges its content.
    pub fn surface(id: impl Into<String>) -> Self {
        Self::with_reach(id, A11yCapability::Opaque)
    }

    fn with_reach(id: impl Into<String>, a11y: A11yCapability) -> Self {
        Self {
            id: id.into(),
            presents: Presents::default(),
            reaches: Reaches {
                mode: SurfaceContractMode::CompositedTexture,
                a11y,
                producer: None,
            },
            accepts: Accepts::default(),
            operations: Vec::new(),
        }
    }

    pub fn schemes(mut self, schemes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.presents.schemes = schemes.into_iter().map(Into::into).collect();
        self
    }

    pub fn content_types(mut self, types: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.presents.content_types = types.into_iter().map(Into::into).collect();
        self
    }

    pub fn extensions(mut self, extensions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.presents.extensions = extensions.into_iter().map(Into::into).collect();
        self
    }

    pub fn mode(mut self, mode: SurfaceContractMode) -> Self {
        self.reaches.mode = mode;
        self
    }

    pub fn a11y(mut self, a11y: A11yCapability) -> Self {
        self.reaches.a11y = a11y;
        self
    }

    pub fn exposing(mut self, operations: impl IntoIterator<Item = Operation>) -> Self {
        self.operations = operations.into_iter().collect();
        self
    }

    /// True when the declaration names nothing it presents: reachable by pin
    /// or override only, like the opt-in surface engines.
    pub fn presents_nothing(&self) -> bool {
        let presents = &self.presents;
        presents.schemes.is_empty()
            && presents.content_types.is_empty()
            && presents.extensions.is_empty()
    }
}

#[cfg(test)]
mod tests;
