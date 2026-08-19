use std::collections::HashMap;

use thiserror::Error;
use tree_sitter::Node;

use crate::graph::{
    Modifier,
    SymbolKind::{self},
    Visibility,
    build::{Relationships, ScopeIndexTable, handle_query},
    grammer::Grammer,
    parser::LanguageParser,
    relationship::{Relationship, RelationshipKind, RelationshipTarget},
    symbol::{
        Constructor, FunctionDefinition, Generic, Language, MemberVariable, Parameter, Scope,
        SymbolId, Type,
    },
};

fn extract_parameters(node: Option<Node>, source: &str) -> Vec<Parameter> {
    let Some(node) = node else {
        return Vec::new();
    };

    node.named_children(&mut node.walk())
        .map(|child| {
            let modifiers = child
                .named_children(&mut child.walk())
                .find(|n| n.kind() == "modifiers")
                .map(|m| {
                    m.children(&mut m.walk())
                        .map(|n| Modifier::from(&source[n.start_byte()..n.end_byte()]))
                        .collect()
                })
                .unwrap_or_default();

            let name = child
                .child_by_field_name("name")
                .or_else(|| {
                    child
                        .named_children(&mut child.walk())
                        .find(|n| n.kind() == "variable_declarator")
                        .and_then(|v| v.child_by_field_name("name"))
                })
                .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();

            let type_specifier = child
                .child_by_field_name("type")
                .or_else(|| {
                    child
                        .named_children(&mut child.walk())
                        .find(|n| n.grammar_name() == "identifier")
                })
                .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                .unwrap_or_default();

            Parameter {
                name,
                type_specifier,
                modifiers,
                variadic: child.kind() == "spread_parameter",
                default: None,
                reference: bool::default(),
                pointer_depth: usize::default(),
            }
        })
        .collect()
}

