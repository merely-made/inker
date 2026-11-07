// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Smolweb document palettes: how a gemtext, gopher or feed page is coloured.
//!
//! The default is a **per-site palette**: the document's host is hashed into a
//! hue, and tinct derives the palette from seeds at that hue under the host's
//! light, dark or high-contrast mode. Each capsule keeps one colour identity
//! across modes (the Lagrange approach), and its text and links clear tinct's
//! contrast floors in every mode. Presets override it: a neutral `Plain`,
//! fixed `Light` and `Dark`, the host application's palette (`App`), or the
//! host's mode (`System`).
//!
//! The lanes' own copy (engine split plan D16, D19): hosts keep Tabard's for
//! their stylesheets and convert at the edge. Tinct comes from Graphshell's
//! contract root, which Inker may name.

use tinct::oklch::Oklch;
use tinct::{Seeds, Srgb, color_to_hex, derive_gated_roles, derive_palette_with};

pub use tinct::ModeProfile;

/// How a smolweb document is coloured.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SmolwebTheme {
    /// Default: a palette derived from the site's host under the host's mode,
    /// so each capsule has its own consistent colour identity (the Lagrange
    /// approach).
    #[default]
    Site,
    /// A neutral, un-themed palette.
    Plain,
    /// A warm fixed light palette.
    Light,
    /// A fixed dark palette.
    Dark,
    /// The host application's palette. The host supplies it (e.g. derived from
    /// its tinct theme seeds), so smolweb pages match the surrounding app chrome.
    App(SmolwebPalette),
    /// The fixed `Light` or `Dark` palette, whichever the host's mode selects.
    System,
}

/// A document palette as CSS colour strings (`#rrggbb` or `rgb(…)`). The fixed
/// themes construct one; a host supplies its own through [`SmolwebTheme::App`]
/// (e.g. mapped from tinct), the seam that lets smolweb pages match the app
/// theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmolwebPalette {
    /// Page background.
    pub bg: String,
    /// Body text.
    pub fg: String,
    /// Link colour.
    pub link: String,
    /// Quote text / border, and other muted accents.
    pub quote: String,
    /// Preformatted / code-block background.
    pub pre_bg: String,
}

impl SmolwebPalette {
    /// The palette for a document at `url` under `theme` in the light mode.
    pub fn for_theme(theme: &SmolwebTheme, url: &str) -> Self {
        Self::for_mode(theme, url, ModeProfile::LIGHT)
    }

    /// The palette for a document at `url` under `theme` in the host's `mode`.
    /// `url` seeds the per-site palette; `Site` and `System` follow the mode,
    /// while the fixed and app themes ignore it.
    pub fn for_mode(theme: &SmolwebTheme, url: &str, mode: ModeProfile) -> Self {
        match theme {
            SmolwebTheme::Site => site(url, mode),
            SmolwebTheme::Plain => fixed("#ffffff", "#1a1a1a", "#0b57d0", "#555555", "#f4f4f4"),
            SmolwebTheme::System if mode.dark => dark(),
            SmolwebTheme::Light | SmolwebTheme::System => {
                fixed("#fbfaf7", "#23211c", "#1a6e57", "#5b574e", "#f0eee8")
            },
            SmolwebTheme::Dark => dark(),
            SmolwebTheme::App(palette) => palette.clone(),
        }
    }
}

fn dark() -> SmolwebPalette {
    fixed("#16181c", "#e6e3dc", "#7db4ff", "#a8a49a", "#21242a")
}

fn fixed(bg: &str, fg: &str, link: &str, quote: &str, pre_bg: &str) -> SmolwebPalette {
    SmolwebPalette {
        bg: bg.into(),
        fg: fg.into(),
        link: link.into(),
        quote: quote.into(),
        pre_bg: pre_bg.into(),
    }
}

