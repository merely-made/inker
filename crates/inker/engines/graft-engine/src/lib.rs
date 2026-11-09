// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! # graft-engine
//!
//! `inker::SurfaceEngine` implementation backed by the
//! [`wgpu-graft`](https://github.com/merely-made/wgpu-graft) Servo embedding: an
//! embedded Servo instance renders offscreen and its GL framebuffer is imported
//! into a host `wgpu` texture (Vulkan external memory on Linux, IOSurface/Metal
//! on macOS, ANGLE-D3D11 -> DX12 shared texture on Windows).
//!
//! ## The seam and the Servo producer
//!
//! [`GraftSurface`] is the engine's host seam: a surface the engine drives,
//! built per spawn by a [`GraftProducerFactory`]. The concrete Servo producer
//! lives in wgpu-graft's `grafting-servo` library (process root, `WebView`,
//! delegate, input/cursor/navigation/accessibility translation, frame import),
//! the way scrying's producer lives in `scrying`. With the `servo` feature,
//! the [`servo`] module adapts it onto this seam (`ServoGraftFactory`), so a
//! host writes no Servo code; it supplies a `ServoProducerConfig` and its wgpu
//! device. Without the feature this crate depends on inker only, and the seam
//! remains for tests and other producers.
//!
//! The feature is not forwarded to `grafting-servo/servo` on purpose: a member
//! crate forwarding it would make every `cargo` invocation in the inker/mere
//! workspaces lock and fetch the Servo git tree (whose exact pins also cannot
//! coexist with this workspace's lock). The host enables
//! `grafting-servo/servo` on its own row; the Servo revision stays in
//! wgpu-graft.
//!
//! ## Shape (mirrors scrying-engine)
//!
//! - [`GraftEngine`] — the `inker::SurfaceEngine`, registered as `graft.servo`.
//! - [`GraftProducerFactory`] — host hook that builds a [`GraftSurface`] per
//!   spawn (it owns the wgpu device, the Servo instance, and the interop
//!   adapter — the parts this crate cannot fabricate).
//! - [`GraftProducer`] — adapts a `Box<dyn GraftSurface>` onto
//!   `inker::SurfaceProducer`.
//!
//! See the engine-picker plan
//! (`design_docs/archive_docs/2026-10-28/2026-06-15_engine_picker_and_pluggability_plan.md`,
//! Phase 5) for how this slots into the multi-engine multiplexer alongside
//! `scrying-engine` and `weld-engine`, and for the feature-gating model.

#![doc(html_root_url = "https://docs.rs/graft-engine/0.0.1")]

pub mod engine;
pub mod producer;
#[cfg(feature = "servo")]
pub mod servo;

pub use engine::{GRAFT_SERVO_ENGINE_ID, GraftEngine, GraftProducerFactory};
pub use producer::{GraftFrame, GraftProducer, GraftSurface};
