use thiserror::Error;
use tree_sitter::Node;

use crate::graph::{
    Modifier, SymbolKind, Visibility,
    grammer::Grammer,
    parser::LanguageParser,
    symbol::{FieldKind, Generic, MethodKind, Parameter, Type},
};

fn sanitize_template_names(value: &str) -> &str {
    value
        .strip_prefix("class ")
        .or_else(|| value.strip_prefix("typename "))
        .unwrap_or(&value)
}

#[derive(Default)]
struct DeclaratorInfo<'tree> {
    parameters: Option<Node<'tree>>,
    pointer_depth: usize,
    has_reference: bool,
}

fn analyze_declarator<'tree>(node: Node<'tree>, info: &mut DeclaratorInfo<'tree>) {
    if let Some(params) = node.child_by_field_name("parameters") {
        info.parameters = Some(params);
    }

    match node.kind() {
        "pointer_declarator" => {
            info.pointer_depth += 1;
        }
        "reference_declarator" => {
            info.has_reference = true;
        }
        _ => {}
    }

    for child in node.named_children(&mut node.walk()) {
        analyze_declarator(child, info);
    }
}

fn extract_parameters(node: Option<Node>, source: &str) -> Vec<Parameter> {
    let mut params = Vec::new();

    if let Some(node) = node {
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "parameter_declaration"
                | "variadic_parameter_declaration"
                | "optional_parameter_declaration" => {
                    let type_name = child
                        .child_by_field_name("type")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                        .unwrap_or_default();

                    let name = child
                        .child_by_field_name("declarator")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                        .unwrap_or_default();

                    let default = child
                        .child_by_field_name("default_value")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string());

                    params.push(Parameter {
                        name,
                        type_specifier: type_name,
                        modifiers: Vec::new(),
                        variadic: child.kind() == "variadic_parameter_declaration",
                        default,
                    });
                }
                "..." => {
                    params.push(Parameter {
                        name: child.kind().into(),
                        type_specifier: String::default(),
                        modifiers: Vec::new(),
                        variadic: true,
                        default: None,
                    });
                }
                _ => (),
            }
        }
    }

    params
}

