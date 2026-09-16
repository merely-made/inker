// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! P1 session state over the committed Micron probe pages: fold toggles by
//! pointer and keyboard, in-page reveals, streamed replacement, resize, focus
//! and the zero-transport boundary. Asserted by label, never by raw index,
//! because the diagnostic badge moves indices mid-stream.

use document_canvas::{ColorToken, DocumentStyleSheet, InteractionKind, InteractionRegion, Rect};
use inker::session_engine::{
    DocumentSession, SessionButtonState, SessionEffect, SessionFocusDirection, SessionInput,
    SessionKey, SessionPointerButton,
};
use inker::{Block, Engine, EngineDocument, EngineInput, inline_text};
use netrender::SceneOp;

use super::SmolwebDocumentSession;
use crate::{InPageNavigation, SmolwebDocument, SmolwebTheme};

const NODE: &str = "923706ddc70d389bd3719258c41f6592";
const W: u32 = 614;
const H: u32 = 600;

macro_rules! fixture {
    ($file:literal) => {
        (
            $file,
            include_str!(concat!(
                "../../../../nematic/nematic/tests/fixtures/micron/nomadnet-1.4.2/",
                $file
            )),
        )
    };
}

fn lower(file: &str, source: &str) -> EngineDocument {
    let name = file.rsplit('/').next().unwrap();
    nematic::MicronEngine::new()
        .render(&EngineInput::new(format!("{NODE}:/page/{name}"), source))
        .unwrap()
}

fn load_sized((file, source): (&str, &str), width: u32, height: u32) -> SmolwebDocumentSession {
    let document =
        SmolwebDocument::from_document_with_theme(lower(file, source), SmolwebTheme::Plain);
    let mut session = SmolwebDocumentSession::new(document, (width, height));
    let _ = session.frame(width, height);
    session
}

fn load(page: (&str, &str)) -> SmolwebDocumentSession {
    load_sized(page, W, H)
}

fn label(block: &Block) -> String {
    match block {
        Block::Presented { block, .. } => label(block),
        Block::Heading { spans, .. } | Block::Paragraph { spans } => inline_text(spans),
        Block::Preformatted { text } | Block::Badge { text } => text.clone(),
        other => format!("{other:?}"),
    }
}

fn block(session: &SmolwebDocumentSession, text: &str) -> usize {
    let blocks = &session.doc.document().blocks;
    (0..blocks.len())
        .find(|&index| label(&blocks[index]) == text)
        .unwrap_or_else(|| panic!("no block {text:?}"))
}

fn fold(session: &SmolwebDocumentSession, heading: &str) -> usize {
    let document = session.doc.document();
    document
        .navigation
        .folds
        .iter()
        .position(|fold| label(&document.blocks[fold.heading]) == heading)
        .unwrap_or_else(|| panic!("no fold {heading:?}"))
}

fn is_open(session: &SmolwebDocumentSession, heading: &str) -> bool {
    let navigation = &session.doc.document().navigation;
    session
        .doc
        .folds()
        .is_open(navigation, fold(session, heading))
}

/// Lay out at the session viewport and return the retained packet's
/// top-level bounds for `block`, if it is laid out.
fn bounds(session: &mut SmolwebDocumentSession, block: usize) -> Option<Rect> {
    let (width, height) = session.viewport;
    let _ = session.content_height(width, height);
    let packet = session.doc.packet().unwrap();
    packet
        .top_level_block(block)
        .map(|rendered| rendered.bounds)
}

fn regions(session: &mut SmolwebDocumentSession) -> Vec<InteractionRegion> {
    let (width, height) = session.viewport;
    let _ = session.content_height(width, height);
    session.doc.packet().unwrap().interactions.clone()
}

fn fold_region(session: &mut SmolwebDocumentSession, heading: &str) -> InteractionRegion {
    let fold = fold(session, heading);
    regions(session)
        .into_iter()
        .find(|region| region.kind == InteractionKind::Fold { fold })
        .unwrap_or_else(|| panic!("no fold region {heading:?}"))
}

fn link_region(session: &mut SmolwebDocumentSession, text: &str) -> InteractionRegion {
    regions(session)
        .into_iter()
        .find(|region| {
            region
                .link_semantics
                .as_ref()
                .is_some_and(|semantics| semantics.accessible_label == text)
        })
        .unwrap_or_else(|| panic!("no link region {text:?}"))
}

