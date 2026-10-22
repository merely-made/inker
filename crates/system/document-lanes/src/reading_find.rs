// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Find over the exact flattened text and Parley cluster geometry presented by
//! document-canvas. Inline runs and soft wraps do not create search boundaries.
use document_canvas::{Rect, text::SearchableText};
use inker::{
    DocumentFindDirection, DocumentFindMatch, DocumentFindQuery, DocumentFindReveal,
    DocumentFindState,
};

#[derive(Default)]
pub(crate) struct ReadingFind {
    query: DocumentFindQuery,
    matches: Vec<(String, Vec<Rect>)>,
    current: Option<usize>,
}
impl ReadingFind {
    pub fn replace(&mut self, query: DocumentFindQuery, text: &[SearchableText]) {
        self.query = query;
        self.current = None;
        self.reflow(text);
    }
    pub fn reflow(&mut self, texts: &[SearchableText]) {
        self.matches.clear();
        if self.query.text.is_empty() {
            self.current = None;
            return;
        }
        for text in texts {
            for range in occurrences(&text.text, &self.query) {
                let rects: Vec<_> = text
                    .clusters
                    .iter()
                    .filter(|(r, rect)| {
                        r.start < range.end && range.start < r.end && rect.size.width > 0.0
                    })
                    .map(|(_, rect)| *rect)
                    .collect();
                if !rects.is_empty() {
                    self.matches.push((text.text[range].to_owned(), rects));
                }
            }
        }
        self.current = if self.matches.is_empty() {
            None
        } else {
            Some(self.current.unwrap_or(0).min(self.matches.len() - 1))
        };
    }
    pub fn step(&mut self, direction: DocumentFindDirection) {
        let count = self.matches.len();
        if count == 0 {
            return;
        }
        let current = self.current.unwrap_or(0);
        self.current = Some(match direction {
            DocumentFindDirection::Next => (current + 1) % count,
            DocumentFindDirection::Previous => (current + count - 1) % count,
        });
    }
    pub fn current_y(&self) -> Option<f32> {
        self.current
            .and_then(|i| self.matches.get(i))?
            .1
            .iter()
            .map(|r| r.origin.y)
            .reduce(f32::min)
    }
    pub fn state(&self) -> DocumentFindState {
        DocumentFindState {
            query: self.query.clone(),
            count: self.matches.len(),
            current: self.current,
            complete: true,
            matches: self
                .matches
                .iter()
                .map(|(label, rects)| DocumentFindMatch {
                    label: label.clone(),
                    role: Some("text".into()),
                    reveal: DocumentFindReveal::ScrollY(
                        rects
                            .iter()
                            .map(|r| r.origin.y)
                            .reduce(f32::min)
                            .unwrap_or(0.0),
                    ),
                })
                .collect(),
        }
    }
    pub fn paint(&self, scene: &mut netrender::Scene, left: f32, top: f32, size: (u32, u32)) {
        let mut marks = netrender::Scene::new(size.0, size.1);
        for (i, (_, rects)) in self.matches.iter().enumerate() {
            let current = self.current == Some(i);
            for rect in rects {
                let x = (rect.origin.x - left).max(0.0);
                let y = (rect.origin.y - top).max(0.0);
                let right = (rect.max_x() - left).min(size.0 as f32);
                let bottom = (rect.max_y() - top).min(size.1 as f32);
                if right > x && bottom > y {
                    marks.push_rect(
                        x,
                        y,
                        right,
                        bottom,
                        if current {
                            [0.65, 0.4355, 0.078, 0.65]
                        } else {
                            [0.35, 0.308, 0.0875, 0.35]
                        },
                    );
                }
            }
        }
        // Highlights precede glyphs; the caller then prepends the paper.
        scene.ops.splice(0..0, marks.ops);
    }
}

fn occurrences(text: &str, query: &DocumentFindQuery) -> Vec<std::ops::Range<usize>> {
    if query.text.is_empty() {
        return Vec::new();
    }
    if query.match_case {
        return text
            .match_indices(&query.text)
            .map(|(start, value)| start..start + value.len())
            .collect();
    }
    // Unicode lowercase may change byte length (İ, capital sharp S). Keep a
    // map to original scalar boundaries rather than slicing source by the
    // folded byte offsets. This is literal lowercase matching, not a regex.
    let mut folded = String::new();
    let mut map = Vec::new();
    for (start, ch) in text.char_indices() {
        for lower in ch.to_lowercase() {
            let old = folded.len();
            folded.push(lower);
            map.extend(std::iter::repeat_n(
                (start, start + ch.len_utf8()),
                folded.len() - old,
            ));
        }
    }
    folded
        .match_indices(&query.text.to_lowercase())
        .map(|(start, value)| map[start].0..map[start + value.len() - 1].1)
        .fold(Vec::new(), |mut out, range| {
            if out.last() != Some(&range) {
                out.push(range);
            }
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_unicode_search_preserves_original_boundaries() {
        let text = "İstanbul CAFÉ café <>&";
        assert_eq!(
            occurrences(text, &DocumentFindQuery::new("café")),
            vec![10..15, 16..21]
        );
        for range in occurrences(text, &DocumentFindQuery::new("i")) {
            assert_eq!(&text[range], "İ");
        }
        assert!(occurrences(text, &DocumentFindQuery::new("xyz")).is_empty());
    }
    #[test]
    fn off_origin_highlights_keep_ordered_edges_inside_the_viewport() {
        let mut find = ReadingFind::default();
        find.replace(
            DocumentFindQuery::new("needle"),
            &[SearchableText {
                text: "needle".into(),
                clusters: vec![(0..6, Rect::from_xywh(400.0, 350.0, 60.0, 20.0))],
            }],
        );
        let mut scene = netrender::Scene::new(640, 400);
        find.paint(&mut scene, 0.0, 0.0, (640, 400));
        assert!(!scene.ops.is_empty());
        for op in &scene.ops {
            let netrender::SceneOp::Rect(rect) = op else {
                panic!("find emits rectangle highlights");
            };
            assert!(
                rect.x1 > rect.x0 && rect.y1 > rect.y0,
                "renderer requires ordered edges"
            );
            assert!(rect.x0 >= 0.0 && rect.y0 >= 0.0 && rect.x1 <= 640.0 && rect.y1 <= 400.0);
            assert!(
                rect.color[..3]
                    .iter()
                    .all(|component| *component <= rect.color[3]),
                "scene colors are premultiplied"
            );
        }
    }
    #[test]
    fn stepping_wraps_reflows_and_clears() {
        let mut find = ReadingFind::default();
        let text = |y| SearchableText {
            text: "one one".into(),
            clusters: vec![
                (0..3, Rect::from_xywh(4.0, y, 30.0, 20.0)),
                (4..7, Rect::from_xywh(40.0, y, 30.0, 20.0)),
            ],
        };
        find.replace(DocumentFindQuery::new("one"), &[text(10.0)]);
        assert_eq!((find.state().count, find.state().current), (2, Some(0)));
        find.step(DocumentFindDirection::Previous);
        assert_eq!(find.state().current, Some(1));
        find.reflow(&[text(200.0)]);
        assert_eq!(find.current_y(), Some(200.0));
        assert_eq!(find.state().current, Some(1));
        find.step(DocumentFindDirection::Next);
        assert_eq!(find.state().current, Some(0));
        find.replace(DocumentFindQuery::default(), &[text(10.0)]);
        assert_eq!((find.state().count, find.state().current), (0, None));
    }
}
