// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The lanes' own reading intent, reading anchor and reader styles (engine split
//! plan D16). Hosts persist and theme these their own way and convert at the
//! edge; nothing here names a host's settings types.
use std::collections::BTreeMap;

/// Reading intent for one projection: type size and measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadingPreferences {
    pub text_size: u16,
    /// Measure in ems, so changing type size retains the intended line length.
    pub column_em: u16,
}
impl Default for ReadingPreferences {
    fn default() -> Self {
        Self {
            text_size: 18,
            column_em: 36,
        }
    }
}
impl ReadingPreferences {
    pub const MIN_TEXT: u16 = 14;
    pub const MAX_TEXT: u16 = 28;
    pub const MIN_COLUMN: u16 = 24;
    pub const MAX_COLUMN: u16 = 48;
    pub fn validate(&self) -> Result<(), String> {
        if !(Self::MIN_TEXT..=Self::MAX_TEXT).contains(&self.text_size)
            || !(Self::MIN_COLUMN..=Self::MAX_COLUMN).contains(&self.column_em)
        {
            return Err("Reading text must be 14–28 px and its column 24–48 em.".into());
        }
        Ok(())
    }
    pub fn step_text(&mut self, larger: bool) {
        self.text_size = (i32::from(self.text_size) + if larger { 2 } else { -2 })
            .clamp(i32::from(Self::MIN_TEXT), i32::from(Self::MAX_TEXT))
            as u16;
    }
    pub fn step_column(&mut self, wider: bool) {
        self.column_em = (i32::from(self.column_em) + if wider { 4 } else { -4 })
            .clamp(i32::from(Self::MIN_COLUMN), i32::from(Self::MAX_COLUMN))
            as u16;
    }
    /// The reader styles this intent implies: only the reading size.
    pub fn styles(self) -> ReaderStyles {
        let mut value = ReaderStyles::default();
        value
            .set(
                ReaderRole::Reading,
                ReaderFontStyle {
                    size_px: Some(f32::from(self.text_size)),
                    ..Default::default()
                },
            )
            .expect("bounded reading size");
        value
    }
    pub fn width_px(self) -> f32 {
        f32::from(self.text_size) * f32::from(self.column_em)
    }
}

/// A bounded text quote at the leading visible line. Geometry is only a fallback
/// for the identical source revision; a changed source must resolve the quote.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadingAnchor {
    pub source: String,
    pub revision: u64,
    pub text: usize,
    pub offset: usize,
    pub exact: String,
    pub prefix: String,
    pub suffix: String,
    pub line_offset: f32,
    pub fallback_y: f32,
}
impl ReadingAnchor {
    pub fn validate(&self) -> Result<(), String> {
        if self.source.len() > 8192
            || self.exact.len() > 512
            || self.prefix.len() > 128
            || self.suffix.len() > 128
            || self.exact.is_empty()
            || !self.line_offset.is_finite()
            || !(-2.0..=2.0).contains(&self.line_offset)
            || !self.fallback_y.is_finite()
            || self.fallback_y < 0.0
        {
            return Err("Invalid reading anchor".into());
        }
        Ok(())
    }
}

/// The document roles a host may restyle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReaderRole {
    Reading,
    Heading,
    Code,
    Label,
}

/// One role's authored face. `family` is a CSS family list the host has
/// already resolved (its fallback included); absent fields leave the
/// document's baseline untouched. Sizes are CSS pixels; line height is unitless.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReaderFontStyle {
    pub family: Option<String>,
    pub size_px: Option<f32>,
    pub line_height: Option<f32>,
}
impl ReaderFontStyle {
    pub fn is_empty(&self) -> bool {
        self.family.is_none() && self.size_px.is_none() && self.line_height.is_none()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.family.as_ref().is_some_and(|f| f.trim().is_empty()) {
            return Err("Font family must not be empty.".into());
        }
        if self
            .size_px
            .is_some_and(|v| !v.is_finite() || !(6.0..=128.0).contains(&v))
        {
            return Err("Font size must be between 6 and 128 CSS pixels.".into());
        }
        if self
            .line_height
            .is_some_and(|v| !v.is_finite() || !(0.8..=3.0).contains(&v))
        {
            return Err("Line height must be between 0.8 and 3.".into());
        }
        Ok(())
    }
}

/// Reader styles by role.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReaderStyles(pub BTreeMap<ReaderRole, ReaderFontStyle>);
impl ReaderStyles {
    pub fn validate(&self) -> Result<(), String> {
        for (role, style) in &self.0 {
            style.validate().map_err(|error| format!("{role:?}: {error}"))?;
        }
        Ok(())
    }
    pub fn set(&mut self, role: ReaderRole, style: ReaderFontStyle) -> Result<(), String> {
        style.validate()?;
        if style.is_empty() {
            self.0.remove(&role);
        } else {
            self.0.insert(role, style);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_step_independently_and_keep_line_length() {
        let mut a = ReadingPreferences::default();
        let b = a;
        a.step_text(true);
        a.step_column(false);
        assert_eq!(b, ReadingPreferences::default());
        assert_eq!(a.width_px(), 640.0);
        for _ in 0..50 {
            a.step_text(true);
            a.step_column(false);
        }
        assert_eq!((a.text_size, a.column_em), (28, 24));
        assert!(a.validate().is_ok());
        assert!(ReadingPreferences { text_size: 0, column_em: 36 }.validate().is_err());
    }
    #[test]
    fn styles_refuse_out_of_range_and_drop_empty_roles() {
        let mut styles = ReadingPreferences::default().styles();
        assert_eq!(styles.0[&ReaderRole::Reading].size_px, Some(18.0));
        assert!(styles.set(ReaderRole::Code, ReaderFontStyle { size_px: Some(500.0), ..Default::default() }).is_err());
        styles.set(ReaderRole::Reading, ReaderFontStyle::default()).unwrap();
        assert!(styles.0.is_empty());
    }
    #[test]
    fn anchor_bounds_quote_and_geometry() {
        let anchor = ReadingAnchor {
            source: "gemini://x.test/".into(),
            revision: 1,
            text: 0,
            offset: 0,
            exact: "word".into(),
            prefix: String::new(),
            suffix: String::new(),
            line_offset: 0.0,
            fallback_y: 0.0,
        };
        assert!(anchor.validate().is_ok());
        assert!(ReadingAnchor { exact: String::new(), ..anchor.clone() }.validate().is_err());
        assert!(ReadingAnchor { fallback_y: -1.0, ..anchor }.validate().is_err());
    }
}