/// Press the primary button at the region's centre, which must be on screen.
fn click(session: &mut SmolwebDocumentSession, region: InteractionRegion) -> SessionEffect {
    let x = region.bounds.origin.x + region.bounds.size.width * 0.5 - session.doc.scroll_x();
    let y = region.bounds.origin.y + region.bounds.size.height * 0.5 - session.doc.scroll_y();
    let (width, height) = session.viewport;
    assert!(
        (0.0..width as f32).contains(&x) && (0.0..height as f32).contains(&y),
        "click target on screen"
    );
    session
        .input(SessionInput::PointerButton {
            x,
            y,
            button: SessionPointerButton::Primary,
            state: SessionButtonState::Pressed,
            modifiers: Default::default(),
        })
        .effect
}

fn key(session: &mut SmolwebDocumentSession, key: SessionKey) -> SessionEffect {
    session
        .input(SessionInput::Key {
            key,
            state: SessionButtonState::Pressed,
            modifiers: Default::default(),
            repeat: false,
        })
        .effect
}

fn forward(session: &mut SmolwebDocumentSession) -> SessionEffect {
    session
        .input(SessionInput::FocusMove(SessionFocusDirection::Forward))
        .effect
}

fn jump(fragment: &str, block: usize) -> InPageNavigation {
    InPageNavigation {
        fragment: Some(fragment.into()),
        block,
    }
}

#[test]
fn probes_08_07c_pointer_toggles_a_collapsible_heading() {
    let mut session = load(fixture!("navigation/probe-nav-08-toggle-keys.mu"));
    let body = block(
        &session,
        "MARKER ENTER BODY: revealed only when Enter Target is open.",
    );
    let height = session.doc.packet().unwrap().content_bounds.size.height;
    assert!(bounds(&mut session, body).is_none(), "authored closed");

    let region = fold_region(&mut session, "Enter Target");
    assert_eq!(click(&mut session, region.clone()), SessionEffect::Handled);
    assert!(bounds(&mut session, body).is_some(), "probe 08: open");
    assert!(session.doc.packet().unwrap().content_bounds.size.height > height);
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    assert!(
        bounds(&mut session, body).is_none(),
        "probe 08: closed again"
    );

    let mut session = session_07c();
    let body = block(&session, "MARKER B BODY: inside the depth-one fold.");
    let after = block(
        &session,
        "MARKER B AFTER <: left depth two inside a closed depth-one fold.",
    );
    assert!(bounds(&mut session, body).is_none());
    let region = fold_region(&mut session, "Closed One B");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    assert!(
        bounds(&mut session, body).unwrap().origin.y
            < bounds(&mut session, after).unwrap().origin.y,
        "probe 07c: the opened body sits above the line after <"
    );
}

fn session_07c() -> SmolwebDocumentSession {
    load(fixture!("navigation/probe-nav-07c-section-exit-fold.mu"))
}

#[test]
fn probe_08_enter_and_space_toggle_the_focused_heading() {
    let mut session = load(fixture!("navigation/probe-nav-08-toggle-keys.mu"));
    let before = session.doc.packet().cloned();
    assert_eq!(
        key(&mut session, SessionKey::Enter),
        SessionEffect::Ignored,
        "Enter with nothing focused toggles nothing"
    );
    assert!(!is_open(&session, "Enter Target"));

    assert_eq!(forward(&mut session), SessionEffect::Handled);
    let _ = session.frame(W, H);
    assert_eq!(
        session.doc.packet().cloned(),
        before,
        "moving focus changes no geometry"
    );
    for open in [true, false] {
        assert_eq!(key(&mut session, SessionKey::Enter), SessionEffect::Handled);
        assert_eq!(is_open(&session, "Enter Target"), open, "probe 08: Enter");
    }

    assert_eq!(forward(&mut session), SessionEffect::Handled);
    for open in [true, false] {
        assert_eq!(key(&mut session, SessionKey::Space), SessionEffect::Handled);
        assert_eq!(is_open(&session, "Space Target"), open, "probe 08: Space");
    }
    assert!(
        !is_open(&session, "Enter Target"),
        "only the focused heading"
    );
}

