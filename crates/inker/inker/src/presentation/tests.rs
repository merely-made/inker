// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;

use super::*;
use crate::routing::*;
use crate::surface_engine::{SurfaceEngine, SurfaceError, SurfaceProducer, SurfaceSpawnRequest};

fn full_declaration() -> PresentationDeclaration {
    let mut declaration = PresentationDeclaration::surface("knot.editor")
        .schemes(["knot"])
        .content_types(["text/x-knot"])
        .extensions(["knot", "kn"])
        .mode(SurfaceContractMode::EmbeddedHost)
        .a11y(A11yCapability::Partial)
        .exposing([Operation::Inspect, Operation::Edit, Operation::Apply]);
    declaration.accepts.source_edits = true;
    declaration.accepts.commands = vec!["knot.fold".into()];
    declaration.reaches.producer = Some(Box::default());
    declaration
}

#[test]
fn declaration_round_trips_through_toml() {
    let declaration = full_declaration();
    let text = toml::to_string(&declaration).expect("declaration serializes");
    let parsed: PresentationDeclaration = toml::from_str(&text).expect("declaration parses");

    assert_eq!(parsed, declaration);
}

#[test]
fn manifest_toml_needs_only_an_id_and_how_it_reaches_the_screen() {
    let parsed: PresentationDeclaration = toml::from_str(
        r#"
        id = "playback"
        operations = ["play", "annotate"]

        [presents]
        extensions = ["ogg"]

        [reaches]
        mode = "Headless"
        a11y = "Opaque"
        "#,
    )
    .expect("minimal declaration parses");

    assert_eq!(parsed.presents.extensions, ["ogg"]);
    assert!(parsed.presents.schemes.is_empty());
    assert_eq!(parsed.reaches.mode, SurfaceContractMode::Headless);
    assert_eq!(parsed.accepts, Accepts::default());
    assert_eq!(parsed.operations, [Operation::Play, Operation::Annotate]);
}

#[test]
fn an_unknown_operation_is_refused() {
    let refused = toml::from_str::<PresentationDeclaration>(
        r#"
        id = "x"
        operations = ["delete"]
        [reaches]
        mode = "Headless"
        a11y = "Opaque"
        "#,
    );

    assert!(refused.is_err());
}

/// The default policy's rules as they were written by hand before the
/// declarations existed.
fn handwritten_policy() -> EngineRoutePolicy {
    let composited = SurfaceContractMode::CompositedTexture;
    let scheme = |schemes: &[&str], engine| {
        EngineRouteRule::new(schemes.iter().copied(), engine, composited)
    };
    let content = |types: &[&str], engine| {
        EngineRouteRule::content_type(types.iter().copied(), engine, composited)
    };
    EngineRoutePolicy {
        rules: vec![
            scheme(&["http", "https"], ENGINE_GENET_WEB),
            scheme(&["gemini", "spartan"], ENGINE_NEMATIC_GEMTEXT),
            scheme(&["gopher"], ENGINE_NEMATIC_GOPHER),
            scheme(&["finger"], ENGINE_NEMATIC_FINGER),
            scheme(&["scroll"], ENGINE_NEMATIC_SCROLL),
            scheme(&["misfin"], ENGINE_NEMATIC_MISFIN),
            scheme(&["nex"], ENGINE_NEMATIC_NEX),
            scheme(&["guppy"], ENGINE_NEMATIC_GUPPY),
            scheme(&["titan"], ENGINE_NEMATIC_TITAN),
            scheme(&["file"], ENGINE_NEMATIC_FILE),
            content(
                &["text/markdown", "text/x-markdown"],
                ENGINE_NEMATIC_MARKDOWN,
            ),
            content(&["text/gemini"], ENGINE_NEMATIC_GEMTEXT),
            content(&["text/plain"], ENGINE_NEMATIC_TEXT),
            content(
                &[
                    "application/rss+xml",
                    "application/atom+xml",
                    "application/feed+xml",
                ],
                ENGINE_NEMATIC_FEED,
            ),
            content(
                &["text/x-knot", "application/x-knot"],
                ENGINE_NEMATIC_KNOT_DJOT,
            ),
            content(&["text/html", "application/xhtml+xml"], ENGINE_GENET_WEB),
            content(&["application/gopher-menu"], ENGINE_NEMATIC_GOPHER),
            content(&["text/x-finger"], ENGINE_NEMATIC_FINGER),
            content(&["application/x-nex"], ENGINE_NEMATIC_NEX),
            content(&["application/x-guppy"], ENGINE_NEMATIC_GUPPY),
            content(&["application/x-titan"], ENGINE_NEMATIC_TITAN),
            content(&["message/x-misfin"], ENGINE_NEMATIC_MISFIN),
            content(&["application/feed+json"], ENGINE_NEMATIC_FEED),
        ],
        fallback: EngineRouteRule::new(
            Vec::<&str>::new(),
            ENGINE_EXTERNAL_PROTOCOL,
            SurfaceContractMode::Headless,
        ),
        per_host_overrides: Default::default(),
    }
}

