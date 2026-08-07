use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use tree_sitter::{Language, Node, Query, QueryCursor, StreamingIterator as _, Tree};
use uuid::Uuid;

use crate::graph::{
    SymbolKind, Visibility,
    grammer::Grammer,
    relationship::{Relationship, RelationshipKind, RelationshipTarget},
    symbol::{Location, Symbol},
};

pub(crate) struct SymbolData {
    pub(crate) tree: Tree,
    pub(crate) content: String,
    pub(crate) origin: PathBuf,
    pub(crate) grammer: Box<dyn Grammer>,
}

pub(crate) fn handle_query<F: FnMut(&Node, &str, String)>(
    lang: &Language,
    query: &str,
    node: &Node,
    content: &str,
    mut on_capture: F,
) {
    let q = Query::new(lang, query).unwrap();
    let mut qc = QueryCursor::new();
    let mut matches = qc.matches(&q, *node, content.as_bytes());
    while let Some(m) = matches.next() {
        for cap in m.captures.iter() {
            let cap_name = q.capture_names()[cap.index as usize];
            let value = cap.node.utf8_text(content.as_bytes()).unwrap();
            on_capture(&cap.node, cap_name, value.to_string());
        }
    }
}

fn build_symbols_and_relationships(
    grammer: &Box<dyn Grammer>,
    node: &Node,
    content: &str,
    file: &Path,
    relationships: &mut Vec<Relationship>,
    symbols: &mut Vec<Symbol>,
    default_visibility: &mut Option<Visibility>,
) -> Uuid {
    let mut symbol = Symbol::default();
    symbol.id = Uuid::new_v4();
    symbol.file = file.to_path_buf();
    symbol.location = Location::from((node.start_position(), node.end_position()));

    // MARK: GET SYMBOL KIND
    symbol.kind = grammer.to_symbolkind(node, content);

    if symbol.kind != SymbolKind::Root {
        // MARK: GET SYMBOL NAME
        symbol.name = grammer.to_name(node, content).to_string();

        // MARK: GET SYMBOL MODIFIERS
        let (v, m) = grammer.extract_declaration_attributes(node, content);
        if let Some(def) = default_visibility {
            symbol.visibility = def.clone();
        } else {
            symbol.visibility = v;
        }
        symbol.modifier = m;

        // MARK: GET GENERIC PARAMETERS
        symbol.generics = grammer.to_generics(node, content);

        // MARK: GET RELATIONSHIPS
        if let Some(interfaces) = grammer.gather_all_inheritance(node, content) {
            for i in interfaces {
                relationships.push(Relationship {
                    from: symbol.id,
                    to: RelationshipTarget::Unresolved(i.trim().to_string()),
                    kind: RelationshipKind::Inherits,
                    metadata: HashMap::new(),
                });
            }
        }
        if let Some(permits) = grammer.gather_all_permits(node, content) {
            for p in permits {
                relationships.push(Relationship {
                    from: symbol.id,
                    to: RelationshipTarget::Unresolved(p.trim().to_string()),
                    kind: RelationshipKind::Permits,
                    metadata: HashMap::new(),
                });
            }
        }

        // MARK: GET GRAMMER SPECIFIC METADATA
        grammer.extract_metadata(node, content, &mut symbol.metadata);
    }

    // GET CHILD DECLARATION
    collect_symbols_recursively(
        node,
        symbol.id,
        grammer,
        file,
        content,
        relationships,
        symbols,
        default_visibility,
    );

    let id = symbol.id;
    symbols.push(symbol);

    id
}

fn collect_symbols_recursively(
    node: &Node,
    parent_id: Uuid,
    grammer: &Box<dyn Grammer>,
    file: &Path,
    content: &str,
    relationships: &mut Vec<Relationship>,
    symbols: &mut Vec<Symbol>,
    default_visibility: &mut Option<Visibility>,
) {
    let mut scoped_visiblity = default_visibility;
    for child in node.children(&mut node.walk()) {
        if grammer.declaration_nodes().contains(&child.kind()) {
            let child_id = build_symbols_and_relationships(
                grammer,
                &child,
                content,
                file,
                relationships,
                symbols,
                &mut scoped_visiblity,
            );
            relationships.push(Relationship {
                from: parent_id,
                to: RelationshipTarget::Resolved(child_id),
                kind: RelationshipKind::Contains,
                metadata: HashMap::new(),
            });
        } else if grammer.flatten_nodes().contains(&child.kind()) {
            // Flatten body {} so we get
            // directly to declarations
            collect_symbols_recursively(
                &child,
                parent_id,
                grammer,
                file,
                content,
                relationships,
                symbols,
                &mut scoped_visiblity,
            )
        } else {
            // println!("Node: {}", child);
            grammer.apply_visibility_change(&child, content, &mut scoped_visiblity)
        }
    }
}

/// Graph represents the symbology
/// of all languages unified
pub(crate) struct Graph {
    symbols: Vec<Symbol>,
    relationships: Vec<Relationship>,
}

impl Graph {
    pub(crate) fn new(data: Vec<SymbolData>) -> Graph {
        let mut symbols = Vec::new();
        let mut relationships = Vec::new();
        for sym in data {
            build_symbols_and_relationships(
                &sym.grammer,
                &sym.tree.root_node(),
                &sym.content,
                &sym.origin,
                &mut relationships,
                &mut symbols,
                &mut None,
            );
        }
        Graph {
            symbols,
            relationships,
        }
    }

    pub fn symbols(&self) -> &Vec<Symbol> {
        &self.symbols
    }
    pub fn relationships(&self) -> &Vec<Relationship> {
        &self.relationships
    }
}
