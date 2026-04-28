use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};
use tracing::{event, Level};
use tree_sitter::{
    Node,
    StreamingIterator,
};

use crate::{
    tools::built_in_tools::codebase::{
        file_tree::FileTree,
        symbol_table::{
            Language,
            Symbol,
            SymbolTable,
            SymbolKind,
        },
        walker::scan_directory,
        extract_symbols::extract_all_symbols,
        grep_pattern::grep_with_scope,
        tree_sitter_query::get_language_config,
    },
    parse_paras::PARAS,
    error::MyError,
};

/// caller relationship
#[derive(Debug)]
pub struct AllCallers {
    pub info_src_map: HashMap<CallerInfo, (String, String)>, // value: (source code, signature)
    symbol_file:      HashSet<String>, // symbol::file
}

impl AllCallers {
    /// new
    fn new() -> Self {
        Self {
            info_src_map: HashMap::new(),
            symbol_file: HashSet::new(),
        }
    }

    /// get all callers source code or signature
    pub fn get_all_callers_src_code_or_signature(&self, only_signature: bool) -> String {
        let mut result_vec: Vec<String> = Vec::new();

        for (k, v) in &self.info_src_map {
            result_vec.push(format!("/// source file: {}\n{}", k.file, if only_signature { &v.1 } else { &v.0 }));
        }

        result_vec.join("\n\n")
    }
}

/// A single indexed project with its own file tree and symbol table
#[derive(Serialize, Deserialize)]
pub struct Project {
    pub root: PathBuf,
    pub file_tree: FileTree,
    pub symbol_table: SymbolTable,
    pub file_path: String,
}

impl Project {
    /// load an existing project or index a new one
    pub fn load_or_create_project(cwd: &str) -> Result<Project, MyError> {
        let canonical = Path::new(cwd).canonicalize().map_err(|e| MyError::OtherError{info: format!("Path not accessible: {}", e)})?;

        if !canonical.is_dir() {
            return Err(MyError::DirNotExistError{dir: format!("'{}' is not a directory", canonical.display())});
        }
        let file_path = format!("{}/{}.json", PARAS.outpath, canonical.file_name().map_or(cwd.replace("/", "_"), |n| n.to_str().unwrap().to_string()));
        let tmp_file_path = Path::new(&file_path);
        if tmp_file_path.exists() && tmp_file_path.is_file() {
            // Return existing project if found
            match std::fs::read_to_string(&tmp_file_path) {
                Ok(content) => {
                    event!(Level::INFO, "load Project `{}`", file_path);
                    let mut project = serde_json::from_str::<Project>(&content).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
                    project.update_and_save()?;
                    Ok(project)
                },
                Err(e) => {
                    event!(Level::ERROR, "read Project file `{}` error: {:?}", file_path, e);
                    Err(MyError::OtherError{info: format!("read Project file `{}` error: {:?}", file_path, e)})
                },
            }
        } else {
            // Scan directory
            let mut file_tree = FileTree::new();
            let mut symbol_table = SymbolTable::new();

            event!(Level::INFO, "Indexing new project: {}", canonical.display());
            let file_count = scan_directory(&canonical, &mut file_tree, u64::MAX, false)?;
            event!(Level::INFO, "Indexed {} files for {}", file_count, canonical.display());

            // symbol extraction
            event!(Level::INFO, "Starting symbol extraction for {}...", canonical.display());
            match extract_all_symbols(&canonical, &mut file_tree, &mut symbol_table) {
                Ok(count) => event!(Level::INFO, "Extracted {} symbols for {}", count, canonical.display()),
                Err(e) => event!(Level::INFO, "Symbol extraction failed for {}: {}", canonical.display(), e),
            }

            let project = Project {
                root: canonical,
                file_tree: file_tree,
                symbol_table: symbol_table,
                file_path,
            };

            // save this project
            project.save()?;

            Ok(project)
        }
    }

    /// save current project
    pub fn save(&self) -> Result<(), MyError> {
        let json_str = serde_json::to_string(&self).map_err(|e| MyError::ToJsonStirngError{uuid: "".to_string(), error: e})?;
        std::fs::write(&self.file_path, json_str).map_err(|e| MyError::WriteFileError{file: self.file_path.clone(), error: e})
    }

