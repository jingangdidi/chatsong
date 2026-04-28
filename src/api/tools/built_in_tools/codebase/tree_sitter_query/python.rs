use crate::tools::built_in_tools::codebase::tree_sitter_query::{
    LanguageConfig,
    TestPattern,
    join_children,
};

pub const SYMBOLS_QUERY: &str = r#"
(function_definition
  name: (identifier) @function.name) @function.def

(class_definition
  name: (identifier) @class.name
  body: (block
    (function_definition
      name: (identifier) @method.name) @method.def)?) @class.def
"#;

pub const CALLERS_QUERY: &str = r#"
(call
  function: (identifier) @callee)

(call
  function: (attribute
    attribute: (identifier) @callee))
"#;

pub const VARIABLES_QUERY: &str = r#"
(assignment
  left: (identifier) @var.name)

(assignment
  left: (pattern_list
    (identifier) @var.name))

(assignment
  left: (tuple_pattern
    (identifier) @var.name))

(for_statement
  left: (identifier) @var.name)

(for_statement
  left: (tuple_pattern
    (identifier) @var.name))

(with_item
  (as_pattern
    alias: (as_pattern_target
      (identifier) @var.name)))

(parameters
  (identifier) @var.name)

(parameters
  (default_parameter
    name: (identifier) @var.name))

(parameters
  (typed_parameter
    (identifier) @var.name))

(parameters
  (typed_default_parameter
    name: (identifier) @var.name))
"#;

pub fn config() -> LanguageConfig {
    LanguageConfig {
        language: tree_sitter_python::LANGUAGE.into(),
        symbols_query: SYMBOLS_QUERY,
        callers_query: CALLERS_QUERY,
        variables_query: VARIABLES_QUERY,
        test_patterns: vec![TestPattern::FunctionPrefix("test_")],
        func_method: vec!["function_definition", "method_definition"],
        name: Some("name"),
    }
}

/// get function/method signature
pub fn get_sig(n: tree_sitter::Node, src: &[u8]) -> String {
    join_children(n, src, |c| {
        matches!(
            c.kind(),
            "async" | "def" | "identifier" | "parameters" | "type" | "return_type"
        )
    })
}
