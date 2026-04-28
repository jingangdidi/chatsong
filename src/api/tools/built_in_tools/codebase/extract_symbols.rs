use std::path::Path;

use tree_sitter::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};
use tracing::{event, Level};

use crate::{
    error::MyError,
    tools::built_in_tools::codebase::{
        symbol_table::{
            Language,
            Symbol,
            SymbolKind,
            SymbolTable,
        },
        file_tree::FileTree,
        tree_sitter_query::{
            get_language_config,
            get_func_method_signature,
        },
    },
};

/// Extract symbols from a single file
fn extract_symbols_from_file(root: &Path, rel_path: &str, language: Language) -> Result<Vec<Symbol>, MyError> {
    let config = match get_language_config(language) {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let abs_path = root.join(rel_path);
    let source = std::fs::read_to_string(&abs_path)?;

    let mut parser = Parser::new();
    parser.set_language(&config.language).map_err(|_| MyError::ParseLanguageError{language: language.convert_to_string()})?;

    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => {
            event!(Level::ERROR, "Failed to parse {}", rel_path);
            return Ok(Vec::new());
        }
    };

    let query = Query::new(&config.language, config.symbols_query).map_err(|e| MyError::TreeSitterQueryError{error: e})?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

    let capture_names: Vec<String> = query.capture_names().iter().map(|s| s.to_string()).collect();

    let mut symbols = Vec::new();
    let mut current_impl_type: Option<String> = None;

    while let Some(m) = matches.next() {
        let mut name: Option<String> = None;
        let mut kind: Option<SymbolKind> = None;
        let mut def_node: Option<tree_sitter::Node> = None;
        let mut parent: Option<String> = None;

        for cap in m.captures {
            let cap_name = &capture_names[cap.index as usize];
            let text = cap.node.utf8_text(source.as_bytes()).unwrap_or("");

            match cap_name.as_str() {
                "function.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Function);
                }
                "function.def" => {
                    def_node = Some(cap.node);
                }
                "method.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Method);
                    parent = current_impl_type.clone();
                }
                "method.def" => {
                    def_node = Some(cap.node);
                }
                "impl.type" => {
                    current_impl_type = Some(text.to_string());
                }
                "struct.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Struct);
                }
                "struct.def" => {
                    def_node = Some(cap.node);
                }
                "enum.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Enum);
                }
                "enum.def" => {
                    def_node = Some(cap.node);
                }
                "trait.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Trait);
                }
                "trait.def" => {
                    def_node = Some(cap.node);
                }
                "class.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Class);
                }
                "class.def" => {
                    def_node = Some(cap.node);
                }
                "interface.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Interface);
                }
                "interface.def" => {
                    def_node = Some(cap.node);
                }
                "type.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Type);
                }
                "type.def" => {
                    def_node = Some(cap.node);
                }
                "const.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Constant);
                }
                "const.def" => {
                    def_node = Some(cap.node);
                }
                "static.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Constant);
                }
                "static.def" => {
                    def_node = Some(cap.node);
                }
                "mod.name" => {
                    name = Some(text.to_string());
                    kind = Some(SymbolKind::Module);
                }
                "mod.def" => {
                    def_node = Some(cap.node);
                }
                _ => {}
            }
        }

        if let (Some(name), Some(kind), Some(node)) = (name, kind, def_node) {
            let start = node.start_position();
            let end = node.end_position();
            let byte_range = (node.start_byte(), node.end_byte());
            let line_range = (start.row + 1, end.row + 1); // 1-indexed

            // Extract signature (first line of the definition)
            let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
            let signature = if kind == SymbolKind::Method || kind == SymbolKind::Function {
                // get function/method signature
                match get_func_method_signature(node, source.as_bytes(), language) {
                    Some(s) => s,
                    None => node_text.to_string(),
                }
            } else {
                //node_text.lines().next().unwrap_or("").to_string() // only first line
                node_text.to_string()
            };

            symbols.push(Symbol {
                name,
                kind,
                file: rel_path.to_string(),
                byte_range,
                line_range,
                language,
                signature,
                definition: None,
                parent,
            });
        }
    }

    event!(Level::INFO, "Extracted {} symbols from {}", symbols.len(), rel_path);
    Ok(symbols)
}

/// Extract symbols from all files in the tree. Runs on blocking threads
/// with bounded concurrency
pub fn extract_all_symbols(root: &Path, file_tree: &mut FileTree, symbol_table: &mut SymbolTable) -> Result<usize, MyError> {
    let root = root.to_path_buf();
    let mut count = 0;

    let paths: Vec<(String, Language)> = file_tree
        .files
        .iter()
        .filter(|(_, v)| !v.symbols_extracted && v.language.has_tree_sitter_support())
        .map(|(k, v)| (k.clone(), v.language))
        .collect();

    for (rel_path, language) in paths {
        match extract_symbols_from_file(&root, &rel_path, language) {
            Ok(symbols) => {
                count += symbols.len();
                for sym in symbols {
                    symbol_table.insert(sym);
                }
                // Mark file as having symbols extracted
                if let Some(v) = file_tree.files.get_mut(&rel_path) {
                    v.symbols_extracted = true;
                }
            }
            Err(e) => event!(Level::INFO, "Failed to extract symbols from {}: {}", rel_path, e),
        }
    }

    Ok(count)
}
