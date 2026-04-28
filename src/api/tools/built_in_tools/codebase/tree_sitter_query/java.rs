use crate::tools::built_in_tools::codebase::tree_sitter_query::{
    LanguageConfig,
    TestPattern,
    join_children,
};

pub const SYMBOLS_QUERY: &str = r#"
(class_declaration
  name: (identifier) @class.name) @class.def

(interface_declaration
  name: (identifier) @interface.name) @interface.def

(enum_declaration
  name: (identifier) @enum.name) @enum.def

(record_declaration
  name: (identifier) @record.name) @record.def

(method_declaration
  name: (identifier) @method.name) @method.def

(constructor_declaration
  name: (identifier) @constructor.name) @constructor.def
"#;

pub const CALLERS_QUERY: &str = r#"
(method_invocation
  name: (identifier) @callee)

(object_creation_expression
  type: (type_identifier) @callee)
"#;

pub const VARIABLES_QUERY: &str = r#"
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @var.name))

(enhanced_for_statement
  name: (identifier) @var.name)

(formal_parameter
  name: (identifier) @var.name)
"#;

pub fn config() -> LanguageConfig {
    LanguageConfig {
        language: tree_sitter_java::LANGUAGE.into(),
        symbols_query: SYMBOLS_QUERY,
        callers_query: CALLERS_QUERY,
        variables_query: VARIABLES_QUERY,
        test_patterns: vec![
            TestPattern::Attribute("Test"),
            TestPattern::Attribute("org.junit"),
        ],
        func_method: vec!["method_declaration", "constructor_declaration", "static_initializer"],
        name: Some("name"),
    }
}

/// get function/method signature
pub fn get_sig(n: tree_sitter::Node, src: &[u8]) -> String {
    join_children(n, src, |c| {
        matches!(
            c.kind(),
            "modifiers" | "type_identifier" | "identifier" | "type_parameters" | "formal_parameters" | "throws"
        )
    })
}
