// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Minimal robots.txt honoring for the crawl frontier.
//!
//! A from-scratch implementation of the de-facto robots.txt subset crawlers honor:
//! `User-agent` groups, `Allow` / `Disallow` path rules, longest-match wins (an
//! `Allow` breaking a tie), `*` wildcard and `$` end anchor. Unreserved percent
//! escapes are decoded for matching, so `%61dmin` cannot bypass `/admin`.
//! This remains a bounded subset, not a full RFC 9309 implementation. Transport
//! errors fail closed in the runtime; an explicitly missing file is allowed.

/// The crawler's user-agent token, matched against `User-agent:` groups. The same
/// `merebot` token rides the descriptive UA sent on crawl fetches
/// ([`fetch::CRAWLER_USER_AGENT`](fetch::CRAWLER_USER_AGENT)).
pub const CRAWLER_UA: &str = "merebot";

/// Parsed robots.txt rules for one host: the `Allow` / `Disallow` path rules from the
/// most specific matching `User-agent` group (our UA, else `*`). Empty = allow all.
pub struct RobotsRules {
    /// `(is_allow, path_prefix)` in file order; matched longest-prefix-wins.
    rules: Vec<(bool, String)>,
}

impl RobotsRules {
    /// Parse robots.txt `text`, selecting the rules for [`CRAWLER_UA`] and falling
    /// back to the `*` group when our UA declares none. Comments (`#…`), blank lines,
    /// and unknown directives are ignored.
    pub fn parse(text: &str) -> Self {
        let mut star: Vec<(bool, String)> = Vec::new();
        let mut ours: Vec<(bool, String)> = Vec::new();
        let mut group_uas: Vec<String> = Vec::new();
        let mut seen_rule = false;
        let mut matched_ours = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "user-agent" => {
                    // A User-agent after a rule starts a fresh group; consecutive
                    // User-agent lines (no rule between) accumulate into one group.
                    if seen_rule {
                        group_uas.clear();
                        seen_rule = false;
                    }
                    group_uas.push(value.to_ascii_lowercase());
                    matched_ours |= value.eq_ignore_ascii_case(CRAWLER_UA);
                },
                "disallow" | "allow" => {
                    seen_rule = true;
                    if value.is_empty() {
                        continue; // an empty value imposes no restriction
                    }
                    let rule = (key == "allow", normalize_path(value));
                    if group_uas.iter().any(|u| u == "*") {
                        star.push(rule.clone());
                    }
                    if group_uas.iter().any(|u| u == CRAWLER_UA) {
                        ours.push(rule);
                    }
                },
                _ => {},
            }
        }
        let rules = if matched_ours { ours } else { star };
        Self { rules }
    }

    /// Whether `path` (a URL path like `/foo/bar`) may be fetched. The longest matching
    /// rule wins; an `Allow` beats a `Disallow` of equal length; no match = allowed.
    pub fn allows(&self, path: &str) -> bool {
        let path = normalize_path(path);
        let mut best: Option<(usize, bool)> = None; // (matched prefix len, is_allow)
        for (is_allow, prefix) in &self.rules {
            if pattern_matches(prefix.as_bytes(), path.as_bytes()) {
                let len = prefix
                    .bytes()
                    .filter(|byte| !matches!(byte, b'*' | b'$'))
                    .count();
                let better = match best {
                    None => true,
                    Some((blen, ballow)) => len > blen || (len == blen && *is_allow && !ballow),
                };
                if better {
                    best = Some((len, *is_allow));
                }
            }
        }
        best.map(|(_, allow)| allow).unwrap_or(true)
    }
}

fn normalize_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut normalized = String::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = |byte: u8| (byte as char).to_digit(16).map(|digit| digit as u8);
            if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                let decoded = high * 16 + low;
                if decoded.is_ascii_alphanumeric() || matches!(decoded, b'-' | b'.' | b'_' | b'~') {
                    normalized.push(decoded as char);
                } else {
                    normalized.push_str(&format!("%{decoded:02X}"));
                }
                index += 3;
                continue;
            }
        }
        if bytes[index].is_ascii() {
            normalized.push(bytes[index] as char);
        } else {
            normalized.push_str(&format!("%{:02X}", bytes[index]));
        }
        index += 1;
    }
    normalized
}

