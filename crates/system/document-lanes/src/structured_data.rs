// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Composition between Fleece's lossless HTML evidence and Mere's JSON-LD
//! processor.

use fleece::{EmbeddedJsonLdBlock, JsonLdParseStatus};
use linked_data::{ContextCache, GraphContribution};

/// One source block's identity and RDF projection outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonLdBlockProjection {
    pub document_order: u64,
    pub element_id: Option<String>,
    pub declared_type: String,
    /// The exact DOM text given to the JSON-LD processor.
    pub dom_text: String,
    pub outcome: JsonLdProjectionOutcome,
}

/// The result of projecting one preserved JSON-LD block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonLdProjectionOutcome {
    InvalidJson,
    Projected(GraphContribution),
    ExpansionFailed(String),
}

/// Project every preserved block while retaining its source identity and error.
pub fn project_json_ld_blocks(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
) -> Vec<JsonLdBlockProjection> {
    project_json_ld_blocks_with_base_iri(blocks, contexts, None)
}

/// Project every preserved block with a caller-supplied resolved document URL.
///
/// JSON-LD resolves relative IRIs against its `base` option. Fleece deliberately
/// does not know a page's resolved address, transport, or custody, so the host
/// that owns those facts supplies `base_iri` here. The value is used only for
/// JSON-LD expansion and is not folded into the retained block provenance.
pub fn project_json_ld_blocks_with_base_iri(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
    base_iri: Option<&str>,
) -> Vec<JsonLdBlockProjection> {
    blocks
        .iter()
        .map(|block| {
            let outcome = if !matches!(&block.parse, JsonLdParseStatus::Parsed(_)) {
                JsonLdProjectionOutcome::InvalidJson
            } else {
                match linked_data::from_jsonld_with_contexts_and_base_iri(
                    block.dom_text.as_bytes(),
                    contexts.clone(),
                    base_iri,
                ) {
                    Ok(contribution) => JsonLdProjectionOutcome::Projected(contribution),
                    Err(error) => JsonLdProjectionOutcome::ExpansionFailed(error.to_string()),
                }
            };
            JsonLdBlockProjection {
                document_order: block.document_order,
                element_id: block.element_id.clone(),
                declared_type: block.declared_type.clone(),
                dom_text: block.dom_text.clone(),
                outcome,
            }
        })
        .collect()
}

/// Project preserved JSON-LD blocks into graph contributions in input order.
///
/// Fleece supplies these blocks in DOM order. This adapter uses each block's
/// retained DOM text as the JSON-LD input; its syntax tree remains evidence,
/// rather than becoming a second serialization path. Blocks that failed
/// Fleece's JSON syntax parse, or that fail JSON-LD processing with the supplied
/// offline context cache, do not contribute under the current best-effort
/// behavior.
pub fn json_ld_contributions(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
) -> Vec<GraphContribution> {
    json_ld_contributions_with_base_iri(blocks, contexts, None)
}

