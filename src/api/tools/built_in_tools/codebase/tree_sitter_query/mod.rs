pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;
pub mod java;

use crate::tools::built_in_tools::codebase::symbol_table::Language;

#[allow(dead_code)]
pub enum TestPattern {
    /// Match functions whose name starts with a prefix (e.g., "test_" in Python)
    FunctionPrefix(&'static str),
    /// Match functions with a specific attribute/decorator (e.g., #[test] in Rust)
    Attribute(&'static str),
    /// Match call expressions (e.g., it(), test(), describe() in JS/TS)
    CallExpression(&'static str),
}

#[allow(dead_code)]
pub struct LanguageConfig {
    pub language: tree_sitter::Language,
    pub symbols_query: &'static str,
    /// Tree-sitter query for call expressions. Captures `@callee` for the called name
    pub callers_query: &'static str,
    /// Tree-sitter query for local variable bindings. Captures `@var.name`
    pub variables_query: &'static str,
    pub test_patterns: Vec<TestPattern>,
    /// get function and method
    pub func_method: Vec<&'static str>,
    pub name: Option<&'static str>,
}

/// Get the tree-sitter language and symbol query for a given language
pub fn get_language_config(lang: Language) -> Option<LanguageConfig> {
    match lang {
        Language::Rust => Some(rust::config()),
        Language::Python => Some(python::config()),
        Language::TypeScript => Some(typescript::config()),
        Language::JavaScript => Some(typescript::js_config()),
        Language::Go => Some(go::config()),
        Language::Java => Some(java::config()),
        _ => None,
    }
}

/// get function/method signature
pub fn join_children<'a, F>(n: tree_sitter::Node, src: &'a [u8], mut pred: F) -> String
where
    F: FnMut(tree_sitter::Node) -> bool,
{
    let mut s = String::new();
    let mut cursor = n.walk();
    for child in n.children(&mut cursor) {
        if child.kind() == "block" {
            break
        }
        if pred(child) {
            if !s.is_empty() && !s.ends_with(' ') {
                s.push(' ');
            }
            s.push_str(std::str::from_utf8(&src[child.byte_range()]).unwrap());
        }
    }
    s
}

/// get function/method signature
pub fn get_func_method_signature(n: tree_sitter::Node, src: &[u8], lang: Language) -> Option<String> {
    match lang {
        Language::Rust => Some(rust::get_sig(n, src)),
        Language::Python => Some(python::get_sig(n, src)),
        Language::TypeScript => Some(typescript::get_sig(n, src)),
        Language::JavaScript => Some(typescript::get_sig(n, src)),
        Language::Go => Some(go::get_sig(n, src)),
        Language::Java => Some(java::get_sig(n, src)),
        _ => None,
    }
}
