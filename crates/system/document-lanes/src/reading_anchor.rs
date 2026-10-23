// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Text location resolution over the exact laid-out text, without a second parser.
use document_canvas::text::SearchableText;
use tabard::ReadingAnchor;

fn revision(texts: &[SearchableText]) -> u64 {
    // A deterministic change detector, not a security digest. Include boundaries.
    texts.iter().fold(0xcbf29ce484222325, |mut hash, text| {
        for b in text.text.bytes().chain([0xff]) { hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3); }
        hash
    })
}
pub(crate) fn capture(source: &str, texts: &[SearchableText], y: f32, scale: f32) -> Option<ReadingAnchor> {
    let (index, text, offset, rect) = texts.iter().enumerate().flat_map(|(i,t)| t.clusters.iter().map(move |(range,rect)| (i,t,range.start,rect)))
        .filter(|(_,_,_,r)| r.size.height > 0.0 && r.max_y() > y)
        .min_by(|a,b| a.3.origin.y.total_cmp(&b.3.origin.y).then(a.3.origin.x.total_cmp(&b.3.origin.x)))?;
    let exact: String = text.text[offset..].chars().take(64).collect();
    if exact.is_empty() { return None; }
    Some(ReadingAnchor { source:source.into(), revision:revision(texts), text:index, offset,
        prefix:text.text[..offset].chars().rev().take(24).collect::<String>().chars().rev().collect(),
        suffix:text.text[offset + exact.len()..].chars().take(24).collect(), exact,
        line_offset: ((y - rect.origin.y) / rect.size.height).clamp(-2.0,2.0), fallback_y:y / scale })
}
pub(crate) fn resolve(source: &str, texts: &[SearchableText], anchor: &ReadingAnchor, scale:f32) -> Option<f32> {
    anchor.validate().ok()?;
    if source != anchor.source { return None; }
    let same = revision(texts) == anchor.revision;
    let direct = texts.get(anchor.text).filter(|t| same && t.text.get(anchor.offset..).is_some_and(|s|s.starts_with(&anchor.exact)))
        .map(|_|(anchor.text,anchor.offset));
    let found = direct.or_else(|| {
        let matches: Vec<_> = texts.iter().enumerate().flat_map(|(i,t)| t.text.match_indices(&anchor.exact).filter(|(o,_)|
            t.text[..*o].ends_with(&anchor.prefix) && t.text[o+anchor.exact.len()..].starts_with(&anchor.suffix)).map(move |(o,_)|(i,o))).collect();
        (matches.len() == 1).then(||matches[0])
    });
    if let Some((index,offset)) = found {
        let (_,rect) = texts[index].clusters.iter().find(|(r,_)|r.start <= offset && offset < r.end)?;
        Some((rect.origin.y + anchor.line_offset * rect.size.height).max(0.0))
    } else { same.then_some(anchor.fallback_y * scale) }
}
#[cfg(test)]
mod tests {
    use super::*;
    use document_canvas::{Point,Size,Rect};
    fn text(value:&str,y:f32)->SearchableText { SearchableText { text:value.into(), clusters:value.char_indices().map(|(i,c)|(i..i+c.len_utf8(),Rect { origin:Point{x:i as f32,y},size:Size{width:10.,height:20.} })).collect() } }
    #[test]
    fn quote_survives_reflow_revision_change_and_refuses_ambiguous_or_other_source() {
        let a=capture("a",&[text("first",0.),text("é exact passage",200.)],205.,1.).unwrap();
        assert_eq!(resolve("a",&[text("first",0.),text("é exact passage",400.)],&a,1.),Some(405.));
        assert_eq!(resolve("a",&[text("inserted",0.),text("first",30.),text("é exact passage",500.)],&a,1.),Some(505.));
        assert_eq!(resolve("other",&[text("é exact passage",500.)],&a,1.),None);
        assert_eq!(resolve("a",&[text("é exact passage",100.),text("é exact passage",500.)],&a,1.),None);
        assert_eq!(resolve("a",&[text("replaced",0.)],&a,1.),None);
    }
}