/// Like [`json_ld_contributions`], resolving relative JSON-LD IRIs against a
/// caller-owned document address.
pub fn json_ld_contributions_with_base_iri(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
    base_iri: Option<&str>,
) -> Vec<GraphContribution> {
    project_json_ld_blocks_with_base_iri(blocks, contexts, base_iri)
        .into_iter()
        .filter_map(|projection| match projection.outcome {
            JsonLdProjectionOutcome::Projected(contribution) => Some(contribution),
            JsonLdProjectionOutcome::InvalidJson | JsonLdProjectionOutcome::ExpansionFailed(_) => {
                None
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use fleece::{JsonLdParseStatus, StructuredValue, extract_json_ld_blocks};
    use genet_static_dom::StaticDocument;
    use linked_data::ContextCache;

    use super::{
        JsonLdProjectionOutcome, json_ld_contributions, json_ld_contributions_with_base_iri,
        project_json_ld_blocks, project_json_ld_blocks_with_base_iri,
    };

    fn subject_id(contribution: &linked_data::GraphContribution) -> &str {
        contribution
            .nodes
            .iter()
            .find(|node| node.id.starts_with("https://page.test/"))
            .expect("page subject")
            .id
            .as_str()
    }

    #[test]
    fn projects_raw_json_ld_text_in_dom_order_and_skips_invalid_and_js_scripts() {
        let document = StaticDocument::parse(
            r#"<html><head>
              <script id="first" type="Application/LD+JSON; charset=utf-8">
                {"@context":{"name":"https://schema.org/name"},
                 "@id":"https://page.test/first","name":"First"}
              </script>
              <script id="broken" type="application/ld+json">{"broken":</script>
              <script id="javascript" type="text/javascript">
                {"@id":"https://page.test/javascript"}
              </script>
            </head><body>
              <script id="second" type="application/ld+json">
                {"@context":{"name":"https://schema.org/name"},
                 "@id":"https://page.test/second","name":"Second"}
              </script>
            </body></html>"#,
        );
        let mut blocks = extract_json_ld_blocks(&document);

        assert_eq!(blocks.len(), 3, "JavaScript is not JSON-LD evidence");
        assert_eq!(blocks[0].element_id.as_deref(), Some("first"));
        assert_eq!(
            blocks[0].declared_type,
            "Application/LD+JSON; charset=utf-8"
        );
        assert_eq!(blocks[1].parse, JsonLdParseStatus::InvalidJson);
        assert_eq!(blocks[2].element_id.as_deref(), Some("second"));
        assert!(
            blocks
                .windows(2)
                .all(|pair| pair[0].document_order < pair[1].document_order)
        );

        // Deliberately replace the retained syntax tree. The graph still comes
        // from dom_text, which is the lossless processing input.
        blocks[0].parse = JsonLdParseStatus::Parsed(StructuredValue::Null);
        let projections = project_json_ld_blocks(&blocks, &ContextCache::new());
        assert_eq!(projections.len(), 3);
        assert_eq!(projections[0].element_id.as_deref(), Some("first"));
        assert_eq!(projections[0].dom_text, blocks[0].dom_text);
        assert!(matches!(
            projections[0].outcome,
            JsonLdProjectionOutcome::Projected(_)
        ));
        assert_eq!(projections[1].element_id.as_deref(), Some("broken"));
        assert_eq!(projections[1].outcome, JsonLdProjectionOutcome::InvalidJson);
        assert_eq!(projections[2].element_id.as_deref(), Some("second"));
        assert!(matches!(
            projections[2].outcome,
            JsonLdProjectionOutcome::Projected(_)
        ));

        let contributions = json_ld_contributions(&blocks, &ContextCache::new());

        assert_eq!(contributions.len(), 2);
        assert_eq!(subject_id(&contributions[0]), "https://page.test/first");
        assert_eq!(subject_id(&contributions[1]), "https://page.test/second");
        assert_eq!(
            contributions[0]
                .nodes
                .iter()
                .find(|node| node.id == "https://page.test/first")
                .and_then(|node| node.title.as_deref()),
            Some("First")
        );
    }

    #[test]
    fn resolves_only_contexts_supplied_by_the_offline_cache() {
        const CONTEXT_URL: &str = "https://contexts.test/article-v1";
        const CONTEXT: &[u8] = br#"{"@context":{"name":"https://schema.org/name"}}"#;
        let document = StaticDocument::parse(&format!(
            r#"<script type="application/ld+json">
              {{"@context":"{CONTEXT_URL}",
               "@id":"https://page.test/cached","name":"Cached"}}
            </script>"#
        ));
        let blocks = extract_json_ld_blocks(&document);

        let missing = project_json_ld_blocks(&blocks, &ContextCache::new());
        assert!(matches!(
            missing[0].outcome,
            JsonLdProjectionOutcome::ExpansionFailed(_)
        ));
        assert!(json_ld_contributions(&blocks, &ContextCache::new()).is_empty());

        let contexts = ContextCache::new().with(CONTEXT_URL, CONTEXT);
        let contributions = json_ld_contributions(&blocks, &contexts);
        assert_eq!(contributions.len(), 1);
        assert_eq!(subject_id(&contributions[0]), "https://page.test/cached");
        assert_eq!(
            contributions[0]
                .nodes
                .iter()
                .find(|node| node.id == "https://page.test/cached")
                .and_then(|node| node.title.as_deref()),
            Some("Cached")
        );
    }

    #[test]
    fn resolves_relative_ids_against_the_caller_owned_document_base() {
        // W3C JSON-LD ToRDF test #t0017, "Relative IRI expands relative
        // resource location":
        // https://w3c.github.io/json-ld-api/tests/toRdf-manifest.jsonld#t0017
        // Original input:
        // https://w3c.github.io/json-ld-api/tests/toRdf/0017-in.jsonld
        // Copyright © W3C; distributed under the W3C Test Suite License:
        // https://www.w3.org/Consortium/Legal/2008/04-testsuite-license.html
        // Keep the tiny authoritative input inline so this regression remains
        // offline and does not vendor the full suite.
        const W3C_T0017_INPUT: &str = r#"{
          "@id": "a/b",
          "@type": "http://www.w3.org/2000/01/rdf-schema#Resource"
        }"#;
        const DOCUMENT_BASE: &str = "https://w3c.github.io/json-ld-api/tests/toRdf/0017-in.jsonld";
        const EXPECTED_ID: &str = "https://w3c.github.io/json-ld-api/tests/toRdf/a/b";

        let document = StaticDocument::parse(&format!(
            r#"<script id="relative" type="application/ld+json">{W3C_T0017_INPUT}</script>"#
        ));
        let blocks = extract_json_ld_blocks(&document);

        let without_base = project_json_ld_blocks(&blocks, &ContextCache::new());
        let JsonLdProjectionOutcome::Projected(without_base) = &without_base[0].outcome else {
            panic!("the relative-only node is ignored when no base is supplied");
        };
        assert!(without_base.nodes.is_empty());

        let invalid_base = project_json_ld_blocks_with_base_iri(
            &blocks,
            &ContextCache::new(),
            Some("relative/base"),
        );
        assert!(matches!(
            invalid_base[0].outcome,
            JsonLdProjectionOutcome::ExpansionFailed(_)
        ));

        let projections = project_json_ld_blocks_with_base_iri(
            &blocks,
            &ContextCache::new(),
            Some(DOCUMENT_BASE),
        );
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].element_id.as_deref(), Some("relative"));
        assert_eq!(projections[0].declared_type, "application/ld+json");
        assert_eq!(projections[0].dom_text, blocks[0].dom_text);
        let JsonLdProjectionOutcome::Projected(contribution) = &projections[0].outcome else {
            panic!("the W3C relative-IRI fixture must project with the supplied base");
        };
        assert!(contribution.nodes.iter().any(|node| {
            node.id == EXPECTED_ID
                && node.types == ["http://www.w3.org/2000/01/rdf-schema#Resource"]
        }));

        let contributions =
            json_ld_contributions_with_base_iri(&blocks, &ContextCache::new(), Some(DOCUMENT_BASE));
        assert_eq!(contributions.len(), 1);
        assert!(
            contributions[0]
                .nodes
                .iter()
                .any(|node| node.id == EXPECTED_ID)
        );
    }
}
