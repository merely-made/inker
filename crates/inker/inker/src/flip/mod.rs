// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The engine flip.
//!
//! A *flip* re-presents the same page through a different engine with the
//! user's place and session carried across (the compatibility-view charter;
//! `design_docs/verso_docs/technical_architecture/2026-06-10_compatibility_view_charter.md`
//! is the design record). Where inker's multiplexer picks an engine per
//! address, the flip is its **dynamic** counterpart: it swaps engines
//! mid-session, carrying cookies, scroll, and forms from a glass-box donor to
//! a black-box receiver and back.
//!
//! Folded into inker 2026-09-05 from the `verso-tile` crate (itself the
//! 2026-07-09 consolidation of `verso` / `verso-api` / `verso-scry` /
//! `verso-genet`). The crate had one external consumer, `fetch`, for the
//! [`api::Cookie`] type; the flip is inker's concern, not a boundary of its own.
//!
//! * [`api`] — the engine-agnostic contract: [`api::PortableViewState`], the
//!   layer lattice, and the donor / back / receiver traits. Dependency-free so
//!   an external black-box implementor reaches it without engine deps.
//! * [`orchestrator`] — masks the carry to the layers both sides support
//!   (degrade, never block) and runs the one-hop forward/back choreography. A
//!   secondary never implements [`api::FlipDonor`], so flips cannot chain.
//! * [`scry`] — the black-box receiver: a two-phase forward-inject state
//!   machine (cookies + navigate, then restore on load) over the thin
//!   [`scry::ScrySurface`] seam a host implements on its concrete WebView
//!   producer.
//! * [`genet`] (behind the `genet-donor` feature) — the glass-box donor over
//!   genet's scripted DOM plus host-fed runtime and session state.
//!
//! [`api::Cookie`] and [`api::SameSite`] are the flip's portable carry types;
//! the same-named types at the crate root belong to `surface_engine` and
//! describe a live surface's cookie capabilities. They are deliberately not
//! unified here.

pub mod api;
#[cfg(feature = "genet-donor")]
pub mod genet;
pub mod orchestrator;
pub mod scry;
