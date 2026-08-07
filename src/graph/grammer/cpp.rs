use std::collections::HashMap;

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

#[derive(Default, Debug)]
struct DeclaratorInfo<'tree> {
    parameters: Option<Node<'tree>>,
    pointer_depth: usize,
    has_reference: bool,
    is_destructor: bool,
}

fn analyze_declarator<'tree>(node: Node<'tree>, info: &mut DeclaratorInfo<'tree>) {
    if let Some(params) = node.child_by_field_name("parameters") {
        info.parameters = Some(params);
    }

    if info.parameters.is_none() {
        match node.kind() {
            "pointer_declarator" => {
                info.pointer_depth += 1;
            }
            "reference_declarator" => {
                info.has_reference = true;
            }
            "destructor_name" => {
                info.is_destructor = true;
            }
            _ => {}
        }
    }

    for child in node.named_children(&mut node.walk()) {
        analyze_declarator(child, info);
    }
}

fn extract_parameters(node: Option<Node>, source: &str) -> Vec<Parameter> {
    let mut params = Vec::new();

    if let Some(node) = node {
        for child in node.children(&mut node.walk()) {
            let modifiers: Vec<Modifier> = child
                .named_children(&mut child.walk())
                .find(|n| n.kind() == "type_qualifier")
                .map(|m| {
                    m.children(&mut m.walk())
                        .map(|n| Modifier::from(&source[n.start_byte()..n.end_byte()]))
                        .collect()
                })
                .unwrap_or_default();
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

                    let info = &mut DeclaratorInfo::default();
                    analyze_declarator(child, info);

                    params.push(Parameter {
                        name,
                        type_specifier: type_name,
                        modifiers,
                        variadic: child.kind() == "variadic_parameter_declaration",
                        reference: info.has_reference,
                        pointer_depth: info.pointer_depth,
                        default,
                    });
                }
                "..." => {
                    params.push(Parameter {
                        name: child.kind().into(),
                        type_specifier: String::default(),
                        modifiers,
                        variadic: true,
                        default: None,
                        reference: bool::default(),
                        pointer_depth: usize::default(),
                    });
                }
                _ => (),
            }
        }
    }

    params
}

fn extract_function_declarator<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    node.children(&mut node.walk())
        .find(|n| n.kind() == "function_declarator")
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

            let value = node
                .children(&mut node.walk())
                .find(|n| n.kind() == "=")
                .and_then(|n| {
                    n.next_sibling()
                        .map(|n| &content[n.start_byte()..n.end_byte()])
                })
                .or_else(|| {
                    node.children(&mut node.walk())
                        .find(|n| n.kind() == "default_method_clause")
                        .map(|_| "default")
                })
                .unwrap_or_default()
                .to_string();

            if info.is_destructor {
                return SymbolKind::Destructor(value);
            }

            let type_qualifier = extract_function_declarator(node)
                .and_then(|_| {
                    node.children(&mut node.walk())
                        .find(|n| n.kind() == "type_qualifier")
                })
                .map(|n| &content[n.start_byte()..n.end_byte()])
                .unwrap_or_default();

            if let Some(type_node) = node.child_by_field_name("type") {
                let name = [
                    type_qualifier.trim().to_string(),
                    content[type_node.start_byte()..declarator.start_byte()]
                        .trim()
                        .to_string(),
                ]
                .join(" ")
                .trim()
                .to_string();
                if declarator.kind() == "function_declarator" {
                    return SymbolKind::Method(MethodKind {
                        value: value,
                        params: extract_parameters(info.parameters, content),
                        return_type: Type {
                            reference: info.has_reference,
                            pointer_depth: info.pointer_depth,
                            name,
                        },
                    });
                } else {
                    return SymbolKind::Field(FieldKind {
                        value: value,
                        dtype: Type {
                            reference: info.has_reference,
                            pointer_depth: info.pointer_depth,
                            name,
                        },
                    });
                }
            }
        }
        match node.kind() {
            "class_specifier" => SymbolKind::Class,
            "struct_specifier" => SymbolKind::Struct,
            "enum_specifier" => SymbolKind::Enum,
            "translation_unit" => SymbolKind::Root,
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

        let qualifier_modifier = |qualifier: &str| match qualifier {
            "constexpr" | "consteval" | "constinit" | "const" => Modifier::Constant,
            _ => Modifier::from(qualifier),
        };

        let function_declarator = extract_function_declarator(node);

        if let Some(func_node) = function_declarator {
            // type qualifier can appear at end of method
            if let Some(type_name) = func_node
                .children(&mut func_node.walk())
                .find(|n| n.kind() == "type_qualifier")
                .map(|n| &content[n.start_byte()..n.end_byte()])
            {
                modifs.push(qualifier_modifier(type_name))
            }
        }

        let has_function_declarator = function_declarator.is_some();
        for n in node.children(&mut node.walk()) {
            let val = &content[n.start_byte()..n.end_byte()];
            let modif = match n.kind() {
                "type_qualifier" if !has_function_declarator => Some(qualifier_modifier(val)),
                "virtual_specifier"
                | "attribute_specifier"
                | "storage_class_specifier"
                | "ms_declspec_modifier"
                | "virtual"
                | "attribute_declaration" => Some(Modifier::from(val)),
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
    fn extract_metadata(&self, node: &Node, content: &str, metadata: &mut HashMap<String, String>) {
        ()
    }
}

#[derive(Error, Debug)]
pub enum CppError {
    #[error("{0}")]
    StdError(#[from] std::io::Error),
}
