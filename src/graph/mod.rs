pub(in crate::graph) mod build;
pub(in crate::graph) mod grammer;
pub(in crate::graph) mod parser;
pub(in crate::graph) mod relationship;
pub(in crate::graph) mod symbol;

pub(crate) use symbol::Modifier;
pub(crate) use symbol::SymbolKind;
pub(crate) use symbol::Visibility;

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

use crate::graph::build::Relationships;
use crate::graph::build::SymbolMap;
use crate::graph::build::build_symbols_and_relationships;
use crate::graph::grammer::cpp::{Cpp, CppError};
use crate::graph::grammer::java::{Java, JavaError};
use crate::graph::parser::LanguageParser;
use crate::graph::relationship::Relationship;
use crate::graph::relationship::RelationshipTarget;
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
    pub fn create(files: Vec<&Path>) -> Result<Graph, GraphError> {
        let cpp_parser = Cpp {};
        let java_parser = Java {};

        let mut symbols = HashMap::new();
        let mut relationships = Vec::new();

        for file in files {
            let parsed = match file.extension().and_then(|ext| ext.to_str()) {
                Some("cpp" | "cx" | "cxx") => cpp_parser.parse(file),
                Some("java") => java_parser.parse(file),
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
                &mut None,
            );
        }

        Ok(Graph {
            symbols,
            relationships,
        })
    }

    pub fn symbols(&self) -> &SymbolMap {
        &self.symbols
    }

    pub fn relationships(&self) -> &Vec<Relationship> {
        &self.relationships
    }

    pub fn write_tree<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;

        let mut visited = HashSet::new();

        for root in self.root_symbols() {
            self.write_node(&mut file, *root, 0, &mut visited)?;
        }

        Ok(())
    }

    fn root_symbols(&self) -> Vec<&SymbolId> {
        let mut has_parent = HashSet::new();

        for relationship in self.relationships() {
            if let RelationshipTarget::Resolved(id) = relationship.to {
                has_parent.insert(id);
            }
        }

        self.symbols()
            .into_iter()
            .filter_map(|(id, symbol)| {
                if !has_parent.contains(id) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    fn relationships_from(&self, id: SymbolId) -> impl Iterator<Item = &Relationship> {
        self.relationships
            .iter()
            .filter(move |relationship| relationship.from == id)
    }

    fn write_node(
        &self,
        file: &mut File,
        id: SymbolId,
        depth: usize,
        visited: &mut HashSet<SymbolId>,
    ) -> io::Result<()> {
        let Some(symbol) = self.symbols.get(&id) else {
            return Ok(());
        };

        // If we've already printed this node,
        // don't expand it again.
        if !visited.insert(id) {
            return Ok(());
        }

        let indent = "    ".repeat(depth);

        writeln!(file, "{}{:?}", indent, symbol)?;

        for relationship in self.relationships_from(id) {
            let edge_indent = "    ".repeat(depth + 1);

            match &relationship.to {
                RelationshipTarget::Resolved(target_id) => {
                    let already_visited = visited.contains(&target_id);

                    if let Some(target) = self.symbols.get(&target_id) {
                        if already_visited {
                            // The node exists elsewhere in the tree.
                            // Show the edge, but don't expand the node.
                            writeln!(
                                file,
                                "{}└── [{:?}] → [{:?}]",
                                edge_indent, relationship.kind, target
                            )?;
                        } else {
                            // New node: follow the edge and expand it.
                            writeln!(file, "{}└── [{:?}] →", edge_indent, relationship.kind)?;

                            self.write_node(file, *target_id, depth + 2, visited)?;
                        }
                    }
                }

                RelationshipTarget::Unresolved(name) => {
                    writeln!(
                        file,
                        "{}└── [{:?}] → {} [unresolved]",
                        edge_indent, relationship.kind, name
                    )?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("{0}")]
    JavaError(#[from] JavaError),
    #[error("{0}")]
    CppError(#[from] CppError),
}
