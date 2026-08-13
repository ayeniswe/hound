use crate::graph::{
    Graph, SymbolKind,
    SymbolKindQuery::Function,
    relationship::{RelationshipKind, RelationshipTarget},
    symbol::SymbolId,
};

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub symbols: Vec<SymbolResult>,
    pub relationships: Vec<RelationshipResult>,
}
impl QueryResult {
    pub(crate) fn new(symbols: Vec<SymbolResult>, relationships: Vec<RelationshipResult>) -> Self {
        Self {
            symbols,
            relationships,
        }
    }
    pub(crate) fn symbols(&self) -> impl Iterator<Item = &SymbolResult> {
        self.symbols.iter()
    }

    pub(crate) fn relationships(&self) -> impl Iterator<Item = &RelationshipResult> {
        self.relationships.iter()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SymbolResult {
    pub(crate) id: SymbolId,
    pub(crate) name: String,
    pub(crate) kind: SymbolKind,
    pub(crate) language: super::symbol::Language,
}

#[derive(Debug, Clone)]
pub(crate) struct RelationshipResult {
    pub(crate) from: SymbolId,
    pub(crate) to: RelationshipTarget,
    pub(crate) kind: RelationshipKind,
}

pub(crate) enum Query {
    Find(FindQuery),
}

pub(crate) struct FindQuery {
    pub(crate) name: Option<String>,
    pub(crate) kind: Option<SymbolKindQuery>,
    pub(crate) relationship: Option<RelationshipQuery>,
}

pub(crate) struct RelationshipQuery {
    pub(crate) direction: Direction,
    pub(crate) kind: Option<RelationshipKind>,
    pub(crate) depth: usize,
}

pub(crate) enum Direction {
    Incoming,
    Outgoing,
    Both,
}

pub(crate) enum SymbolKindQuery {
    Call,
    Function,
    Class,
}
impl PartialEq<SymbolKind> for SymbolKindQuery {
    fn eq(&self, other: &SymbolKind) -> bool {
        match self {
            SymbolKindQuery::Call => matches!(other, SymbolKind::FunctionCall),
            SymbolKindQuery::Function => matches!(other, SymbolKind::FunctionDefinition(_)),
            SymbolKindQuery::Class => matches!(other, SymbolKind::Class),
        }
    }
}
pub(crate) struct QueryEngine<'a> {
    graph: &'a Graph,
}

impl<'a> QueryEngine<'a> {
    pub(crate) fn new(graph: &'a Graph) -> Self {
        Self { graph }
    }

    pub(crate) fn execute(&self, query: Query) -> QueryResult {
        match query {
            Query::Find(query) => self.execute_find(query),
        }
    }
    fn collect_relationships(
        &self,
        id: SymbolId,
        query: &RelationshipQuery,
        symbols: &mut Vec<SymbolResult>,
        relationships: &mut Vec<RelationshipResult>,
    ) {
        match query.direction {
            Direction::Outgoing => {
                for relationship in self.graph.relationships_from(id) {
                    if let Some(kind) = &query.kind {
                        if &relationship.kind != kind {
                            continue;
                        }
                    }

                    relationships.push(RelationshipResult {
                        from: relationship.from,
                        to: relationship.to.clone(),
                        kind: relationship.kind,
                    });

                    match relationship.to {
                        RelationshipTarget::Resolved(uuid) => {
                            if let Some(symbol) = self.graph.symbol(uuid) {
                                symbols.push(SymbolResult {
                                    id: uuid,
                                    name: symbol.name.clone(),
                                    kind: symbol.kind.clone(),
                                    language: symbol.language,
                                });
                            }
                        }
                        _ => (),
                    }
                }
            }

            Direction::Incoming => {
                for relationship in self.graph.relationships_to(id) {
                    if let Some(kind) = &query.kind {
                        if &relationship.kind != kind {
                            continue;
                        }
                    }

                    relationships.push(RelationshipResult {
                        from: relationship.from,
                        to: relationship.to.clone(),
                        kind: relationship.kind,
                    });

                    if let Some(sym) = self.graph.symbol(relationship.from) {
                        symbols.push(SymbolResult {
                            id: sym.id,
                            name: sym.name.clone(),
                            kind: sym.kind.clone(),
                            language: sym.language,
                        });
                    }
                }
            }

            Direction::Both => {
                // Do outgoing + incoming
            }
        }
    }
    fn execute_find(&self, query: FindQuery) -> QueryResult {
        let mut symbols = Vec::new();
        let mut relationships = Vec::new();

        for (id, symbol) in self.graph.symbols() {
            if let Some(name) = &query.name {
                if symbol.name != *name {
                    continue;
                }
            }

            if let Some(kind) = &query.kind {
                if kind != &symbol.kind {
                    continue;
                }
            }

            symbols.push(SymbolResult {
                id: *id,
                name: symbol.name.clone(),
                kind: symbol.kind.clone(),
                language: symbol.language,
            });

            if let Some(relationship_query) = &query.relationship {
                self.collect_relationships(
                    *id,
                    relationship_query,
                    &mut symbols,
                    &mut relationships,
                )
            }
        }

        QueryResult::new(symbols, relationships)
    }
}