#[test]
fn probes_05_17_links_into_closed_sections_open_ancestors_and_scroll() {
    for (page, link, target, fragment, ancestors) in [
        (
            fixture!("navigation/probe-nav-05-closed-target.mu"),
            "jump to the hidden heading",
            "Hidden Target",
            "hidden-target",
            &["Closed Outer"][..],
        ),
        (
            fixture!("navigation/probe-nav-05-closed-target.mu"),
            "jump to the hidden explicit anchor",
            "MARKER HIDDEN EXPLICIT: line bound inside the closed section.",
            "hidden-explicit",
            &["Closed Outer"],
        ),
        (
            fixture!("navigation/probe-nav-17-nested-closed-target.mu"),
            "jump into two closed sections",
            "MARKER NESTED DEEP TARGET: bound inside both closed folds.",
            "nested-deep",
            &["Outer Closed", "Inner Closed"],
        ),
    ] {
        let mut session = load(page);
        let target_block = block(&session, target);
        assert!(
            bounds(&mut session, target_block).is_none(),
            "{link}: hidden before"
        );

        let region = link_region(&mut session, link);
        assert_eq!(click(&mut session, region), SessionEffect::Handled);
        for ancestor in ancestors {
            assert!(is_open(&session, ancestor), "{link}: opened {ancestor}");
        }
        let top = bounds(&mut session, target_block)
            .expect("revealed")
            .origin
            .y;
        assert_eq!(
            session.doc.scroll_y(),
            top,
            "{link}: target at the viewport top"
        );
        assert_eq!(
            session.take_in_page_navigations(),
            [jump(fragment, target_block)]
        );
    }
}

#[test]
fn probes_03_04_missing_targets_are_inert_but_handled() {
    let mut session = load(fixture!("navigation/probe-nav-03-missing-anchor.mu"));
    let state = |session: &SmolwebDocumentSession| {
        (
            session.doc.scroll_y(),
            session.doc.packet().cloned(),
            session.doc.folds().clone(),
        )
    };
    let before = state(&session);
    let region = link_region(&mut session, "jump to a missing anchor");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    assert_eq!(state(&session), before, "probe 03: nothing moves");
    assert!(session.take_in_page_navigations().is_empty());

    let control = block(
        &session,
        "MARKER CONTROL: line bound by the present-control anchor.",
    );
    let region = link_region(&mut session, "jump to a present anchor");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    assert_eq!(
        session.doc.scroll_y(),
        bounds(&mut session, control).unwrap().origin.y,
        "probe 03 control: the present anchor scrolls"
    );
    assert_eq!(
        session.take_in_page_navigations(),
        [jump("present-control", control)]
    );

    let mut session = load(fixture!("navigation/probe-nav-04-next-heading-tail.mu"));
    let tail = link_region(&mut session, "next heading from the tail");
    session.scroll_to(tail.bounds.origin.y);
    let before = state(&session);
    assert_eq!(click(&mut session, tail), SessionEffect::Handled);
    assert_eq!(state(&session), before, "probe 04: # past the last heading");
    assert!(session.take_in_page_navigations().is_empty());

    session.scroll_to(0.0);
    let first = block(&session, "First Heading");
    let top = link_region(&mut session, "next heading from the top");
    assert_eq!(click(&mut session, top), SessionEffect::Handled);
    assert_eq!(
        session.doc.scroll_y(),
        bounds(&mut session, first).unwrap().origin.y
    );
    assert_eq!(
        session.take_in_page_navigations(),
        [jump("first-heading", first)]
    );
}

#[test]
fn guide_structure_07c_fold_state_and_scroll_survive_replace_document() {
    const SHORT: u32 = 120;
    for (page, heading) in [
        (fixture!("guide-structure.mu"), "Closed fold"),
        (
            fixture!("navigation/probe-nav-07c-section-exit-fold.mu"),
            "Closed One A",
        ),
    ] {
        let (file, source) = page;
        let lines: Vec<&str> = source.split_inclusive('\n').collect();
        let first = 1 + lines
            .iter()
            .position(|line| line.trim_end().ends_with(heading))
            .unwrap();
        let mut session = load_sized((file, &lines[..first].concat()), W, SHORT);
        let region = fold_region(&mut session, heading);
        session.scroll_to(region.bounds.origin.y);
        assert_eq!(click(&mut session, region), SessionEffect::Handled);
        let scroll = session.doc.scroll_y();
        assert!(scroll > 0.0, "{file}: the prefix scrolls");

        let mut headings = Vec::new();
        for end in first + 1..=lines.len() {
            session.replace_document(lower(file, &lines[..end].concat()));
            let _ = session.frame(W, SHORT);
            assert!(is_open(&session, heading), "{file}: open after line {end}");
            assert_eq!(session.doc.scroll_y(), scroll, "{file}: pixel scroll kept");
            let navigation = &session.doc.document().navigation;
            headings.push(navigation.folds[fold(&session, heading)].heading);
        }
        headings.dedup();
        assert!(
            headings.len() > 1,
            "{file}: control, the badge moved the heading's block"
        );
        assert!(
            !is_open(&load_sized(page, W, SHORT), heading),
            "{file}: control, a session respawned per prefix loses the state"
        );
    }
}