    /// update current project, then save to file
    fn update_and_save(&mut self) -> Result<(), MyError> {
        let count_file = scan_directory(&self.root, &mut self.file_tree, u64::MAX, true)?;
        let count_symbol = match extract_all_symbols(&self.root, &mut self.file_tree, &mut self.symbol_table) {
            Ok(count) => {
                event!(Level::INFO, "Extracted {} symbols for {}", count, self.root.display());
                count
            },
            Err(e) => {
                event!(Level::INFO, "Symbol extraction failed for {}: {}", self.root.display(), e);
                0
            },
        };
        if count_file > 0 || count_symbol > 0 {
            self.save()?
        }
        Ok(())
    }

    /// get file tree structure, depth 0 = unlimit
    /// 获取项目代码的文件结构
    pub fn get_structure(&self, depth: usize) -> String {
        self.file_tree.render_tree(depth)
    }

    /// get source code or signature by symbols
    /// 从指定symbols中获取源代码或签名
    fn get_src_code_or_signature_by_symbol(&self, symbols: &[Symbol], only_signature: bool) -> Result<String, MyError> {
        let mut result_vec: Vec<String> = Vec::new();
        for s in symbols {
            if only_signature {
                result_vec.push(format!("`source file: {}`\n```\n{}\n```", s.file, &s.signature));
            } else {
                let src_code = self.symbol_table.get_src_code_by_symbol(&self.root, &s)?;
                result_vec.push(format!("`source file: {}`\n```\n{}\n```", s.file, src_code));
            }
        }
        Ok(result_vec.join("\n\n"))
    }

    /// search symbols
    /// 从所有symbols名称中搜索指定query，忽略大小写
    pub fn search_symbols(&self, query: &str, limit: usize, only_signature: bool) -> Result<String, MyError> {
        let mut symbols = self.symbol_table.search(query, limit);
        symbols.sort_by(|a, b| a.file.cmp(&b.file).then(a.line_range.0.cmp(&b.line_range.0)));
        self.get_src_code_or_signature_by_symbol(&symbols, only_signature)
    }

    /// grep search
    /// 读取项目中每个文件，用每行正则匹配指定的pattern，匹配到则记录行内容，以及前后各扩展2行的内容
    pub fn grep_code(&self, pattern: &str, only_code: bool) -> Result<String, MyError> {
        let mut grep_vec: Vec<String> = Vec::new();
        let grep_result = grep_with_scope(&self.root, &self.file_tree, pattern, usize::MAX, 2, only_code)?;
        for m in grep_result.matches {
            //grep_vec.push(format!("/// source file: {}\n{}\n{}\n{}", m.file, m.context_before.join("\n"), m.text, m.context_after.join("\n")));
            grep_vec.push(format!("`source file: {}`\n```{}\n{}\n{}\n```",
                m.file,
                m.context_before.iter().fold("".to_string(), |acc, l| if acc.is_empty() && l.is_empty() { acc } else { format!("{}\n{}", acc, l) }), // skip consecutive leading spaces
                m.text,
                //m.context_after.iter().rev().fold("".to_string(), |acc, l| if acc.is_empty() && l.is_empty() { acc } else if acc.is_empty() { acc } { format!("{}\n{}", l, acc) }), // skip consecutive trailing spaces
                m.context_after.iter().rev().fold("".to_string(), |acc, l| {
                    if acc.is_empty() && l.is_empty() {
                        acc
                    } else if acc.is_empty() {
                        l.to_string()
                    } else {
                        format!("{}\n{}", l, acc)
                    }
                }), // skip consecutive trailing spaces
            ));
        }
        Ok(grep_vec.join("\n\n"))
    }

    /*
    /// get symbol source code
    /// 获取`文件名::symbol名`对应的symbol，返回源代码或签名
    fn get_symbol_source_code_from_file(&self, symbol_name: &str, file: &str, only_signature: bool) -> Result<String, MyError> {
        let (src_code, signature) = self.symbol_table.get_src_code_signature_by_symbol_file(&self.root, symbol_name, file)?;
        if only_signature {
            Ok(signature)
        } else {
            Ok(src_code)
        }
    }
    */

