use std::collections::{HashSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::MyError;

/// all programming language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Java,
    C,
    Cpp,
    Ruby,
    Shell,
    Markdown,
    Json,
    Yaml,
    Toml,
    Html,
    Css,
    Sql,
    Other,
}

impl Language {
    /// str to Language
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "py" | "pyi" => Language::Python,
            "ts" | "tsx" => Language::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Language::Cpp,
            "rb" => Language::Ruby,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            "md" | "mdx" => Language::Markdown,
            "json" | "jsonc" => Language::Json,
            "yml" | "yaml" => Language::Yaml,
            "toml" => Language::Toml,
            "html" | "htm" => Language::Html,
            "css" | "scss" | "less" => Language::Css,
            "sql" => Language::Sql,
            _ => Language::Other,
        }
    }

    /// file path to Language
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|e| e.to_str())
            .map(Self::from_extension)
            .unwrap_or(Language::Other)
    }

    /// Whether this language supports tree-sitter symbol extraction
    /// currently only support Rust, Python, TypeScript, JavaScript, Go
    pub fn has_tree_sitter_support(&self) -> bool {
        matches!(
            self,
            Language::Rust | Language::Python | Language::TypeScript | Language::JavaScript | Language::Go
        )
    }

    /// convert to string
    pub fn convert_to_string(&self) -> String {
        match self {
            Language::Rust => "Rust".to_string(),
            Language::Python => "Python".to_string(),
            Language::TypeScript => "TypeScript".to_string(),
            Language::JavaScript => "JavaScript".to_string(),
            Language::Go => "Go".to_string(),
            Language::Java => "Java".to_string(),
            Language::C => "C".to_string(),
            Language::Cpp => "Cpp".to_string(),
            Language::Ruby => "Ruby".to_string(),
            Language::Shell => "Shell".to_string(),
            Language::Markdown => "Markdown".to_string(),
            Language::Json => "Json".to_string(),
            Language::Yaml => "Yaml".to_string(),
            Language::Toml => "Toml".to_string(),
            Language::Html => "Html".to_string(),
            Language::Css => "Css".to_string(),
            Language::Sql => "Sql".to_string(),
            Language::Other => "Other".to_string(),
        }
    }
}

/// symbol kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    Constant,
    Variable,
    Type,
    Module,
    Import,
    Other,
}

impl SymbolKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "function" | "fn" | "func" => Some(SymbolKind::Function),
            "method" => Some(SymbolKind::Method),
            "class" => Some(SymbolKind::Class),
            "struct" => Some(SymbolKind::Struct),
            "enum" => Some(SymbolKind::Enum),
            "trait" => Some(SymbolKind::Trait),
            "interface" => Some(SymbolKind::Interface),
            "constant" | "const" => Some(SymbolKind::Constant),
            "variable" | "var" | "let" => Some(SymbolKind::Variable),
            "type" => Some(SymbolKind::Type),
            "module" | "mod" => Some(SymbolKind::Module),
            "import" | "use" => Some(SymbolKind::Import),
            _ => None,
        }
    }
}

/// symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub byte_range: (usize, usize),
    pub line_range: (usize, usize),
    pub language: Language,
    /// First line of the symbol (e.g. function signature)
    pub signature: String,
    /// Agent-set human-readable description
    pub definition: Option<String>,
    /// Parent symbol name (e.g. struct for a method)
    pub parent: Option<String>,
}

/// symbol table
#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolTable {
    /// Primary store, key: `file::name`, value: Symbol
    pub symbols: HashMap<String, Symbol>,
    /// Secondary index, key: symbol name, value: `file::name`
    pub by_name: HashMap<String, HashSet<String>>,
    /// Secondary index, key: file path, value: `file::name`
    pub by_file: HashMap<String, HashSet<String>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            by_name: HashMap::new(),
            by_file: HashMap::new(),
        }
    }

    /// insert new symbol, primary key: `file::name`
    pub fn insert(&mut self, symbol: Symbol) {
        let key = format!("{}::{}", &symbol.file, &symbol.name);

        // Update secondary indices
        self.by_name
            .entry(symbol.name.clone())
            .or_insert_with(HashSet::new)
            .insert(key.clone());
        self.by_file
            .entry(symbol.file.clone())
            .or_insert_with(HashSet::new)
            .insert(key.clone());

        //self.symbols.insert(key, symbol);
        match self.symbols.get(&key) {
            Some(s) => {
                if s.parent.is_none() {
                    self.symbols.insert(key, symbol);
                }
            },
            None => {
                self.symbols.insert(key, symbol);
            },
        }
    }

    /// remove file from by_file and get primary keys
    /// then remove primary key and symbol from symbols
    /// remove name and primary keys from by_name
    pub fn remove_file(&mut self, file: &str) {
        if let Some(keys) = self.by_file.remove(file) {
            for key in &keys {
                if let Some(sym) = self.symbols.remove(key) {
                    if let Some(name_set) = self.by_name.get_mut(&sym.name) {
                        name_set.remove(key);
                        if name_set.is_empty() {
                            self.by_name.remove(&sym.name);
                        }
                    }
                }
            }
        }
    }

    /// get Symbol by file and name, primary key is `file::name`
    pub fn get(&self, file: &str, name: &str) -> Option<Symbol> {
        let key = format!("{}::{}", file, name); // primary key
        self.symbols.get(&key).map(|v| v.clone())
    }

    /// get symbols which symbol name contain query
    pub fn search(&self, query: &str, limit: usize) -> Vec<Symbol> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        for (_, v) in self.symbols.iter() {
            if v.name.to_lowercase().contains(&query_lower) {
                results.push(v.clone());
                if results.len() >= limit {
                    break;
                }
            }
        }
        results
    }

    /// get all primary keys of by_file, then get all symbols by primary keys
    pub fn list_by_file(&self, file: &str) -> Vec<Symbol> {
        if let Some(keys) = self.by_file.get(file) {
            keys.iter()
                .filter_map(|key| self.symbols.get(key).map(|v| v.clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// get all symbols
    pub fn all_symbols(&self) -> Vec<Symbol> {
        self.symbols.iter().map(|(_, v)| v.clone()).collect()
    }

    /// get all symbols number
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// get all methods of symbol
    pub fn get_methods(&self, symbol_name: &str) -> Vec<Symbol> {
        let mut methods: Vec<Symbol> = Vec::new();
        for v in self.symbols.values() {
            if let Some(parent) = &v.parent {
                if parent == symbol_name {
                    methods.push(v.clone())
                }
            }
        }
        methods
    }

    /// get source code by symbol
    pub fn get_src_code_by_symbol(&self, root: &Path, symbol: &Symbol) -> Result<String, MyError> {
        let abs_path = root.join(&symbol.file);
        let source = std::fs::read_to_string(&abs_path).map_err(|e| MyError::OtherError{info: format!("Failed to read '{}': {}", symbol.file, e)})?;
        let start = symbol.byte_range.0;
        let end = symbol.byte_range.1.min(source.len());
        Ok(source[start..end].to_string())
    }

    /// get source code by symbol and file
    pub fn get_src_code_signature_by_symbol_file(&self, root: &Path, symbol_name: &str, file: &str) -> Result<(String, String), MyError> {
        let sym = self.get(file, symbol_name).ok_or_else(|| MyError::OtherError{info: format!("Symbol '{}' not found in '{}'", symbol_name, file)})?;
        Ok((self.get_src_code_by_symbol(root, &sym)?, sym.signature.clone()))
    }
}
