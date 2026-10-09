// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Composition between Fleece's lossless HTML evidence and Mere's JSON-LD
//! processor.

use fleece::{EmbeddedJsonLdBlock, JsonLdParseStatus};
use linked_data::{ContextCache, GraphContribution, ImportEnvelope};

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

/// One source block with the parser evidence needed for an exact RDF import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonLdImportBlockProjection {
    pub document_order: u64,
    pub element_id: Option<String>,
    pub declared_type: String,
    pub dom_text: String,
    pub outcome: JsonLdImportOutcome,
}

/// The result of retaining an import envelope for one preserved JSON-LD block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonLdImportOutcome {
    InvalidJson,
    Projected(ImportEnvelope),
    ExpansionFailed(String),
}

/// Retain each block's source identity, errors and exact RDF import evidence.
pub fn project_json_ld_import_blocks(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
) -> Vec<JsonLdImportBlockProjection> {
    project_json_ld_import_blocks_with_base_iri(blocks, contexts, None)
}

/// Resolve relative RDF IRIs against the caller's document URL, retaining import evidence.
pub fn project_json_ld_import_blocks_with_base_iri(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
    base_iri: Option<&str>,
) -> Vec<JsonLdImportBlockProjection> {
    blocks
        .iter()
        .map(|block| {
            let outcome = if !matches!(&block.parse, JsonLdParseStatus::Parsed(_)) {
                JsonLdImportOutcome::InvalidJson
            } else {
                match linked_data::from_jsonld_envelope_with_contexts_and_base_iri(
                    block.dom_text.as_bytes(),
                    contexts.clone(),
                    base_iri,
                ) {
                    Ok(envelope) => JsonLdImportOutcome::Projected(envelope),
                    Err(error) => JsonLdImportOutcome::ExpansionFailed(error.to_string()),
                }
            };
            JsonLdImportBlockProjection {
                document_order: block.document_order,
                element_id: block.element_id.clone(),
                declared_type: block.declared_type.clone(),
                dom_text: block.dom_text.clone(),
                outcome,
            }
        })
        .collect()
}

/// Retain successful import envelopes in DOM order, using the existing best-effort policy.
pub fn json_ld_imports(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
) -> Vec<ImportEnvelope> {
    json_ld_imports_with_base_iri(blocks, contexts, None)
}

