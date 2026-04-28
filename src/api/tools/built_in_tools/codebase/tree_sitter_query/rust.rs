use crate::tools::built_in_tools::codebase::tree_sitter_query::{
    LanguageConfig,
    TestPattern,
    join_children,
};

pub const SYMBOLS_QUERY: &str = r#"
(function_item
  name: (identifier) @function.name) @function.def

(impl_item
  type: (_) @impl.type
  body: (declaration_list
    (function_item
      name: (identifier) @method.name) @method.def))

(struct_item
  name: (type_identifier) @struct.name) @struct.def

(enum_item
  name: (type_identifier) @enum.name) @enum.def

(trait_item
  name: (type_identifier) @trait.name) @trait.def

(type_item
  name: (type_identifier) @type.name) @type.def

(const_item
  name: (identifier) @const.name) @const.def

(static_item
  name: (identifier) @static.name) @static.def

(mod_item
  name: (identifier) @mod.name) @mod.def
"#;

pub const CALLERS_QUERY: &str = r#"
(call_expression
  function: (identifier) @callee)

(call_expression
  function: (field_expression
    field: (field_identifier) @callee))

(call_expression
  function: (scoped_identifier
    name: (identifier) @callee))

(macro_invocation
  macro: (identifier) @callee)
"#;

pub const VARIABLES_QUERY: &str = r#"
(let_declaration
  pattern: (identifier) @var.name)

(let_declaration
  pattern: (tuple_pattern
    (identifier) @var.name))

(let_declaration
  pattern: (tuple_struct_pattern
    (identifier) @var.name))

(for_expression
  pattern: (identifier) @var.name)

(if_let_expression
  pattern: (_
    (identifier) @var.name))

(parameter
  pattern: (identifier) @var.name)
"#;

pub fn config() -> LanguageConfig {
    LanguageConfig {
        language: tree_sitter_rust::LANGUAGE.into(),
        symbols_query: SYMBOLS_QUERY,
        callers_query: CALLERS_QUERY,
        variables_query: VARIABLES_QUERY,
        test_patterns: vec![TestPattern::Attribute("test")],
        func_method: vec!["function_item", "function_signature"],
        name: Some("name"),
    }
}

/// get function/method signature
pub fn get_sig(n: tree_sitter::Node, src: &[u8]) -> String {
    join_children(n, src, |c| {
        matches!(
            c.kind(),
            "visibility_modifier" | "async" | "const" | "unsafe" | "fn" | "identifier" | "generic_parameter_list" | "parameters"
            | "->" | "type_identifier" | "generic_type" | "scoped_type_identifier" | "qualified_type" | "reference_type" | "pointer_type" | "tuple_type" | "array_type" | "slice_type" | "impl_trait_type" | "abstract_type"
            | "where_clause"
        )
    })
}
