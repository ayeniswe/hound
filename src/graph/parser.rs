use std::{fs, path::Path};

use thiserror::Error;
use tree_sitter::Parser;

use crate::graph::{build::SymbolData, grammer::Grammer};

pub(crate) trait LanguageParser: Clone
where
    Self: 'static,
{
    fn parse(&self, file: &Path) -> Result<SymbolData, ParserError>
    where
        Self: Grammer,
    {
        let mut parser = Parser::new();
        parser.set_language(&self.language().into()).unwrap();
        let content = fs::read_to_string(file)?;
        Ok(SymbolData {
            tree: parser.parse(&content, None).unwrap(),
            content,
            origin: file.to_path_buf(),
            grammer: Box::new(self.clone()),
        })
    }
}

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("{0}")]
    StdError(#[from] std::io::Error),
}