    /// Find callers of a symbol using tree-sitter call-expression queries
    /// Falls back to regex for files without tree-sitter support
    /// 获取`文件名::symbol名`对应的symbol，遍历每个文件，搜索直接调用该symbol的函数或方法
    fn find_callers(&self, symbol_name: &str, file: &str, limit: usize) -> Result<Vec<CallerInfo>, MyError> {
        // Verify symbol exists
        let _sym = self.symbol_table.get(file, symbol_name).ok_or_else(|| MyError::OtherError{info: format!("Symbol '{}' not found in '{}'", symbol_name, file)})?;

        let mut callers_vec = Vec::new();
        let mut callers_set: HashSet<CallerInfo> = HashSet::new();

        for (k, v) in self.file_tree.files.iter() {
            let rel_path = k.clone();
            let language = v.language;
            let abs_path = self.root.join(&rel_path);

            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let file_callers = if language.has_tree_sitter_support() {
                find_callers_ast(&source, &rel_path, language, symbol_name, file)
            } else {
                find_callers_regex(&source, &rel_path, symbol_name, file)
            };

            for caller in file_callers {
                if !callers_set.contains(&caller) {
                    callers_set.insert(caller.clone());
                    callers_vec.push(caller);
                    if callers_vec.len() >= limit {
                        return Ok(callers_vec);
                    }
                }
            }
        }

        Ok(callers_vec)
    }