type Targets = BTreeMap<String, (String, SurfaceContractMode)>;

/// Every scheme and content type a policy routes, with where it goes. Fails
/// on a key two rules claim, which would make rule order matter.
fn targets(policy: &EngineRoutePolicy) -> (Targets, Targets) {
    let (mut by_scheme, mut by_content_type) = (Targets::new(), Targets::new());
    for rule in &policy.rules {
        let target = (rule.engine_id.clone(), rule.mode);
        for key in &rule.schemes {
            assert!(
                by_scheme.insert(key.clone(), target.clone()).is_none(),
                "{key}"
            );
        }
        for key in &rule.content_types {
            assert!(
                by_content_type
                    .insert(key.clone(), target.clone())
                    .is_none(),
                "{key}"
            );
        }
    }
    (by_scheme, by_content_type)
}

#[test]
fn default_policy_rules_reproduce_from_declarations() {
    let declared = EngineRoutePolicy::default();
    let handwritten = handwritten_policy();

    assert_eq!(targets(&declared), targets(&handwritten));
    assert_eq!(declared.fallback, handwritten.fallback);
    assert_eq!(declared.per_host_overrides, handwritten.per_host_overrides);
}

#[test]
fn declared_and_handwritten_policies_route_every_request_alike() {
    let declared = EngineRoutePolicy::default();
    let handwritten = handwritten_policy();
    let (by_scheme, by_content_type) = targets(&handwritten);

    let schemes = by_scheme
        .keys()
        .map(String::as_str)
        .chain(["mailto", "ipfs"]);
    let types = by_content_type
        .keys()
        .map(String::as_str)
        .chain(["image/png", "text/markdown; charset=utf-8"]);
    let engines = declared
        .rules
        .iter()
        .map(|rule| rule.engine_id.as_str())
        .chain([ENGINE_EXTERNAL_PROTOCOL]);

    for scheme in schemes {
        for content_type in std::iter::once(None).chain(types.clone().map(Some)) {
            let address = format!("{scheme}://example.test/page");
            let request = EngineRouteRequest {
                workspace_id: WorkspaceRouteId::new("main"),
                view: None,
                node: None,
                address,
                content_type: content_type.map(str::to_string),
                pinned_engine: None,
            };
            assert_eq!(declared.route(&request), handwritten.route(&request));
            for missing in engines.clone() {
                let available = |id: &str| id != missing;
                assert_eq!(
                    declared.route_filtered(&request, available),
                    handwritten.route_filtered(&request, available),
                    "{scheme} {content_type:?} without {missing}"
                );
            }
        }
    }
}

#[test]
fn builtin_readers_declare_no_operations_or_acceptance() {
    for declaration in default_declarations() {
        assert!(declaration.operations.is_empty(), "{}", declaration.id);
        assert_eq!(
            declaration.accepts,
            Accepts::default(),
            "{}",
            declaration.id
        );
        assert!(!declaration.presents_nothing(), "{}", declaration.id);
    }
    assert!(external_protocol_declaration().presents_nothing());
}

struct BareEngine;

impl SurfaceEngine for BareEngine {
    fn engine_id(&self) -> &str {
        "test.bare"
    }

    fn spawn(&self, _: &SurfaceSpawnRequest) -> Result<Box<dyn SurfaceProducer>, SurfaceError> {
        Err(SurfaceError::Unsupported("test".into()))
    }
}

#[test]
fn a_surface_engine_without_its_own_declaration_gets_the_default() {
    let declaration = BareEngine.declaration();

    assert_eq!(declaration, PresentationDeclaration::surface("test.bare"));
    assert_eq!(declaration.reaches.a11y, BareEngine.a11y_capability());
    assert!(declaration.presents_nothing());
}
