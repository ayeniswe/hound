use std::collections::HashMap;

use crate::graph::symbol::{Symbol, SymbolId};

pub(crate) type Nodes = Vec<Symbol>;
pub(crate) type Edges = Vec<Relationship>;

#[derive(Clone, Default, Debug)]
pub(crate) enum RelationshipKind {
    #[default]
    Contains,
    Calls,
    Inherits,
    Imports,
    Permits,
}

/// Relationship creates an edge from one Symbol
/// to another
#[derive(Clone, Debug)]
pub(crate) struct Relationship {
    pub(crate) from: SymbolId,
    pub(crate) to: RelationshipTarget,
    pub(crate) kind: RelationshipKind,
    pub(crate) metadata: HashMap<String,String>,
}

/// RelationshipTarget holds a future
/// reference to resolve in cases
/// resolution is not immediate
#[derive(Clone, Debug)]
pub(crate) enum RelationshipTarget {
    Resolved(SymbolId),
    Unresolved(String),
}
impl Relationship {
    /// Set the relationship as resolved
    ///
    /// Note: If relationship is already resolved
    /// the function is `noOp`
    pub(crate) fn resolve(&mut self, id: SymbolId) {
        if matches!(self.to, RelationshipTarget::Resolved(_)) {
            return;
        }
        self.to = RelationshipTarget::Resolved(id);
    }

    /// Returns the name of the unresolved symbol, otherwise
    /// `None`
    pub(crate) fn is_unresolved(&self) -> Option<&str> {
        match &self.to {
            RelationshipTarget::Resolved(_) => None,
            RelationshipTarget::Unresolved(name) => Some(name),
        }
    }
}
