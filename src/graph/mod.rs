pub(in crate::graph) mod build;
pub(in crate::graph) mod grammer;
pub(in crate::graph) mod parser;
pub(in crate::graph) mod relationship;
pub(in crate::graph) mod symbol;

pub(crate) use build::Graph;
pub(crate) use symbol::Modifier;
pub(crate) use symbol::SymbolKind;
pub(crate) use symbol::Visibility;

use std::path::Path;

use thiserror::Error;

use crate::graph::grammer::cpp::{Cpp, CppError};
use crate::graph::grammer::java::{Java, JavaError};
use crate::graph::parser::LanguageParser;

/// Create graph of all symbols found and supported
/// in files list
pub fn create_graph(files: Vec<&Path>) -> Result<Graph, GraphError> {
    let mut symbols = Vec::new();

    // Setup Parsers
    let cpp_parser = Cpp {};
    let java_parser = Java {};

    for file in files {
        let symbol = match file.extension() {
            Some(ext) => match ext.to_str() {
                // Some("rs") => Rust,
                // Some("py") => Python,
                Some("cpp") | Some("cx") | Some("cxx") => {
                    if let Ok(sym) = cpp_parser.parse(file) {
                        sym
                    } else {
                        continue;
                    }
                }
                Some("java") => {
                    if let Ok(sym) = java_parser.parse(file) {
                        sym
                    } else {
                        continue;
                    }
                }
                _ => continue, // Ignore unsupported
            },
            None => continue, // Ignore files missing extensions
        };
        symbols.push(symbol);
    }

    Ok(Graph::new(symbols))
}

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("{0}")]
    JavaError(#[from] JavaError),
    #[error("{0}")]
    CppError(#[from] CppError),
}
