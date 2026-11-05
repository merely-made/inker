/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

fn named_node(iri: &str) -> NamedNode {
    NamedNode::new(iri).expect("fixture IRI")
}

fn blank_node(id: &str) -> BlankNode {
    BlankNode::new(id).expect("fixture blank node")
}

fn collect_blank_nodes(dataset: &Dataset) -> Vec<BlankNode> {
    let mut nodes = HashSet::new();
    for quad in dataset.iter().map(|quad| quad.into_owned()) {
        if let NamedOrBlankNode::BlankNode(node) = quad.subject {
            nodes.insert(node);
        }
        if let Term::BlankNode(node) = quad.object {
            nodes.insert(node);
        }
        if let GraphName::BlankNode(node) = quad.graph_name {
            nodes.insert(node);
        }
    }
    nodes.into_iter().collect()
}

fn map_subject(
    subject: &NamedOrBlankNode,
    mapping: &HashMap<BlankNode, BlankNode>,
) -> NamedOrBlankNode {
    match subject {
        NamedOrBlankNode::BlankNode(node) => mapping
            .get(node)
            .cloned()
            .unwrap_or_else(|| node.clone())
            .into(),
        NamedOrBlankNode::NamedNode(node) => node.clone().into(),
    }
}

fn map_term(term: &Term, mapping: &HashMap<BlankNode, BlankNode>) -> Term {
    match term {
        Term::BlankNode(node) => mapping
            .get(node)
            .cloned()
            .unwrap_or_else(|| node.clone())
            .into(),
        _ => term.clone(),
    }
}

fn map_graph(graph: &GraphName, mapping: &HashMap<BlankNode, BlankNode>) -> GraphName {
    match graph {
        GraphName::BlankNode(node) => mapping
            .get(node)
            .cloned()
            .unwrap_or_else(|| node.clone())
            .into(),
        _ => graph.clone(),
    }
}

fn mapped_quad(quad: &Quad, mapping: &HashMap<BlankNode, BlankNode>) -> Quad {
    Quad::new(
        map_subject(&quad.subject, mapping),
        quad.predicate.clone(),
        map_term(&quad.object, mapping),
        map_graph(&quad.graph_name, mapping),
    )
}

fn isomorphic_dataset(expected: &Dataset, actual: &Dataset) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let expected_quads: Vec<_> = expected.iter().map(|quad| quad.into_owned()).collect();
    let actual_quads: HashSet<_> = actual.iter().map(|quad| quad.into_owned()).collect();
    let expected_nodes = collect_blank_nodes(expected);
    let actual_nodes = collect_blank_nodes(actual);
    if expected_nodes.len() != actual_nodes.len() {
        return false;
    }

    fn search(
        index: usize,
        expected_nodes: &[BlankNode],
        actual_nodes: &[BlankNode],
        used: &mut [bool],
        mapping: &mut HashMap<BlankNode, BlankNode>,
        expected_quads: &[Quad],
        actual_quads: &HashSet<Quad>,
    ) -> bool {
        if index == expected_nodes.len() {
            return expected_quads
                .iter()
                .map(|quad| mapped_quad(quad, mapping))
                .all(|quad| actual_quads.contains(&quad));
        }
        for actual_index in 0..actual_nodes.len() {
            if used[actual_index] {
                continue;
            }
            used[actual_index] = true;
            mapping.insert(
                expected_nodes[index].clone(),
                actual_nodes[actual_index].clone(),
            );
            if search(
                index + 1,
                expected_nodes,
                actual_nodes,
                used,
                mapping,
                expected_quads,
                actual_quads,
            ) {
                return true;
            }
            mapping.remove(&expected_nodes[index]);
            used[actual_index] = false;
        }
        false
    }

    search(
        0,
        &expected_nodes,
        &actual_nodes,
        &mut vec![false; actual_nodes.len()],
        &mut HashMap::new(),
        &expected_quads,
        &actual_quads,
    )
}