fn extract_package_declaration(node: &Node, content: &str) -> Scope {
    if let Some(n) = node
        .named_children(&mut node.walk())
        .find(|n| n.kind() == "package_declaration")
    {
        return Scope {
            scopes: content[n.start_byte()..n.end_byte()]
                .trim_start_matches("package")
                .trim_end_matches(";")
                .trim()
                .to_string()
                .split(".")
                .map(|x| x.to_string())
                .collect(),
            wildcard: bool::default(),
        };
    }
    Scope::default()
}
fn extract_import_declaration(node: &Node, content: &str) -> Scope {
    let mut scopes: Vec<String> = content[node.start_byte()..node.end_byte()]
        .trim_start_matches("import")
        .trim_end_matches(";")
        .trim()
        .to_string()
        .split(".")
        .map(|x| x.to_string())
        .collect();
    let wildcard = scopes.last().map_or("", |v| v) == "*";
    let is_static = scopes.first().map_or("", |v| v).starts_with("static");
    if let Some(c) = scopes.first_mut() {
        if let Some(s) = c.strip_prefix("static") {
            *c = s.trim().to_string();
        }
    }
    // Lookup to function properly requires no static method ending
    if is_static {
        scopes.pop();
    }
    Scope { scopes, wildcard }
}
#[derive(Clone)]
pub(crate) struct Java;
impl LanguageParser for Java {}
impl Grammer for Java {
    fn declaration_nodes(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "record_declaration",
            "interface_declaration",
            "method_declaration",
            "field_declaration",
            "constructor_declaration",
            "enum_declaration",
            "method_invocation",
            "import_declaration",
        ]
    }
    fn flatten_nodes(&self) -> &'static [&'static str] {
        &[
            "class_body",
            "interface_body",
            "block",
            "return_statement",
            "expression_statement",
            "labeled_statement",
            "assert_statement",
            "do_statement",
            "yield_statement",
            "try_statement",
            "catch_clause",
            "finally_clause",
            "parenthesized_expression",
            "binary_expression",
            "assignment_expression",
            "if_statement",
            "for_statement",
            "argument_list",
        ]
    }
    fn to_symbolkind(&self, node: &Node, content: &str) -> SymbolKind {
        match node.kind() {
            "method_declaration" => node
                .child_by_field_name("type")
                .map(|ty| {
                    SymbolKind::FunctionDefinition(FunctionDefinition {
                        value: "".into(),
                        params: extract_parameters(node.child_by_field_name("parameters"), content),
                        return_type: Type::from((ty, content)),
                    })
                })
                .unwrap_or(SymbolKind::Unknown),
            "field_declaration" => node
                .child_by_field_name("type")
                .map(|ty| {
                    let value = node
                        .named_children(&mut node.walk())
                        .find(|n| n.kind() == "variable_declarator")
                        .and_then(|v| v.child_by_field_name("value"))
                        .map(|n| &content[n.start_byte()..n.end_byte()])
                        .unwrap_or_default()
                        .to_string();
                    SymbolKind::MemberVariable(MemberVariable {
                        value,
                        dtype: Type::from((ty, content)),
                    })
                })
                .unwrap_or(SymbolKind::Unknown),
            "constructor_declaration" => SymbolKind::Constructor(Constructor {
                value: "".into(),
                params: extract_parameters(node.child_by_field_name("parameters"), content),
            }),
            "method_invocation" => SymbolKind::FunctionCall,
            "class_declaration" => SymbolKind::Class,
            "interface_declaration" => SymbolKind::Interface,
            "import_declaration" => SymbolKind::Import(extract_import_declaration(node, content)),
            "program" => SymbolKind::Module(extract_package_declaration(node, content)),
            _ => SymbolKind::Unknown,
        }
    }

    fn extract_declaration_attributes(
        &self,
        node: &Node,
        content: &str,
    ) -> (Visibility, Vec<Modifier>) {
        let mut visibility = Visibility::Package;
        let mut modifs = Vec::new();
        if let Some(modif_node) = node
            .children(&mut node.walk())
            .find(|child| child.kind() == "modifiers")
        {
            let value = &content[modif_node.start_byte()..modif_node.end_byte()];
            for m in value.split(" ") {
                // The first valid modifier will act as visibility
                // so everything else will default to the modifs
                let found_vis = Visibility::from(m);
                if found_vis != Visibility::Unknown {
                    visibility = found_vis;
                } else {
                    modifs.push(Modifier::from(m))
                }
            }
        }
        (visibility, modifs)
    }

    fn to_generics(&self, node: &Node, content: &str) -> Vec<Generic> {
        let mut generics = Vec::new();
        if let Some(ty_params_node) = node.child_by_field_name("type_parameters") {
            let mut generics_map: HashMap<String, Generic> = HashMap::new();
            let mut current_generic: String = String::new();
            handle_query(
                &tree_sitter_java::LANGUAGE.into(),
                "(type_parameter
            (type_identifier) @name
                (type_bound [
                    (type_identifier) @bound
                    (generic_type (type_identifier) @bound)
                ])?
        )",
                &ty_params_node,
                content,
                |_, cap_name, value| match cap_name {
                    "bound" => generics_map
                        .get_mut(&current_generic)
                        .expect("name to always be parsed first")
                        .bounds
                        .push(value),
                    "name" => {
                        let g = generics_map.entry(value.clone()).or_default();
                        g.name = value.clone();
                        current_generic = value;
                    }
                    _ => (),
                },
            );
            generics = generics_map.values().cloned().collect()
        }
        generics
    }
    fn gather_all_permits<'a>(&self, node: &Node, content: &'a str) -> Option<Vec<&'a str>> {
        if let Some(permit_node) = node.child_by_field_name("permits") {
            let value = &content[permit_node.start_byte()..permit_node.end_byte()];
            return Some(value.strip_prefix("permits ").unwrap().split(",").collect());
        }
        None
    }

    fn gather_all_inheritance<'a>(&self, node: &Node, content: &'a str) -> Option<Vec<&'a str>> {
        let mut inherits = Vec::new();
        if let Some(interface_node) = node.child_by_field_name("interfaces") {
            let classes = interface_node
                .named_children(&mut interface_node.walk())
                .find(|n| n.kind() == "type_list")?;
            inherits = classes
                .named_children(&mut classes.walk())
                .map(|n| &content[n.start_byte()..n.end_byte()])
                .collect();
        }
        if let Some(superclass_node) = node.child_by_field_name("superclass") {
            for n in superclass_node.named_children(&mut superclass_node.walk()) {
                inherits.push(&content[n.start_byte()..n.end_byte()]);
            }
        }
        Some(inherits)
    }

    fn language(&self) -> Language {
        Language::Java
    }
    fn tree_language(&self) -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
    }
    fn to_scope(&self, node: &Node, content: &str) -> Scope {
        match node.kind() {
            "class_declaration" | "interface_declaration" => Scope {
                scopes: vec![self.to_name(node, content)],
                wildcard: bool::default(),
            },
            "program" => extract_package_declaration(node, content),
            _ => Scope::default(),
        }
    }

    fn to_name(&self, node: &Node, content: &str) -> String {
        match node.kind() {
            "program" => "program".into(),
            "import_declaration" => content[node.start_byte()..node.end_byte()]
                .trim_start_matches("import")
                .trim_end_matches(";")
                .trim()
                .to_string(),
            _ => node
                .child_by_field_name("name")
                .or_else(|| {
                    node.named_children(&mut node.walk())
                        .find(|n| n.kind() == "variable_declarator")
                        .and_then(|v| v.child_by_field_name("name"))
                })
                .map(|n| &content[n.start_byte()..n.end_byte()])
                .unwrap_or_default()
                .to_string(),
        }
    }
    fn extract_metadata(&self, node: &Node, content: &str, metadata: &mut HashMap<String, String>) {
        let exceptions: Vec<&str> = node
            .named_children(&mut node.walk())
            .find(|n| n.kind() == "throws")
            .map(|n| {
                n.named_children(&mut n.walk())
                    .map(|n| &content[n.start_byte()..n.end_byte()])
                    .collect()
            })
            .unwrap_or_default();

        if !exceptions.is_empty() {
            metadata.insert("throws".into(), exceptions.join(","));
        }
    }

    fn pair_relationships(
        &self,
        node: &Node,
        parent_id: SymbolId,
        child_id: SymbolId,
        relationships: &mut Relationships,
    ) {
        let kind = match node.kind() {
            "method_invocation" => RelationshipKind::Calls,
            "import_declaration" => RelationshipKind::Imports,
            _ => RelationshipKind::Contains,
        };

        relationships.push(Relationship {
            from: parent_id,
            to: RelationshipTarget::Resolved(child_id),
            kind,
            metadata: HashMap::new(),
        })
    }

    fn try_resolve_scope(
        &self,
        node: &Node,
        content: &str,
        table: &mut ScopeIndexTable,
        local_scope: &Scope,
        base_types: &mut Vec<String>,
        local_imports: &mut Vec<Scope>,
    ) -> Scope {
        match node.kind() {
            "record_declaration"
            | "field_declaration"
            | "constructor_declaration"
            | "method_declaration"
            | "enum_declaration" => local_scope.clone(),
            "class_declaration" | "interface_declaration" => {
                // Base types are only resolved to class scope
                base_types.clear();

                local_scope.clone()
            }
            "program" => {
                let scope = extract_package_declaration(node, content);
                local_imports.push(scope.clone());
                scope
            }
            "import_declaration" => {
                // We need to track imports for ongoing resolution
                let scope = extract_import_declaration(node, content);
                local_imports.push(scope);
                Scope::default()
            }
            "method_invocation" => {
                if let Some(decl) = table.index.get(local_scope) {
                    // Check if local scope has function defintion
                    if let Some(fields) = node
                        .child_by_field_name("object")
                        .map(|n| &content[n.start_byte()..n.end_byte()])
                        .filter(|v| !v.starts_with("this"))
                        .map(|v| v.split(".").collect::<Vec<&str>>())
                    {
                        // Find field declaration and resolve data type
                        let name = fields.first().unwrap();
                        if let Some((_, symbol)) = decl.get(&(
                            name.to_string(),
                            SymbolKind::MemberVariable(MemberVariable::default()),
                        )) {
                            // Use symbol info find path to resolution
                            if let SymbolKind::MemberVariable(mem) = symbol {
                                let name = &mem.dtype.name;
                                // Check local scope
                                if decl.contains(&(name.to_string(), SymbolKind::Class))
                                    || decl.contains(&(name.to_string(), SymbolKind::Interface))
                                {
                                    println!("FOUND MEMBER - local scope");
                                    let mut scopes = local_scope.scopes.clone();
                                    scopes.push(name.to_string());
                                    return Scope {
                                        scopes,
                                        wildcard: bool::default(),
                                    };
                                } else {
                                    // Check local imports and package
                                    for import in local_imports.as_slice() {
                                        if let Some(import_decl) = table.index.get(import) {
                                            if import_decl
                                                .contains(&(name.to_string(), SymbolKind::Class))
                                                || import_decl.contains(&(
                                                    name.to_string(),
                                                    SymbolKind::Interface,
                                                ))
                                            {
                                                println!("FOUND MEMBER - local package/imports");
                                                return import.clone();
                                            }
                                        }
                                    }

                                    // Resolve latter
                                    local_imports.push(local_scope.clone());
                                }
                            }
                        }
                    } else if let Some(name_node) = node.child_by_field_name("name") {
                        // Check local scope
                        let name = &content[name_node.start_byte()..name_node.end_byte()];
                        if decl
                            .get(&(
                                name.to_string(),
                                SymbolKind::FunctionDefinition(FunctionDefinition::default()),
                            ))
                            .is_some()
                        {
                            println!("FOUND MEMBER - local scope");
                            return local_scope.clone();
                        } else {
                            // Check subclasses and interfaces
                            let imports = local_imports.clone();
                            for (idx, import) in imports.iter().enumerate() {
                                let is_package = idx == 0;
                                for base_type in base_types.as_slice() {
                                    // Only check imports that are valid
                                    if is_package
                                        || import.wildcard
                                        || import.scopes.last() == Some(base_type)
                                    {
                                        let scope = if is_package {
                                            let mut scopes = import.scopes.clone();
                                            scopes.push(base_type.clone());
                                            Scope {
                                                scopes,
                                                wildcard: bool::default(),
                                            }
                                        } else if import.wildcard {
                                            let mut scopes = import.scopes.clone();
                                            scopes.pop(); // remove wildcard bit
                                            scopes.push(base_type.clone());
                                            Scope {
                                                scopes,
                                                wildcard: bool::default(),
                                            }
                                        } else {
                                            import.clone()
                                        };

                                        if let Some(import_decl) = table.index.get(import) {
                                            if import_decl.contains(&(
                                                base_type.to_string(),
                                                SymbolKind::Class,
                                            )) || import_decl.contains(&(
                                                base_type.to_string(),
                                                SymbolKind::Interface,
                                            )) {
                                                println!("FOUND MEMBER - base type");
                                                return scope;
                                            }
                                        }

                                        local_imports.push(scope.clone());
                                    } else {
                                        continue;
                                    }
                                }
                            }

                            // Check local imports and package
                            for import in imports.as_slice() {
                                if let Some(import_decl) = table.index.get(import) {
                                    if import_decl
                                        .get(&(
                                            name.to_string(),
                                            SymbolKind::FunctionDefinition(
                                                FunctionDefinition::default(),
                                            ),
                                        ))
                                        .is_some()
                                    {
                                        println!("FOUND MEMBER - local package/imports");
                                        return import.clone();
                                    }
                                }
                            }

                            // Resolve latter
                            local_imports.push(local_scope.clone());
                        }
                    }
                }
                Scope::default()
                // resolveMethodCall("beam")
                // │
                // ├── methods available on current `this`
                // │   ├── current class X
                // │   ├── superclass X
                // │   └── interfaces X
                // │
                // └── static imports
                //     ├── explicit static import X
                //     └── static wildcard import
                //
                // resolveType("A")
                // │
                // ├── local/nested types
                // ├── current package
                // ├── explicit imports
                // ├── wildcard imports
                // └── java.lang
            }
            _ => Scope::default(),
        }
    }
}
#[derive(Error, Debug)]
pub enum JavaError {
    #[error("{0}")]
    StdError(#[from] std::io::Error),
}
