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
    symbol::{Location, Symbol, SymbolId},
};

pub(crate) type SymbolMap = HashMap<SymbolId, Symbol>;
pub(crate) type Relationships = Vec<Relationship>;

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

type SymbolIndexKey = (String, SymbolKind);

pub(crate) fn build_symbols_and_relationships(
    grammer: &Box<dyn Grammer>,
    node: &Node,
    content: &str,
    file: &Path,
    relationships: &mut Relationships,
    symbols: &mut SymbolMap,
    index: &mut HashMap<SymbolIndexKey, SymbolId>,
    default_visibility: &mut Option<Visibility>,
) -> Uuid {
    let mut symbol = Symbol::default();
    symbol.id = Uuid::new_v4();
    symbol.file = file.to_path_buf();
    symbol.language = grammer.language();
    symbol.location = vec![Location::from((node.start_position(), node.end_position()))];

    // MARK: GET SYMBOL KIND
    symbol.kind = grammer.to_symbolkind(node, content);

    // MARK: GET SYMBOL NAME
    symbol.name = if matches!(symbol.kind, SymbolKind::Module) {
        file.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        grammer.to_name(node, content).to_string()
    };

    // MARK: Handle index lookup to dedup
    if let Some(id) = index.get(&(symbol.name.clone(), symbol.kind.clone())) {
        let sym = symbols.get_mut(id).unwrap();
        match sym.kind {
            SymbolKind::FunctionCall => {
                // Location can be differ so track all
                sym.location.append(&mut symbol.location);
            }
            _ => (),
        }
        
        return *id;
    }

    if !matches!(symbol.kind, SymbolKind::Module) {
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
        index,
        default_visibility,
    );

    let id = symbol.id;
    index.insert((symbol.name.clone(), symbol.kind.clone()), id);
    symbols.insert(id, symbol);

    id
}

fn collect_symbols_recursively(
    node: &Node,
    parent_id: SymbolId,
    grammer: &Box<dyn Grammer>,
    file: &Path,
    content: &str,
    relationships: &mut Relationships,
    symbols: &mut SymbolMap,
    index: &mut HashMap<SymbolIndexKey, SymbolId>,
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
                index,
                &mut scoped_visiblity,
            );
            grammer.pair_relationships(&child, parent_id, child_id, relationships);
        } else if grammer.flatten_nodes().contains(&child.kind()) {
            // Flatten nodes so we get
            // directly to declarations
            collect_symbols_recursively(
                &child,
                parent_id,
                grammer,
                file,
                content,
                relationships,
                symbols,
                index,
                &mut scoped_visiblity,
            )
        } else {
            grammer.apply_visibility_change(&child, content, &mut scoped_visiblity)
        }
    }
}