/// Seeds at the capsule's hue: a lightly tinted neutral for the ground and
/// text, and a saturated primary for links. tinct places the lightness ladder
/// for the mode and gates the link against both grounds. The page sits on the
/// surface tier; preformatted blocks take the deeper background tier.
fn site(url: &str, mode: ModeProfile) -> SmolwebPalette {
    let hue = f64::from(hue_from_host(url)).to_radians();
    let tone = |l, c| Oklch { l, c, h: hue }.to_srgb();
    let accent = tone(0.55, 0.14);
    let seeds = Seeds {
        primary: accent,
        secondary: accent,
        tertiary: accent,
        neutral: tone(0.5, 0.025),
        text_header: None,
        text_body: None,
        success: Srgb::rgb(0x4F, 0xB3, 0x6E),
        danger: Srgb::rgb(0xD5, 0x4E, 0x4E),
        dark: mode.dark,
    };
    let palette = derive_palette_with(&seeds, mode);
    SmolwebPalette {
        bg: color_to_hex(palette.surface),
        fg: color_to_hex(palette.text),
        link: color_to_hex(derive_gated_roles(&palette).accent),
        quote: color_to_hex(palette.text_dim),
        pre_bg: color_to_hex(palette.bg),
    }
}

/// Hash the URL's host into a hue in `0..360`: djb2 over the host bytes, so a
/// capsule keeps one identity across its pages.
fn hue_from_host(url: &str) -> u16 {
    let host = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("");
    let hash = host.bytes().fold(5381_u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(u32::from(byte))
    });
    (hash % 360) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_hue_is_stable_and_host_scoped() {
        // Same host, different paths -> same palette (one identity per capsule).
        let a = SmolwebPalette::for_theme(&SmolwebTheme::Site, "gemini://example.test/a");
        let b = SmolwebPalette::for_theme(&SmolwebTheme::Site, "gemini://example.test/b/c");
        assert_eq!(a, b);
        // Different hosts -> different palettes (with overwhelming likelihood).
        let other = SmolwebPalette::for_theme(&SmolwebTheme::Site, "gemini://elsewhere.test/");
        assert_ne!(a, other);
        assert!(
            tinct::color_from_hex(&a.bg).is_some(),
            "parseable colours: {}",
            a.bg
        );
    }

    #[test]
    fn site_palettes_follow_the_mode_and_stay_legible() {
        let colour = |css: &str| tinct::color_from_hex(css).unwrap();
        let modes = [
            ModeProfile::LIGHT,
            ModeProfile::DARK,
            ModeProfile::HC_LIGHT,
            ModeProfile::HC_DARK,
        ];
        for host in [
            "geminiprotocol.net",
            "example.test",
            "elsewhere.test",
            "a",
            "",
        ] {
            let url = format!("gemini://{host}/");
            for mode in modes {
                let p = SmolwebPalette::for_mode(&SmolwebTheme::Site, &url, mode);
                let (bg, fg) = (colour(&p.bg), colour(&p.fg));
                let floor = if mode.high_contrast { 7.0 } else { 4.5 };
                assert!(
                    tinct::contrast(fg, bg) >= floor,
                    "{url} {mode:?} text {p:?}"
                );
                assert!(
                    tinct::contrast(colour(&p.link), bg) >= 4.5,
                    "{url} {mode:?} link {p:?}"
                );
                assert!(
                    tinct::contrast(colour(&p.quote), bg) >= 3.0,
                    "{url} {mode:?} quote {p:?}"
                );
                assert_eq!(
                    tinct::relative_luminance(bg) < tinct::relative_luminance(fg),
                    mode.dark,
                    "{url} {mode:?} ground direction"
                );
            }
        }
        let light = SmolwebPalette::for_theme(&SmolwebTheme::Site, "gemini://example.test/");
        let dark = SmolwebPalette::for_mode(
            &SmolwebTheme::Site,
            "gemini://example.test/",
            ModeProfile::DARK,
        );
        assert_ne!(light, dark);
    }

    #[test]
    fn default_is_site() {
        assert_eq!(SmolwebTheme::default(), SmolwebTheme::Site);
    }

    #[test]
    fn app_passes_through_and_system_follows_the_mode() {
        let palette = fixed("#102030", "#fafafa", "#33ccff", "#99aabb", "#0a1622");
        let app = SmolwebTheme::App(palette.clone());
        assert_eq!(SmolwebPalette::for_theme(&app, "gemini://x.test/"), palette);
        assert_eq!(
            SmolwebPalette::for_mode(&app, "gemini://x.test/", ModeProfile::DARK),
            palette
        );
        assert_eq!(
            SmolwebPalette::for_theme(&SmolwebTheme::System, ""),
            SmolwebPalette::for_theme(&SmolwebTheme::Light, "")
        );
        assert_eq!(
            SmolwebPalette::for_mode(&SmolwebTheme::System, "", ModeProfile::HC_DARK),
            SmolwebPalette::for_theme(&SmolwebTheme::Dark, "")
        );
    }
}