#[test]
fn probe_06b_replace_document_drops_an_edited_heading_and_a_new_address() {
    let (file, source) = fixture!("navigation/probe-nav-06b-nested-collapsible.mu");
    let mut session = load((file, source));
    for heading in ["Outer Closed", "Inner Authored Closed"] {
        let region = fold_region(&mut session, heading);
        assert_eq!(click(&mut session, region), SessionEffect::Handled);
    }

    session.replace_document(lower(file, &format!("Inserted line.\n{source}")));
    assert!(is_open(&session, "Outer Closed") && is_open(&session, "Inner Authored Closed"));

    let edited = source.replace(
        "`->>Inner Authored Closed",
        "`->>Inner Authored Closed (edited)",
    );
    session.replace_document(lower(file, &edited));
    assert!(is_open(&session, "Outer Closed"), "decision 10: untouched");
    assert!(
        !is_open(&session, "Inner Authored Closed (edited)"),
        "decision 10: the edited heading is authored again"
    );

    session.replace_document(lower("another-page.mu", source));
    assert!(
        !is_open(&session, "Outer Closed"),
        "another address starts fresh"
    );
}

#[test]
fn probes_17_05_08_14c_reveal_after_toggle_and_resize() {
    let mut session = load(fixture!("navigation/probe-nav-17-nested-closed-target.mu"));
    let target = block(
        &session,
        "MARKER NESTED DEEP TARGET: bound inside both closed folds.",
    );
    let region = link_region(&mut session, "jump into two closed sections");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    let wide = session.doc.scroll_y();
    assert_eq!(wide, bounds(&mut session, target).unwrap().origin.y);

    let _ = session.frame(320, H);
    assert_eq!(
        session.doc.scroll_y(),
        wide,
        "decision 14: resize keeps pixel scroll"
    );
    for heading in ["Outer Closed", "Inner Closed"] {
        let fold = fold(&session, heading);
        assert!(session.doc.toggle_fold(fold));
    }
    let region = link_region(&mut session, "jump into two closed sections");
    session.scroll_to(0.0);
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    let narrow = bounds(&mut session, target).unwrap().origin.y;
    assert_eq!(session.doc.scroll_y(), narrow, "probe 17 after resize");
    assert_ne!(narrow, wide, "control: the narrow layout moved the target");

    let mut session = load(fixture!("navigation/probe-nav-05-closed-target.mu"));
    let outer = fold(&session, "Closed Outer");
    for _ in 0..2 {
        assert!(session.doc.toggle_fold(outer));
    }
    let hidden = block(&session, "Hidden Target");
    let region = link_region(&mut session, "jump to the hidden heading");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    assert_eq!(
        session.doc.scroll_y(),
        bounds(&mut session, hidden).unwrap().origin.y,
        "probe 05 after a toggle"
    );

    let mut session = load(fixture!("navigation/probe-nav-08-toggle-keys.mu"));
    let region = fold_region(&mut session, "Enter Target");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    let _ = session.frame(320, H);
    assert!(
        is_open(&session, "Enter Target"),
        "probe 08: state survives resize"
    );
    let body = block(
        &session,
        "MARKER ENTER BODY: revealed only when Enter Target is open.",
    );
    assert!(bounds(&mut session, body).is_some());

    let (file, source) = fixture!("navigation/probe-nav-14c-target-closed.mu");
    let mut session = load((file, source));
    let cross = block(&session, "Closed Cross Target");
    assert!(session.doc.reveal_anchor("closed-cross-target"));
    assert!(is_open(&session, "Closed Cross Outer"), "C1b probe 14c");
    assert_eq!(
        session.doc.scroll_y(),
        bounds(&mut session, cross).unwrap().origin.y
    );
    let before = (session.doc.scroll_y(), session.doc.folds().clone());
    assert!(!session.doc.reveal_anchor("no-such"));
    assert_eq!(
        (session.doc.scroll_y(), session.doc.folds().clone()),
        before
    );

    let mut landing =
        SmolwebDocument::from_document_with_theme(lower(file, source), SmolwebTheme::Plain);
    assert!(
        landing.reveal_anchor("closed-cross-target"),
        "a reveal before any layout waits for one"
    );
    let _ = landing.frame(W, H);
    assert_eq!(
        Some(landing.scroll_y()),
        landing
            .packet()
            .unwrap()
            .top_level_block(cross)
            .map(|rendered| rendered.bounds.origin.y)
    );
}

