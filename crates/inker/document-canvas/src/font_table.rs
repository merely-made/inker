// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The font sidecar: parley's actual shaped faces, out-of-band from the
//! serializable [`DocumentRenderPacket`](crate::DocumentRenderPacket).
//!
//! Each [`GlyphRun`](crate::GlyphRun) carries a [`FontFaceId`] (a plain
//! serializable `u32`); the real face bytes — `parley::FontData`, an
//! `Arc`-backed `Blob` plus a collection index — live here. Layout
//! produces the table; the paint-list producer reads it to populate the
//! `PaintList`'s `fonts()` side-table.
//!
//! ## Why a sidecar and not a packet field
//!
//! `DocumentRenderPacket` is `Serialize + Deserialize + PartialEq`;
//! `parley::FontData` is none of those trivially (it's an `Arc<Blob>`
//! handle, not owned bytes). Hanging it on the packet behind
//! `#[serde(skip)]` would make the packet silently lossy on round-trip
//! (deserialize → empty faces → placeholder-only text) and muddy its
//! `PartialEq`. Instead the handles ride beside the packet
//! ([`crate::LaidOutDocument`]); owned bytes materialize only at the
//! `paint_list_api` boundary. The **`PaintList`**, which *does* carry
//! owned `FontResource` bytes, is the IPC-self-contained form — not the
//! packet.
//!
//! Faces deduplicate by `parley::Blob::id()` (a stable per-allocation id)
//! together with the collection index. A face shared across many runs is
//! stored once; distinct faces in one font collection keep distinct ids.

use std::collections::HashMap;

use parley::FontData;

use crate::types::FontFaceId;

/// The font sidecar produced alongside a [`DocumentRenderPacket`]. Maps
/// each [`FontFaceId`] recorded on a `GlyphRun` to the `parley::FontData`
/// the run was shaped against. Holds `Arc`-cheap handles, not owned bytes.
#[derive(Clone, Debug, Default)]
pub struct FontTable {
    faces: Vec<FontData>,
}

impl FontTable {
    /// The face for `id`, or `None` if the id is out of range (shouldn't
    /// happen for ids minted by the matching [`FontInterner`]).
    pub fn get(&self, id: FontFaceId) -> Option<&FontData> {
        self.faces.get(id.0 as usize)
    }

    /// Number of distinct faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Iterate `(FontFaceId, &FontData)` in id order.
    pub fn iter(&self) -> impl Iterator<Item = (FontFaceId, &FontData)> {
        self.faces
            .iter()
            .enumerate()
            .map(|(i, f)| (FontFaceId(i as u32), f))
    }
}

/// Build-time accumulator that dedups parley faces by `(Blob::id(), index)`
/// and hands back stable [`FontFaceId`]s. Lives in the layouter during a
/// `layout_document` pass; [`into_table`](Self::into_table) seals it into
/// the [`FontTable`] sidecar.
#[derive(Default)]
pub struct FontInterner {
    faces: Vec<FontData>,
    by_face: HashMap<(u64, u32), FontFaceId>,
}

impl FontInterner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern parley's chosen face for a run, returning the id to record
    /// on the `GlyphRun`. Identical faces (same `Blob::id()` and collection
    /// index) collapse to one id. The `FontData` clone is an `Arc` bump,
    /// not a byte copy.
    pub fn intern(&mut self, font: &FontData) -> FontFaceId {
        let face_key = (font.data.id(), font.index);
        if let Some(&id) = self.by_face.get(&face_key) {
            return id;
        }
        let id = FontFaceId(self.faces.len() as u32);
        self.faces.push(font.clone());
        self.by_face.insert(face_key, id);
        id
    }

    /// Seal the accumulated faces into a [`FontTable`].
    pub fn into_table(self) -> FontTable {
        FontTable { faces: self.faces }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_faces_intern_separately_and_same_face_clones_dedup() {
        // The interner handles identity, not font parsing. Synthetic bytes
        // make this regression independent of installed fonts or which
        // platforms package regular/bold faces in a shared TTC allocation.
        let first_face = FontData::new(vec![0_u8; 8].into(), 0);
        let second_face = FontData::new(first_face.data.clone(), 1);
        assert_eq!(first_face.data.id(), second_face.data.id());
        let mut interner = FontInterner::new();
        let first = interner.intern(&first_face);
        let second = interner.intern(&second_face);
        assert_ne!(
            first, second,
            "a collection index identifies a distinct face"
        );
        assert_eq!(interner.intern(&first_face.clone()), first);
        assert_eq!(interner.intern(&second_face.clone()), second);
        let table = interner.into_table();
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(first).unwrap().index, 0);
        assert_eq!(table.get(second).unwrap().index, 1);
        assert_eq!(table.get(first).unwrap().data.id(), first_face.data.id());
        assert_eq!(table.get(second).unwrap().data.id(), first_face.data.id());
    }

    #[test]
    fn empty_table_get_is_none() {
        let table = FontInterner::new().into_table();
        assert!(table.is_empty());
        assert!(table.get(FontFaceId(0)).is_none());
    }
}
