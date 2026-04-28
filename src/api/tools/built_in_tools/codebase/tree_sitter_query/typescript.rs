use crate::tools::built_in_tools::codebase::tree_sitter_query::{
    LanguageConfig,
    TestPattern,
    join_children,
};

pub const SYMBOLS_QUERY: &str = r#"
(function_declaration
  name: (identifier) @function.name) @function.def

(class_declaration
  name: (type_identifier) @class.name) @class.def

(method_definition
  name: (property_identifier) @method.name) @method.def

(lexical_declaration
  (variable_declarator
    name: (identifier) @const.name
    value: (arrow_function))) @const.def

(interface_declaration
  name: (type_identifier) @interface.name) @interface.def

(type_alias_declaration
  name: (type_identifier) @type.name) @type.def

(enum_declaration
  name: (identifier) @enum.name) @enum.def
"#;

pub const CALLERS_QUERY: &str = r#"
(call_expression
  function: (identifier) @callee)

(call_expression
  function: (member_expression
    property: (property_identifier) @callee))
"#;

pub const VARIABLES_QUERY: &str = r#"
(variable_declarator
  name: (identifier) @var.name)

(variable_declarator
  name: (object_pattern
    (shorthand_property_identifier_pattern) @var.name))

(variable_declarator
  name: (array_pattern
    (identifier) @var.name))

(for_in_statement
  left: (identifier) @var.name)

(for_in_statement
  left: (lexical_declaration
    (variable_declarator
      name: (identifier) @var.name)))

(required_parameter
  pattern: (identifier) @var.name)

(optional_parameter
  pattern: (identifier) @var.name)
"#;

pub fn config() -> LanguageConfig {
    LanguageConfig {
        language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        symbols_query: SYMBOLS_QUERY,
        callers_query: CALLERS_QUERY,
        variables_query: VARIABLES_QUERY,
        test_patterns: vec![
            TestPattern::CallExpression("it"),
            TestPattern::CallExpression("test"),
            TestPattern::CallExpression("describe"),
        ],
        func_method: vec!["function_declaration", "method_definition", "arrow_function", "function_expression"],
        name: None,
    }
}

pub const JS_SYMBOLS_QUERY: &str = r#"
(function_declaration
  name: (identifier) @function.name) @function.def

(class_declaration
  name: (identifier) @class.name) @class.def

(method_definition
  name: (property_identifier) @method.name) @method.def

(lexical_declaration
  (variable_declarator
    name: (identifier) @const.name
    value: (arrow_function))) @const.def
"#;

pub const JS_CALLERS_QUERY: &str = r#"
(call_expression
  function: (identifier) @callee)

(call_expression
  function: (member_expression
    property: (property_identifier) @callee))
"#;

pub const JS_VARIABLES_QUERY: &str = r#"
(variable_declarator
  name: (identifier) @var.name)

(variable_declarator
  name: (object_pattern
    (shorthand_property_identifier_pattern) @var.name))

(variable_declarator
  name: (array_pattern
    (identifier) @var.name))

(for_in_statement
  left: (identifier) @var.name)

(formal_parameters
  (identifier) @var.name)
"#;

pub fn js_config() -> LanguageConfig {
    LanguageConfig {
        language: tree_sitter_javascript::LANGUAGE.into(),
        symbols_query: JS_SYMBOLS_QUERY,
        callers_query: JS_CALLERS_QUERY,
        variables_query: JS_VARIABLES_QUERY,
        test_patterns: vec![
            TestPattern::CallExpression("it"),
            TestPattern::CallExpression("test"),
            TestPattern::CallExpression("describe"),
        ],
        func_method: vec!["function_declaration", "method_definition", "arrow_function", "function_expression"],
        name: None,
    }
}

/// get function/method signature
pub fn get_sig(n: tree_sitter::Node, src: &[u8]) -> String {
    join_children(n, src, |c| {
        matches!(
            c.kind(),
            "async" | "function" | "identifier" | "property_identifier" | "type_parameters" | "formal_parameters" | "type_annotation"
        )
    })
}
