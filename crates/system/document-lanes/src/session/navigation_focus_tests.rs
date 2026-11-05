// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

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
        bounds(&mut session, target).unwrap().origin.y,
        "resize keeps the revealed passage at the leading edge"
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
    assert!(
        matches!(
            key(&mut spartan, SessionKey::Enter),
            SessionEffect::Submit(submission) if submission.action == "/guestbook/sign"
                && submission.method == SessionFormMethod::Post
                && submission.fields.is_empty()
        ),
        "a mutation endpoint is a POST the host completes, not a GET to follow"
    );

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