#[test]
fn annotation_json_ld_expands_to_expected_rdf_dataset_offline() {
    let record = fixture();
    let json_ld = record.annotation_json_ld().expect("serialize JSON-LD");
    let actual: Dataset = JsonLdParser::new()
        .for_slice(&json_ld)
        .with_load_document_callback(|url, _options| {
            if url == "http://www.w3.org/ns/anno.jsonld" {
                Ok(JsonLdRemoteDocument {
                    document: W3C_ANNOTATION_CONTEXT.as_bytes().to_vec(),
                    document_url: url.to_owned(),
                })
            } else {
                Err::<JsonLdRemoteDocument, _>(
                    format!("offline test rejected remote @context: {url}").into(),
                )
            }
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("expand JSON-LD with the offline W3C context")
        .into_iter()
        .collect();

    let annotation = named_node(&record.annotation.id);
    let source = named_node(&record.target.source.id);
    let scope = named_node(&record.target.scope);
    let target = blank_node("target");
    let position = blank_node("position");
    let quote = blank_node("quote");
    let fragment = blank_node("fragment");
    let oa = "http://www.w3.org/ns/oa#";
    let rdf_type = named_node("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let default_graph = GraphName::DefaultGraph;
    let position_selector: TextPositionSelector =
        serde_json::from_value(record.target.selector[0].clone()).expect("position selector");
    let quote_selector: TextQuoteSelector =
        serde_json::from_value(record.target.selector[1].clone()).expect("quote selector");
    let fragment_selector: FragmentSelector =
        serde_json::from_value(record.target.selector[2].clone()).expect("fragment selector");
    let expected: Dataset = vec![
        Quad::new(
            annotation.clone(),
            rdf_type.clone(),
            named_node(&format!("{oa}Annotation")),
            default_graph.clone(),
        ),
        Quad::new(
            annotation,
            named_node(&format!("{oa}hasTarget")),
            target.clone(),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            rdf_type.clone(),
            named_node(&format!("{oa}SpecificResource")),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node(&format!("{oa}hasSource")),
            source.clone(),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node(&format!("{oa}hasScope")),
            scope,
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node(&format!("{oa}hasSelector")),
            position.clone(),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node(&format!("{oa}hasSelector")),
            quote.clone(),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node(&format!("{oa}hasSelector")),
            fragment.clone(),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node("https://merely-made.org/ns/eidetic#captureHash"),
            Literal::new_simple_literal(record.target.capture_hash.to_string()),
            default_graph.clone(),
        ),
        Quad::new(
            target.clone(),
            named_node("https://merely-made.org/ns/eidetic#captureEvidenceHash"),
            Literal::new_simple_literal(
                record
                    .target
                    .capture_evidence_hash
                    .expect("fixture has capture evidence")
                    .to_string(),
            ),
            default_graph.clone(),
        ),
        Quad::new(
            target,
            named_node("https://merely-made.org/ns/eidetic#canonicalTextHash"),
            Literal::new_simple_literal(record.target.canonical_text_hash.to_string()),
            default_graph.clone(),
        ),
        Quad::new(
            source,
            named_node("http://purl.org/dc/elements/1.1/format"),
            Literal::new_simple_literal(record.target.source.format.clone()),
            default_graph.clone(),
        ),
        Quad::new(
            position.clone(),
            rdf_type.clone(),
            named_node(&format!("{oa}TextPositionSelector")),
            default_graph.clone(),
        ),
        Quad::new(
            position.clone(),
            named_node(&format!("{oa}start")),
            Literal::new_typed_literal(
                position_selector.start.to_string(),
                named_node("http://www.w3.org/2001/XMLSchema#nonNegativeInteger"),
            ),
            default_graph.clone(),
        ),
        Quad::new(
            position,
            named_node(&format!("{oa}end")),
            Literal::new_typed_literal(
                position_selector.end.to_string(),
                named_node("http://www.w3.org/2001/XMLSchema#nonNegativeInteger"),
            ),
            default_graph.clone(),
        ),
        Quad::new(
            quote.clone(),
            rdf_type.clone(),
            named_node(&format!("{oa}TextQuoteSelector")),
            default_graph.clone(),
        ),
        Quad::new(
            quote.clone(),
            named_node(&format!("{oa}exact")),
            Literal::new_simple_literal(quote_selector.exact),
            default_graph.clone(),
        ),
        Quad::new(
            quote.clone(),
            named_node(&format!("{oa}prefix")),
            Literal::new_simple_literal(quote_selector.prefix),
            default_graph.clone(),
        ),
        Quad::new(
            quote,
            named_node(&format!("{oa}suffix")),
            Literal::new_simple_literal(quote_selector.suffix),
            default_graph.clone(),
        ),
        Quad::new(
            fragment.clone(),
            rdf_type,
            named_node(&format!("{oa}FragmentSelector")),
            default_graph.clone(),
        ),
        Quad::new(
            fragment.clone(),
            named_node("http://www.w3.org/1999/02/22-rdf-syntax-ns#value"),
            Literal::new_simple_literal(fragment_selector.value),
            default_graph.clone(),
        ),
        Quad::new(
            fragment,
            named_node("http://purl.org/dc/terms/conformsTo"),
            named_node(&fragment_selector.conforms_to),
            default_graph,
        ),
    ]
    .into_iter()
    .collect();

    assert!(
        isomorphic_dataset(&expected, &actual),
        "expanded JSON-LD dataset differed from the expected W3C Annotation graph"
    );
}
