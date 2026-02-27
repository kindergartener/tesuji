use std::fmt::Display;

use anyhow::Result;
use pest_consume::{Parser, match_nodes};

use crate::sgf::{
    node::SGFProperty,
    tree::{GameTree, NodeId, TreeNode},
};

type Node<'i> = pest_consume::Node<'i, Rule, ()>;
type ParseResult<T> = std::result::Result<T, pest_consume::Error<Rule>>;

/// Recursive intermediate representation that mirrors the SGF grammar.
/// Private to this module — callers only see `GameTree`.
struct ParsedObject {
    nodes: Vec<Vec<SGFProperty>>,
    children: Vec<ParsedObject>,
}

#[derive(Parser)]
#[grammar = "sgf.pest"]
struct SGFParser;

#[pest_consume::parser]
impl SGFParser {
    fn EOI(_input: Node) -> ParseResult<()> {
        Ok(())
    }

    fn node_value(input: Node) -> ParseResult<String> {
        Ok(input.as_str().to_string())
    }

    fn prop_ident(input: Node) -> ParseResult<String> {
        Ok(input.as_str().to_string())
    }

    fn property(input: Node) -> ParseResult<SGFProperty> {
        let span = input.as_span();
        let err = |e: &dyn Display| to_parse_err(e, span.clone());
        let mut children = input.into_children();

        let ident_node = children.next().expect("Property must have prop_ident");
        let ident = Self::prop_ident(ident_node)?;

        let values: Vec<String> = children
            .map(|n| Self::node_value(n))
            .collect::<ParseResult<Vec<_>>>()?;
        let first_val = values.first().cloned().unwrap_or_default();

        Ok(match ident.as_str() {
            "AP" => SGFProperty::AP(first_val),
            "B"  => SGFProperty::B(first_val.parse().map_err(|e| err(&e))?),
            "W"  => SGFProperty::W(first_val.parse().map_err(|e| err(&e))?),
            "AB" => SGFProperty::AB(values.iter().filter_map(|v| v.parse().ok()).collect()),
            "AW" => SGFProperty::AW(values.iter().filter_map(|v| v.parse().ok()).collect()),
            "CA" => SGFProperty::CA(first_val.parse().map_err(|e| err(&e))?),
            "DT" => SGFProperty::DT(first_val),
            "FF" => SGFProperty::FF(first_val.parse().map_err(|e| err(&e))?),
            "GM" => SGFProperty::GM(first_val.parse().map_err(|e| err(&e))?),
            "KM" => SGFProperty::KM(first_val.parse().map_err(|e| err(&e))?),
            "SZ" => SGFProperty::SZ(first_val.parse().map_err(|e| err(&e))?),
            "PB" => SGFProperty::PB(first_val),
            "PW" => SGFProperty::PW(first_val),
            "RE" => SGFProperty::RE(first_val),
            "C"  => SGFProperty::C(first_val),
            _    => SGFProperty::Unknown(ident, values),
        })
    }

    fn node(input: Node) -> ParseResult<Vec<SGFProperty>> {
        match_nodes!(input.into_children();
            [property(props)..] => Ok(props.collect())
        )
    }

    fn object(input: Node) -> ParseResult<ParsedObject> {
        let mut nodes: Vec<Vec<SGFProperty>> = Vec::new();
        let mut children: Vec<ParsedObject> = Vec::new();

        for child in input.into_children() {
            match child.as_rule() {
                Rule::node   => nodes.push(Self::node(child)?),
                Rule::object => children.push(Self::object(child)?),
                _            => {}
            }
        }

        Ok(ParsedObject { nodes, children })
    }

    fn file(input: Node) -> ParseResult<Vec<ParsedObject>> {
        match_nodes!(input.into_children();
            [object(trees).., EOI(_)] => Ok(trees.collect())
        )
    }
}

fn to_parse_err(e: impl Display, span: pest::Span) -> pest_consume::Error<Rule> {
    pest_consume::Error::new_from_span(
        pest::error::ErrorVariant::CustomError { message: e.to_string() },
        span,
    )
}

// ---------------------------------------------------------------------------
// Arena ingestion — defined here because ParsedObject is private to this module
// ---------------------------------------------------------------------------

impl GameTree {
    /// Flatten a list of `ParsedObject`s into an arena-based `GameTree`.
    fn ingest(parsed_objects: Vec<ParsedObject>) -> Self {
        let mut tree = GameTree { nodes: Vec::new(), roots: Vec::new() };
        for parsed in parsed_objects {
            if let Some(root_id) = tree.ingest_object(parsed, None) {
                tree.roots.push(root_id);
            }
        }
        tree
    }

    /// Recursively insert one `ParsedObject` into the arena, linking nodes
    /// to `parent`. Returns the `NodeId` of the first node created (the root
    /// of this branch), or `None` if the object contained no nodes.
    fn ingest_object(&mut self, parsed: ParsedObject, parent: Option<NodeId>) -> Option<NodeId> {
        let mut first_id: Option<NodeId> = None;
        let mut last_id = parent;

        for props in parsed.nodes {
            let id = self.nodes.len();
            self.nodes.push(TreeNode {
                properties: props,
                parent: last_id,
                children: Vec::new(),
            });
            if let Some(p) = last_id {
                self.nodes[p].children.push(id);
            }
            first_id.get_or_insert(id);
            last_id = Some(id);
        }

        for child in parsed.children {
            self.ingest_object(child, last_id);
        }

        first_id
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn parse_sgf(input: &str) -> Result<GameTree> {
    let inputs = SGFParser::parse(Rule::file, input)?;
    let input = inputs.single()?;
    let parsed_objects = SGFParser::file(input)?;
    Ok(GameTree::ingest(parsed_objects))
}