/// Prefix glob matching with greedy wildcard backtracking; no recursion or
/// regex compilation. A trailing `$` requests an exact end-of-path match.
fn pattern_matches(pattern: &[u8], path: &[u8]) -> bool {
    let anchored = pattern.last() == Some(&b'$');
    let pattern = if anchored {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };
    let (mut rule, mut pos) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    loop {
        if rule == pattern.len() {
            if !anchored || pos == path.len() {
                return true;
            }
        } else if pattern[rule] == b'*' {
            star = Some(rule);
            rule += 1;
            retry = pos;
            continue;
        } else if pos < path.len() && pattern[rule] == path[pos] {
            rule += 1;
            pos += 1;
            continue;
        }
        if let Some(star_pos) = star {
            if retry < path.len() {
                retry += 1;
                pos = retry;
                rule = star_pos + 1;
                continue;
            }
        }
        return false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_comment_only_allows_everything() {
        assert!(RobotsRules::parse("").allows("/anything"));
        assert!(RobotsRules::parse("# just a comment\n").allows("/x"));
    }

    #[test]
    fn disallow_blocks_its_prefix() {
        let r = RobotsRules::parse("User-agent: *\nDisallow: /admin/");
        assert!(!r.allows("/admin/users"));
        assert!(r.allows("/public"));
    }

    #[test]
    fn empty_disallow_is_allow_all() {
        let r = RobotsRules::parse("User-agent: *\nDisallow:");
        assert!(r.allows("/anything"));
    }

    #[test]
    fn disallow_root_blocks_all() {
        let r = RobotsRules::parse("User-agent: *\nDisallow: /");
        assert!(!r.allows("/anything"));
        assert!(!r.allows("/"));
    }

    #[test]
    fn allow_overrides_disallow_on_a_longer_match() {
        let r = RobotsRules::parse("User-agent: *\nDisallow: /a\nAllow: /a/b");
        assert!(!r.allows("/a/x"), "the shorter Disallow blocks");
        assert!(r.allows("/a/b/c"), "the longer Allow wins");
    }

    #[test]
    fn our_ua_group_overrides_the_star_group() {
        // `*` disallows everything; our group narrows it to just /secret.
        let txt = "User-agent: *\nDisallow: /\n\nUser-agent: merebot\nDisallow: /secret";
        let r = RobotsRules::parse(txt);
        assert!(
            r.allows("/public"),
            "our UA ignores the * group's Disallow: /"
        );
        assert!(!r.allows("/secret/x"));
    }

    #[test]
    fn comments_and_casing_are_tolerated() {
        let r = RobotsRules::parse("USER-AGENT: *  # any bot\nDISALLOW: /tmp  # temp");
        assert!(!r.allows("/tmp/file"));
        assert!(r.allows("/keep"));
    }

    #[test]
    fn wildcard_end_anchor_and_encoded_paths_are_honored() {
        let rules = RobotsRules::parse(
            "User-agent: *\nDisallow: /*.pdf$\nDisallow: /admin\nDisallow: /é\nAllow: /admin/public",
        );
        assert!(!rules.allows("/reports/deep.pdf"));
        assert!(rules.allows("/reports/deep.pdf?download=1"));
        assert!(!rules.allows("/%61dmin/private"));
        assert!(rules.allows("/%61dmin/public"));
        assert!(!rules.allows("/%C3%A9"));
    }

    #[test]
    fn explicit_empty_bot_group_does_not_inherit_star_restrictions() {
        let rules =
            RobotsRules::parse("User-agent: *\nDisallow: /\nUser-agent: merebot\nDisallow:");
        assert!(rules.allows("/public"));
    }
}
