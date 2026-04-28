use crate::tools::built_in_tools::codebase::tree_sitter_query::{
    LanguageConfig,
    TestPattern,
    join_children,
};

pub const SYMBOLS_QUERY: &str = r#"
(function_declaration
  name: (identifier) @function.name) @function.def

(method_declaration
  name: (field_identifier) @method.name) @method.def

(type_declaration
  (type_spec
    name: (type_identifier) @struct.name
    type: (struct_type))) @struct.def

(type_declaration
  (type_spec
    name: (type_identifier) @interface.name
    type: (interface_type))) @interface.def

(type_declaration
  (type_spec
    name: (type_identifier) @type.name)) @type.def

(const_declaration
  (const_spec
    name: (identifier) @const.name)) @const.def

(var_declaration
  (var_spec
    name: (identifier) @const.name)) @const.def
"#;

pub const CALLERS_QUERY: &str = r#"
(call_expression
  function: (identifier) @callee)

(call_expression
  function: (selector_expression
    field: (field_identifier) @callee))
"#;

pub const VARIABLES_QUERY: &str = r#"
(short_var_declaration
  left: (expression_list
    (identifier) @var.name))

(var_declaration
  (var_spec
    name: (identifier) @var.name))

(range_clause
  left: (expression_list
    (identifier) @var.name))

(parameter_declaration
  name: (identifier) @var.name)
"#;

pub fn config() -> LanguageConfig {
    LanguageConfig {
        language: tree_sitter_go::LANGUAGE.into(),
        symbols_query: SYMBOLS_QUERY,
        callers_query: CALLERS_QUERY,
        variables_query: VARIABLES_QUERY,
        test_patterns: vec![TestPattern::FunctionPrefix("Test")],
        func_method: vec!["function_declaration", "method_declaration"],
        name: Some("name"),
    }
}

/// get function/method signature
pub fn get_sig(n: tree_sitter::Node, src: &[u8]) -> String {
    join_children(n, src, |c| {
        matches!(
            c.kind(),
            "func" | "identifier" | "field_identifier" | "parameter_list" | "result"
        )
    })
}
