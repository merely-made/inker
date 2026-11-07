// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The knot editor's retained buffer and readout. Retiring (engine split plan
//! D17): [`KnotEditor`] and [`KnotReadout`] go to Knot's `knot-document`, and
//! the reuse-lexers already live in [`illume`] (feature `markdown`), re-exported
//! here until consumers move.
pub use illume::{MarkdownLexer, full_pack};

pub mod editor;
mod model;

pub use editor::KnotReadout;
pub use model::{EditOutcome, KnotEditor};