/// Like [`json_ld_imports`], resolving relative RDF IRIs against a caller-owned URL.
pub fn json_ld_imports_with_base_iri(
    blocks: &[EmbeddedJsonLdBlock],
    contexts: &ContextCache,
    base_iri: Option<&str>,
) -> Vec<ImportEnvelope> {
    project_json_ld_import_blocks_with_base_iri(blocks, contexts, base_iri)
        .into_iter()
        .filter_map(|projection| match projection.outcome {
            JsonLdImportOutcome::Projected(envelope) => Some(envelope),
            JsonLdImportOutcome::InvalidJson | JsonLdImportOutcome::ExpansionFailed(_) => None,
        })
        .collect()
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
        JsonLdImportOutcome, JsonLdProjectionOutcome, json_ld_contributions,
        json_ld_contributions_with_base_iri, json_ld_imports, json_ld_imports_with_base_iri,
        project_json_ld_blocks, project_json_ld_blocks_with_base_iri,
        project_json_ld_import_blocks, project_json_ld_import_blocks_with_base_iri,
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
        let imports = project_json_ld_import_blocks(&blocks, &ContextCache::new());
        assert_eq!(imports.len(), blocks.len());
        for (projected, source) in imports.iter().zip(&blocks) {
            assert_eq!(projected.document_order, source.document_order);
            assert_eq!(projected.element_id, source.element_id);
            assert_eq!(projected.declared_type, source.declared_type);
            assert_eq!(projected.dom_text, source.dom_text);
        }
        assert_eq!(imports[1].outcome, JsonLdImportOutcome::InvalidJson);
        let envelopes = json_ld_imports(&blocks, &ContextCache::new());
        assert_eq!(envelopes.len(), 2);
        for (envelope, contribution) in envelopes.iter().zip(&contributions) {
            assert_eq!(envelope.contribution(), contribution);
            assert_eq!(envelope.clone().into_contribution(), *contribution);
        }
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
        let missing_imports = project_json_ld_import_blocks(&blocks, &ContextCache::new());
        assert!(matches!(
            missing_imports[0].outcome,
            JsonLdImportOutcome::ExpansionFailed(_)
        ));
        assert!(json_ld_imports(&blocks, &ContextCache::new()).is_empty());
        let envelopes = json_ld_imports(&blocks, &contexts);
        assert_eq!(envelopes.len(), 1);
        assert_eq!(envelopes[0].contribution(), &contributions[0]);
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
        let import_without_base = project_json_ld_import_blocks(&blocks, &ContextCache::new());
        let JsonLdImportOutcome::Projected(import_without_base) = &import_without_base[0].outcome
        else {
            panic!("relative-only input without a base still parses");
        };
        assert!(import_without_base.contribution().nodes.is_empty());
        let invalid_import_base = project_json_ld_import_blocks_with_base_iri(
            &blocks,
            &ContextCache::new(),
            Some("relative/base"),
        );
        assert!(matches!(
            invalid_import_base[0].outcome,
            JsonLdImportOutcome::ExpansionFailed(_)
        ));
        let import_projections = project_json_ld_import_blocks_with_base_iri(
            &blocks,
            &ContextCache::new(),
            Some(DOCUMENT_BASE),
        );
        assert_eq!(import_projections[0].dom_text, blocks[0].dom_text);
        assert_eq!(
            import_projections[0].element_id.as_deref(),
            Some("relative")
        );
        assert_eq!(import_projections[0].declared_type, "application/ld+json");
        assert_eq!(
            import_projections[0].document_order,
            blocks[0].document_order
        );
        let imports =
            json_ld_imports_with_base_iri(&blocks, &ContextCache::new(), Some(DOCUMENT_BASE));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].contribution(), &contributions[0]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn imports_explicit_assertions_and_complete_tag_definitions_from_dom_text() {
        let document = StaticDocument::parse(
            r#"<script id="profile" type="application/ld+json">[
              {"@id":"urn:document:page",
               "urn:document:related":[{"@id":"urn:document:other"}],
               "https://mere.computer/ns/rel#taggedWith":[{"@id":"urn:document:tag"}]},
              {"@id":"urn:document:tag",
               "@type":["http://www.w3.org/2004/02/skos/core#Concept"],
               "http://www.w3.org/2004/02/skos/core#prefLabel":[{"@value":"Retained"}],
               "http://www.w3.org/ns/prov#wasAttributedTo":[{"@id":"urn:document:owner"}]},
              {"@id":"urn:document:incomplete-tag",
               "@type":["http://www.w3.org/2004/02/skos/core#Concept"],
               "http://www.w3.org/2004/02/skos/core#prefLabel":[{"@value":"Descriptive"}]},
              {"@id":"urn:mere:statement:document-related",
               "@type":["http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement"],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#subject":[{"@id":"urn:document:page"}],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate":[{"@id":"urn:document:related"}],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#object":[{"@id":"urn:document:other"}]},
              {"@id":"urn:mere:statement:document-tag",
               "@type":["http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement"],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#subject":[{"@id":"urn:document:page"}],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate":[{"@id":"https://mere.computer/ns/rel#taggedWith"}],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#object":[{"@id":"urn:document:tag"}]},
              {"@id":"urn:mere:statement:document-tag-label",
               "@type":["http://www.w3.org/1999/02/22-rdf-syntax-ns#Statement"],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#subject":[{"@id":"urn:document:tag"}],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#predicate":[{"@id":"http://www.w3.org/2004/02/skos/core#prefLabel"}],
               "http://www.w3.org/1999/02/22-rdf-syntax-ns#object":[{"@value":"Retained"}]}
            ]</script>"#,
        );
        let blocks = extract_json_ld_blocks(&document);
        let projections = project_json_ld_import_blocks(&blocks, &ContextCache::new());
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].element_id.as_deref(), Some("profile"));
        assert_eq!(projections[0].dom_text, blocks[0].dom_text);
        let JsonLdImportOutcome::Projected(envelope) = &projections[0].outcome else {
            panic!("the complete profile must retain an import envelope");
        };
        for id in ["document-related", "document-tag"] {
            let edge = envelope
                .contribution()
                .edges
                .iter()
                .find(|edge| edge.statement_id.as_deref() == Some(id))
                .expect("explicit metadata-free assertion is carried");
            assert!(edge.label.is_none());
            assert!(edge.provenance_iri.is_none());
            assert!(edge.asserted_at_ms.is_none());
        }
        let mut graph = Default::default();
        let applied = linked_data::apply_import(&mut graph, envelope);
        assert!(applied.nodes_created > 0);
        let find_resource = |iri| {
            graph
                .resource_nodes()
                .find(|resource| resource.canonical_iri() == iri)
                .expect("prepared RDF resource")
                .id()
        };
        let page = find_resource("urn:document:page");
        let tag = find_resource("urn:document:tag");
        let concept = graph
            .resource_tag_concept(tag)
            .expect("complete concept definition survives the adapter");
        assert_eq!(concept.owner_iri, "urn:document:owner");
        assert_eq!(concept.label, "Retained");
        let tag_properties = graph.resource_properties(tag);
        assert_eq!(
            tag_properties.len(),
            1,
            "the definition adds no anonymous label assertion"
        );
        assert_eq!(tag_properties[0].statement_id, "document-tag-label");
        assert_eq!(
            tag_properties[0].predicate,
            "http://www.w3.org/2004/02/skos/core#prefLabel"
        );
        assert_eq!(tag_properties[0].value, "Retained");
        assert!(tag_properties[0].asserted_at_ms.is_none());
        assert!(
            !graph.resource_edges().any(|(from, _, payload)| {
                from.id() == tag
                    && payload
                        .semantic_statements()
                        .iter()
                        .any(|claim| claim.predicate == "http://www.w3.org/ns/prov#wasAttributedTo")
            }),
            "the plain concept owner is definition evidence"
        );
        assert!(
            graph
                .resource_tag_concept(find_resource("urn:document:incomplete-tag"))
                .is_none()
        );
        assert_eq!(graph.resource_tag_labels(page).len(), 1);
        assert!(graph.resource_tag_labels(page).contains("Retained"));
        for (id, predicate) in [
            ("document-related", "urn:document:related"),
            ("document-tag", "https://mere.computer/ns/rel#taggedWith"),
        ] {
            let claims: Vec<_> = graph
                .resource_edges()
                .filter(|(from, _, _)| from.id() == page)
                .flat_map(|(_, _, payload)| payload.semantic_statements())
                .filter(|claim| claim.statement_id == id)
                .collect();
            assert_eq!(claims.len(), 1);
            assert_eq!(claims[0].predicate, predicate);
            assert!(claims[0].label.is_none());
            assert!(claims[0].asserted_at_ms.is_none());
        }
        let first = graph.to_snapshot();
        linked_data::apply_import(&mut graph, envelope);
        let repeated = graph.to_snapshot();
        assert_eq!(repeated.resource_edges, first.resource_edges);
        assert_eq!(repeated.resources, first.resources);
        assert_eq!(repeated.shown_resources, first.shown_resources);
    }
}