#[test]
fn probe_09_in_page_activation_issues_no_transport_at_the_session_boundary() {
    let mut session = load(fixture!("navigation/probe-nav-09-transport.mu"));
    let control = session.doc.document().outgoing_links()[0].to_owned();
    let target = block(
        &session,
        "MARKER TRANSPORT TARGET: reached without leaving this page.",
    );
    assert!(session.subresources().is_empty());
    let urls = |session: &SmolwebDocumentSession| {
        session
            .links()
            .into_iter()
            .map(|link| link.url)
            .collect::<Vec<_>>()
    };
    assert_eq!(urls(&session), [control.clone()], "no in-page URL");
    assert_eq!(session.inspect().unwrap().links, [control.clone()]);

    let region = link_region(&mut session, "in-page anchor jump");
    assert_eq!(click(&mut session, region), SessionEffect::Handled);
    session.scroll_to(0.0);
    assert_eq!(forward(&mut session), SessionEffect::Handled);
    assert_eq!(
        key(&mut session, SessionKey::Space),
        SessionEffect::Ignored,
        "Space on a link stays with the host"
    );
    assert_eq!(key(&mut session, SessionKey::Enter), SessionEffect::Handled);
    assert_eq!(
        session.take_in_page_navigations(),
        [
            jump("transport-target", target),
            jump("transport-target", target)
        ],
        "probe 09: pointer and keyboard"
    );
    assert!(session.subresources().is_empty());

    session.scroll_to(0.0);
    let region = link_region(&mut session, "same-node page load, positive control");
    assert_eq!(
        click(&mut session, region),
        SessionEffect::Navigate(control),
        "control: a network link is the host's request"
    );
    assert!(session.take_in_page_navigations().is_empty());
}

#[test]
fn decision_12_focus_reaches_every_region_with_a_configurable_indicator() {
    let stops = |session: &mut SmolwebDocumentSession| {
        let mut kinds: Vec<InteractionKind> = Vec::new();
        loop {
            assert_eq!(forward(session), SessionEffect::Handled);
            let kind = session.doc.focused_interaction(W, H).unwrap();
            if kinds.first() == Some(&kind) {
                return kinds;
            }
            kinds.push(kind);
        }
    };
    let mut session = load(fixture!("navigation/probe-nav-09-transport.mu"));
    assert!(matches!(
        stops(&mut session)[..],
        [InteractionKind::InPage { .. }, InteractionKind::Link { .. }]
    ));
    let mut session = load(fixture!("navigation/probe-nav-08-toggle-keys.mu"));
    assert_eq!(
        stops(&mut session),
        (0..3)
            .map(|fold| InteractionKind::Fold { fold })
            .collect::<Vec<_>>()
    );
    let mut spartan = SmolwebDocumentSession::new(
        SmolwebDocument::parse(
            "spartan://x.test/guestbook",
            "=: /guestbook/sign Sign it\n",
            SmolwebTheme::Plain,
        ),
        (W, H),
    );
    let _ = spartan.frame(W, H);
    assert_eq!(forward(&mut spartan), SessionEffect::Handled);
    assert!(matches!(
        key(&mut spartan, SessionKey::Enter),
        SessionEffect::Submit(submission) if submission.action == "/guestbook/sign"
    ));

    let (file, source) = fixture!("navigation/probe-nav-09-transport.mu");
    let rects = |style: &DocumentStyleSheet, focused: bool| {
        let mut document =
            SmolwebDocument::from_document(lower(file, source), style.clone(), [1.0; 4]);
        if focused {
            assert!(document.focus_move(SessionFocusDirection::Forward, W, H));
        }
        document
            .frame(W, H)
            .ops
            .iter()
            .filter_map(|op| match op {
                SceneOp::Rect(rect) => Some(rect.color),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut style = DocumentStyleSheet::default();
    style.focus_indicator.color = ColorToken::Rule;
    let focus_color = style.token_color(ColorToken::Rule);
    let unfocused = rects(&style, false);
    let focused = rects(&style, true);
    assert_eq!(focused.len(), unfocused.len() + 4, "one outline");
    assert_eq!(
        focused
            .iter()
            .filter(|color| **color == focus_color)
            .count(),
        unfocused
            .iter()
            .filter(|color| **color == focus_color)
            .count()
            + 4,
        "painted in the style sheet's token"
    );
    style.focus_indicator.width = 0.0;
    assert_eq!(
        rects(&style, true).len(),
        unfocused.len(),
        "a zero width paints nothing, as stock"
    );
}
