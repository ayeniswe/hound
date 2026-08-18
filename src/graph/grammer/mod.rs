pub(crate) mod cpp;
pub(crate) mod java;

use std::collections::HashMap;

use tree_sitter::Node;

use crate::graph::{
    Modifier, SymbolKind, Visibility,
    build::{Relationships, ScopeIndexTable},
    symbol::{Generic, Language, Scope, SymbolId},
};

/// The Grammer trait abstracts the query language tokens
pub(crate) trait Grammer {
    fn declaration_nodes(&self) -> &'static [&'static str];
    fn flatten_nodes(&self) -> &'static [&'static str];
    fn to_symbolkind(&self, node: &Node, content: &str) -> SymbolKind;
    fn to_name(&self, node: &Node, content: &str) -> String;
    fn to_scope(&self, node: &Node, content: &str) -> Scope;
    fn extract_declaration_attributes(
        &self,
        node: &Node,
        content: &str,
    ) -> (Visibility, Vec<Modifier>);
    fn to_generics(&self, node: &Node, content: &str) -> Vec<Generic>;
    fn gather_all_permits<'a>(&self, node: &Node, content: &'a str) -> Option<Vec<&'a str>> {
        None
    }
    fn gather_all_inheritance<'a>(&self, node: &Node, content: &'a str) -> Option<Vec<&'a str>> {
        None
    }
    fn language(&self) -> Language;
    fn tree_language(&self) -> tree_sitter::Language;
    fn apply_visibility_change(
        &self,
        node: &Node,
        content: &str,
        scoped_vis: &mut Option<Visibility>,
    ) {
    }

    fn extract_metadata(&self, node: &Node, content: &str, metadata: &mut HashMap<String, String>);
    fn pair_relationships(
        &self,
        node: &Node,
        parent_id: SymbolId,
        child_id: SymbolId,
        relationships: &mut Relationships,
    );
    fn try_resolve_scope(
        &self,
        node: &Node,
        content: &str,
        table: &mut ScopeIndexTable,
        local_scope: &Scope,
        base_types: &mut Vec<String>,
        local_imports: &mut Vec<Scope>,
    ) -> Scope;
}
