pub(in crate::graph) mod build;
pub(in crate::graph) mod grammer;
pub(in crate::graph) mod parser;
pub(in crate::graph) mod query;
pub(in crate::graph) mod relationship;
pub(in crate::graph) mod symbol;
pub(crate) use symbol::Modifier;
pub(crate) use symbol::SymbolKind;
pub(crate) use symbol::Visibility;
// SHORT TEST EXPOSING NOT LONG TERM
pub(crate) use query::Direction;
pub(crate) use query::FindQuery;
pub(crate) use query::Query;
pub(crate) use query::QueryEngine;
pub(crate) use query::RelationshipQuery;
pub(crate) use query::SymbolKindQuery;
pub(crate) use relationship::RelationshipKind;
pub(crate) use relationship::RelationshipTarget;

use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

use crate::graph::build::Relationships;
use crate::graph::build::SymbolMap;
use crate::graph::build::ScopeIndexTable;
use crate::graph::build::build_symbols_and_relationships;
use crate::graph::grammer::cpp::{Cpp, CppError};
use crate::graph::grammer::java::{Java, JavaError};
use crate::graph::parser::LanguageParser;
use crate::graph::relationship::Relationship;
use crate::graph::symbol::Scope;
use crate::graph::symbol::Symbol;
use crate::graph::symbol::SymbolId;

/// Graph represents the symbology
/// of all languages unified
pub(crate) struct Graph {
    symbols: SymbolMap,
    relationships: Relationships,
}

impl Graph {
    /// Create graph of all symbols found and supported
    /// in files list
    pub(crate) fn create(files: Vec<PathBuf>) -> Result<Graph, GraphError> {
        let mut symbols = HashMap::new();
        let mut relationships = Vec::new();
        let mut index_table: ScopeIndexTable = ScopeIndexTable::default();

        for file in files {
            let parsed = match file.extension().and_then(|ext| ext.to_str()) {
                Some("cpp" | "cx" | "cxx") => Cpp.parse(&file),
                Some("java") => Java.parse(&file),
                _ => continue,
            };

            let Ok(parsed) = parsed else {
                continue;
            };

            build_symbols_and_relationships(
                &parsed.grammer,
                &parsed.tree.root_node(),
                &parsed.content,
                &parsed.origin,
                &mut relationships,
                &mut symbols,
                &mut index_table,
                &mut None,
            Scope::default()
            );
        }

        Ok(Graph {
            symbols,
            relationships,
        })
    }

    pub(crate) fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(&id)
    }

    pub(crate) fn symbols(&self) -> impl Iterator<Item = (&SymbolId, &Symbol)> {
        self.symbols.iter()
    }

    pub(crate) fn relationships(&self) -> impl Iterator<Item = &Relationship> {
        self.relationships.iter()
    }

    pub(crate) fn relationships_from(&self, id: SymbolId) -> impl Iterator<Item = &Relationship> {
        self.relationships
            .iter()
            .filter(move |relationship| relationship.from == id)
    }

    pub(crate) fn relationships_to(&self, id: SymbolId) -> impl Iterator<Item = &Relationship> {
        self.relationships.iter().filter(move |relationship| {
            matches!(
                relationship.to,
                RelationshipTarget::Resolved(target_id) if target_id == id
            )
        })
    }
}

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("{0}")]
    JavaError(#[from] JavaError),
    #[error("{0}")]
    CppError(#[from] CppError),
}