#[derive(Clone)]
pub(crate) struct Cpp {}
impl LanguageParser for Cpp {}
impl Grammer for Cpp {
    fn declaration_nodes(&self) -> &'static [&'static str] {
        &[
            "class_specifier",
            "struct_specifier",
            "enum_specifier",
            "function_definition",
            "field_declaration",
        ]
    }

    fn flatten_nodes(&self) -> &'static [&'static str] {
        &["template_declaration", "field_declaration_list"]
    }

    fn to_symbolkind(&self, node: &Node, content: &str) -> SymbolKind {
        if let Some(declarator) = node.child_by_field_name("declarator") {
            let info = &mut DeclaratorInfo::default();
            analyze_declarator(declarator, info);

            let mut value = "";
            if let Some(assign_node) = node.children(&mut node.walk()).find(|n| n.kind() == "=") {
                if let Some(val) = assign_node.next_sibling() {
                    value = &content[val.start_byte()..val.end_byte()];
                }
            }

            if let Some(type_node) = node.child_by_field_name("type") {
                let name = &content[type_node.start_byte()..declarator.start_byte()];
                if declarator.kind() == "function_declarator" {
                    return SymbolKind::Method(MethodKind {
                        value: value.to_string(),
                        params: extract_parameters(info.parameters, content),
                        return_type: Type {
                            reference: info.has_reference,
                            pointer_depth: info.pointer_depth,
                            name: name.trim().to_string(),
                        },
                    });
                } else {
                    return SymbolKind::Field(FieldKind {
                        value: value.to_string(),
                        dtype: Type {
                            reference: info.has_reference,
                            pointer_depth: info.pointer_depth,
                            name: name.trim().to_string(),
                        },
                    });
                }
            }
        }
        match node.kind() {
            "class_specifier" => SymbolKind::Class,
            "struct_specifier" => SymbolKind::Struct,
            "enum_specifier" => SymbolKind::Enum,
            _ => SymbolKind::Unknown,
        }
    }

    fn gather_all_inheritance<'a>(&self, node: &Node, content: &'a str) -> Option<Vec<&'a str>> {
        let base = node
            .named_children(&mut node.walk())
            .find(|n| n.kind() == "base_class_clause")?;
        Some(
            base.named_children(&mut base.walk())
                .filter(|n| n.kind() == "type_identifier")
                .map(|n| &content[n.start_byte()..n.end_byte()])
                .collect(),
        )
    }
    // C++ has no permits.
    fn gather_all_permits<'a>(&self, _: &Node, _: &'a str) -> Option<Vec<&'a str>> {
        None
    }

    fn extract_declaration_attributes(
        &self,
        node: &Node,
        content: &str,
    ) -> (Visibility, Vec<Modifier>) {
        let mut visibility = Visibility::Private;
        let mut modifs = Vec::new();

        if node.kind() == "struct_specifier" {
            visibility = Visibility::Public
        }

        for n in node.children(&mut node.walk()) {
            let val = &content[n.start_byte()..n.end_byte()];
            let modif = match n.kind() {
                "type_qualifier" => match val {
                    "constexpr" | "consteval" | "constinit" => Some(Modifier::Constant),
                    _ => Some(Modifier::from(val)),
                },
                "virtual_specifier"
                | "attribute_specifier"
                | "storage_class_specifier"
                | "ms_declspec_modifier"
                | "virtual"
                | "attribute_declaration" => match val {
                    _ => Some(Modifier::from(val)),
                },
                _ => None,
            };
            if let Some(v) = modif {
                modifs.push(v);
            }
        }

        (visibility, modifs)
    }

    fn language(&self) -> tree_sitter::Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn to_generics(&self, node: &Node, content: &str) -> Vec<Generic> {
        let extract_generics = |list_node: Node| {
            let mut local_generics = Vec::new();

            for child in list_node.named_children(&mut list_node.walk()) {
                let name = &content[child.start_byte()..child.end_byte()];

                let clean_name = sanitize_template_names(name);

                local_generics.push(Generic {
                    name: clean_name.to_string(),
                    bounds: Vec::new(),
                });
            }
            local_generics
        };

        let mut generics = Vec::new();

        // Case 1: Generics are a child of the current node (C++ template instantiations)
        if let Some(temp_type) = node.child_by_field_name("type") {
            if let Some(type_list) = temp_type.child_by_field_name("arguments") {
                generics = extract_generics(type_list);
            }
        }

        // Case 2: Generics belong to the parent template declaration (C++ style class/struct declarations)
        if let Some(parent) = node.parent() {
            if parent.kind() == "template_declaration" {
                if let Some(param_list) = parent.child_by_field_name("parameters") {
                    generics.extend(extract_generics(param_list));
                }
            }
        }
        generics
    }

    fn apply_visibility_change(
        &self,
        node: &Node,
        content: &str,
        scoped_vis: &mut Option<Visibility>,
    ) {
        if node.kind() == "access_specifier" {
            *scoped_vis = Some(Visibility::from(
                &content[node.start_byte()..node.end_byte()],
            ))
        }
    }

    fn to_name<'a>(&self, node: &Node, content: &'a str) -> &'a str {
        // Direct identifier
        if node.grammar_name() == "identifier" {
            return &content[node.start_byte()..node.end_byte()];
        }

        // Explicit grammar field
        if let Some(name) = node.child_by_field_name("name") {
            return self.to_name(&name, content);
        }

        if let Some(declarator) = node.child_by_field_name("declarator") {
            let result = self.to_name(&declarator, content);
            if !result.is_empty() {
                return result;
            }
        }

        // Fallback: walk named children
        for child in node.named_children(&mut node.walk()) {
            let result = self.to_name(&child, content);
            if !result.is_empty() {
                return result;
            }
        }

        ""
    }
}

#[derive(Error, Debug)]
pub enum CppError {
    #[error("{0}")]
    StdError(#[from] std::io::Error),
}
