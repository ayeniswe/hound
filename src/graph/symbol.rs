use std::{
    collections::{HashMap, HashSet},
    ops::Add,
    path::PathBuf,
};
use tree_sitter::{Node, Point};
use uuid::Uuid;

pub(crate) type SymbolId = Uuid;

#[derive(Debug, Clone, Default, Copy)]
pub enum Language {
    #[default]
    Java,
    Cpp,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct Generic {
    pub(crate) name: String,
    pub(crate) bounds: Vec<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Parameter {
    pub(crate) type_specifier: String,
    pub(crate) modifiers: Vec<Modifier>,
    pub(crate) variadic: bool,
    pub(crate) name: String,
    pub(crate) reference: bool,
    pub(crate) pointer_depth: usize,
    pub(crate) default: Option<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Type {
    pub(crate) reference: bool,
    pub(crate) pointer_depth: usize,
    pub(crate) name: String,
}

impl<'a> From<(Node<'a>, &str)> for Type {
    fn from(v: (Node, &str)) -> Self {
        let (node, content) = v;
        Type {
            reference: false,
            pointer_depth: 0,
            name: content[node.start_byte()..node.end_byte()].to_string(),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FunctionDefinition {
    pub(crate) value: String,
    pub(crate) params: Vec<Parameter>,
    pub(crate) return_type: Type,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Constructor {
    pub(crate) value: String,
    pub(crate) params: Vec<Parameter>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MemberVariable {
    pub(crate) value: String,
    pub(crate) dtype: Type,
}

#[derive(Clone, Default, Debug)]
pub(crate) enum SymbolKind {
    Destructor(String),
    Constructor(Constructor),

    Class,
    Struct,
    Interface,
    Trait,
    Enum,

    FunctionDefinition(FunctionDefinition),
    MemberVariable(MemberVariable),
    FunctionCall,

    Module(Scope),
    Import(Scope),

    #[default]
    Unknown,
}
impl PartialEq for SymbolKind {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}
impl Eq for SymbolKind {}

impl std::hash::Hash for SymbolKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct Location {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}
impl From<(Point, Point)> for Location {
    fn from(v: (Point, Point)) -> Self {
        Self {
            start_line: v.0.row + 1,
            start_column: v.0.column + 1,
            end_line: v.1.row + 1,
            end_column: v.1.column + 1,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) enum Visibility {
    Public,
    Private,
    Protected,
    Package,
    #[default]
    Unknown,
}
impl From<&str> for Visibility {
    fn from(value: &str) -> Self {
        match value.to_lowercase() {
            val if val == "public".to_string() => Visibility::Public,
            val if val == "private".to_string() => Visibility::Private,
            val if val == "protected".to_string() => Visibility::Protected,
            val if val == "package".to_string() => Visibility::Protected,
            _ => Visibility::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Modifier {
    Static,
    Final,
    Abstract,
    Volatile,
    Constant,
    Virtual,
    Override,
    Special(String),
}
impl From<&str> for Modifier {
    fn from(value: &str) -> Self {
        match value {
            "const" => Modifier::Constant,
            "static" => Modifier::Static,
            "abstract" => Modifier::Abstract,
            "virtual" => Modifier::Virtual,
            "ovveride" => Modifier::Override,
            "volatile" => Modifier::Volatile,
            "final" => Modifier::Final,
            _ => Modifier::Special(value.into()),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Scope {
    pub(crate) scopes: Vec<String>,
    pub(crate) wildcard: bool,
    pub(crate) is_static: bool,
}
#[derive(Clone, Default, Debug)]
pub(crate) struct Symbol {
    pub(crate) id: SymbolId,

    // What is it called?
    pub(crate) name: String,

    // Function, class, struct, etc.
    pub(crate) kind: SymbolKind,

    // Where does it live?
    pub(crate) file: PathBuf,
    pub(crate) location: Vec<Location>,

    // Context
    pub(crate) visibility: Visibility,
    pub(crate) modifier: Vec<Modifier>,
    pub(crate) scope: Scope,

    pub(crate) generics: Vec<Generic>,

    // Language-specific extras
    pub(crate) metadata: HashMap<String, String>,
    pub(crate) language: Language,
}