    /// get all caller helper
    fn get_all_caller_helper(&self, children_caller_info: &[CallerInfo], caller_mermaid: &mut AllCallers) -> Result<(), MyError> {
        for child_c in children_caller_info {
            if let Some(caller_function) = &child_c.caller_function {
                let tmp_symbol_file = format!("{}::{}", &caller_function, &child_c.file);
                if !caller_mermaid.symbol_file.contains(&tmp_symbol_file) {
                    caller_mermaid.symbol_file.insert(tmp_symbol_file);

                    // insert child caller info
                    let (src_code, signature) = self.symbol_table.get_src_code_signature_by_symbol_file(&self.root, &caller_function, &child_c.file)?; // (source code, signature)
                    caller_mermaid.info_src_map.insert(child_c.clone(), (src_code, signature));

                    let parent_caller_info = self.find_callers(&caller_function, &child_c.file, usize::MAX)?;
                    if !parent_caller_info.is_empty() {
                        self.get_all_caller_helper(&parent_caller_info, caller_mermaid)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// get caller chains
    /// 获取`文件名::symbol名`对应的symbol，遍历每个文件，搜索直接调用该symbol的函数或方法，然后递归搜索，最终得到直接、间接调用指定`文件名::symbol名`的所有方法或函数
    pub fn get_all_callers(&self, symbol_name: &str, file: &str) -> Result<AllCallers, MyError> {
        let init_info = CallerInfo{file: file.to_string(), line: 0, text: "".to_string(), caller_function: Some(symbol_name.to_string())};
        let mut caller_mermaid = AllCallers::new();
        self.get_all_caller_helper(&[init_info], &mut caller_mermaid)?;
        Ok(caller_mermaid)
    }

    /// get all methods of symbol
    /// 获取指定结构体、枚举、class的所有方法的源代码或签名
    pub fn get_methods(&self, symbol_name: &str, only_signature: bool) -> Result<String, MyError> {
        let mut symbols = self.symbol_table.get_methods(symbol_name);
        symbols.sort_by(|a, b| a.file.cmp(&b.file).then(a.line_range.0.cmp(&b.line_range.0)));
        self.get_src_code_or_signature_by_symbol(&symbols, only_signature)
    }

    /// list symbols
    /// 获取项目代码中所有symbol的源代码或签名，可指定一个文件进行限制，也可以限制symbol的kind类型
    pub fn get_code_or_signature(&self, kind_filter: Option<SymbolKind>, file_filter: Option<String>, only_signature: bool) -> Result<String, MyError> {
        // filter by file
        let mut results: Vec<Symbol> = if let Some(file) = file_filter {
            self.symbol_table.list_by_file(&file)
        } else {
            self.symbol_table.all_symbols()
        };

        // filter by kind
        if let Some(kind) = kind_filter {
            results.retain(|s| s.kind == kind);
        }

        results.sort_by(|a, b| a.file.cmp(&b.file).then(a.line_range.0.cmp(&b.line_range.0)));
        //results.truncate(limit);

        // get symbol source code or signature string
        self.get_src_code_or_signature_by_symbol(&results, only_signature)
    }

    /// get all symbols signature
    /// 获取项目代码中所有symbol的签名
    pub fn get_all_signature(&self) -> String {
        let mut sig: Vec<String> = Vec::new();
        for s in self.symbol_table.all_symbols() {
            sig.push(format!("`source file: {}`\n```\n{}\n```", s.file, s.signature));
        }
        sig.join("\n\n")
    }
}

/// find function name
fn find_function_name(mut node: Node, source: &str, func_method: &[&str], name: &Option<&str>) -> Option<String> {
    loop {
        if func_method.contains(&node.kind()) {
            let name_node = if let Some(field) = name.as_ref() {
                node.child_by_field_name(field)?
            } else {
                // no field name, get first identifier
                node.children(&mut node.walk()).find(|n| n.kind() == "identifier")?
            };
            return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
        }
        node = node.parent()?;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CallerInfo {
    pub file: String,
    pub line: usize,
    pub text: String,
    pub caller_function: Option<String>,
}

/// AST-aware caller detection: parse the file, run the callers query
/// and check if any call-expression callee matches the target symbol name
fn find_callers_ast(source: &str, rel_path: &str, language: Language, symbol_name: &str, definition_file: &str) -> Vec<CallerInfo> {
    let config = match get_language_config(language) {
        Some(c) => c,
        None => return find_callers_regex(source, rel_path, symbol_name, definition_file),
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&config.language).is_err() {
        return find_callers_regex(source, rel_path, symbol_name, definition_file);
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return find_callers_regex(source, rel_path, symbol_name, definition_file),
    };

    let query = match tree_sitter::Query::new(&config.language, config.callers_query) {
        Ok(q) => q,
        Err(_) => return find_callers_regex(source, rel_path, symbol_name, definition_file),
    };

    let capture_names: Vec<String> = query.capture_names().iter().map(|s| s.to_string()).collect();
    let callee_idx = capture_names.iter().position(|n| n == "callee");

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let mut callers = Vec::new();

    while let Some(m) = matches.next() {
        for cap in m.captures {
            if Some(cap.index as usize) == callee_idx {
                let text = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                if text == symbol_name {
                    let line_num = cap.node.start_position().row + 1;
                    // Skip the definition itself
                    if rel_path == definition_file {
                        let line_text = source
                            .lines()
                            .nth(line_num - 1)
                            .unwrap_or("");
                        if is_definition_line(line_text, symbol_name, language) {
                            continue;
                        }
                    }
                    let line_text = source
                        .lines()
                        .nth(line_num - 1)
                        .map(|l| l.trim().to_string())
                        .unwrap_or_default();
                    let caller_function = find_function_name(cap.node, source, &config.func_method, &config.name);

                    callers.push(CallerInfo {
                        file: rel_path.to_string(),
                        line: line_num,
                        text: line_text,
                        caller_function,
                    });
                }
            }
        }
    }

    callers
}

/// Regex fallback for files without tree-sitter support
fn find_callers_regex(source: &str, rel_path: &str, symbol_name: &str, definition_file: &str) -> Vec<CallerInfo> {
    let pattern = match regex::Regex::new(&regex::escape(symbol_name)) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let mut callers = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        if pattern.is_match(line) {
            // Skip the definition itself
            if rel_path == definition_file
                && (line.contains(&format!("fn {}", symbol_name))
                    || line.contains(&format!("def {}", symbol_name))
                    || line.contains(&format!("function {}", symbol_name))
                    || line.contains(&format!("func {}", symbol_name)))
            {
                continue;
            }

            callers.push(CallerInfo {
                file: rel_path.to_string(),
                line: line_num + 1,
                text: line.trim().to_string(),
                caller_function: None,
            });
        }
    }

    callers
}

fn is_definition_line(line: &str, name: &str, language: Language) -> bool {
    match language {
        Language::Rust => line.contains(&format!("fn {}", name)),
        Language::Python => line.contains(&format!("def {}", name)),
        Language::TypeScript | Language::JavaScript => line.contains(&format!("function {}", name)) || line.contains(&format!("{} =", name)),
        Language::Go => line.contains(&format!("func {}", name)),
        _ => false,
    }
}
