// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The built-in readers, declared in code. `EngineRoutePolicy::default`
//! builds its rules from these.

use super::PresentationDeclaration;
use crate::a11y::A11yCapability;
use crate::routing::*;

/// The readers the default route policy opens addresses into. Nematic's
/// readers are document engines (`Full`); genet's web lane is a session
/// engine, which defaults to `Partial`.
pub fn default_declarations() -> Vec<PresentationDeclaration> {
    let reader = PresentationDeclaration::reader;
    vec![
        // HTML by content type routes to genet whatever the scheme, so a local
        // `file://` page and an HTTPS response that turns out to be HTML both
        // land on the web lane.
        reader(ENGINE_GENET_WEB)
            .a11y(A11yCapability::Partial)
            .schemes(["http", "https"])
            .content_types(["text/html", "application/xhtml+xml"]),
        reader(ENGINE_NEMATIC_GEMTEXT)
            .schemes(["gemini", "spartan"])
            .content_types(["text/gemini"]),
        reader(ENGINE_NEMATIC_GOPHER)
            .schemes(["gopher"])
            .content_types(["application/gopher-menu"]),
        reader(ENGINE_NEMATIC_FINGER)
            .schemes(["finger"])
            .content_types(["text/x-finger"]),
        reader(ENGINE_NEMATIC_SCROLL).schemes(["scroll"]),
        reader(ENGINE_NEMATIC_MISFIN)
            .schemes(["misfin"])
            .content_types(["message/x-misfin"]),
        reader(ENGINE_NEMATIC_NEX)
            .schemes(["nex"])
            .content_types(["application/x-nex"]),
        reader(ENGINE_NEMATIC_GUPPY)
            .schemes(["guppy"])
            .content_types(["application/x-guppy"]),
        reader(ENGINE_NEMATIC_TITAN)
            .schemes(["titan"])
            .content_types(["application/x-titan"]),
        reader(ENGINE_NEMATIC_FILE).schemes(["file"]),
        // A fetch that learns one of these content types wins over the scheme
        // rule, so an HTTPS response of `text/markdown` leaves genet.
        reader(ENGINE_NEMATIC_MARKDOWN).content_types(["text/markdown", "text/x-markdown"]),
        reader(ENGINE_NEMATIC_TEXT).content_types(["text/plain"]),
        reader(ENGINE_NEMATIC_FEED).content_types([
            "application/rss+xml",
            "application/atom+xml",
            "application/feed+xml",
            "application/feed+json",
        ]),
        // djot is the default knot grammar (knot design doc §10.5 Phase 5); the
        // CommonMark `nematic.knot` engine stays available by explicit pin for
        // import and compatibility.
        reader(ENGINE_NEMATIC_KNOT_DJOT).content_types(["text/x-knot", "application/x-knot"]),
    ]
}

/// The fallback: an address nothing presents is handed to the host's external
/// protocol handler, which draws nothing and exposes nothing to the tree.
pub fn external_protocol_declaration() -> PresentationDeclaration {
    PresentationDeclaration::surface(ENGINE_EXTERNAL_PROTOCOL).mode(SurfaceContractMode::Headless)
}
