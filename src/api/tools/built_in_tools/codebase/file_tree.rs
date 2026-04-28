use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::tools::built_in_tools::codebase::symbol_table::Language;

/// language count of file tree
#[derive(Debug, Serialize)]
pub struct LanguageCount {
    pub language: Language,
    pub count: usize,
}

/// file category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileMark {
    Documentation,
    Ignore,
    Test,
    Config,
    Generated,
    Custom,
}

/// file entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// relative path
    pub rel_path: String,
    /// file size
    pub size: u64,
    /// last modified time
    pub modified: DateTime<Utc>,
    /// programming laguage
    pub language: Language,
    /// Agent-set human-readable definition of what this file does
    pub definition: Option<String>,
    /// Agent-set marks for categorization
    pub marks: Vec<FileMark>,
    /// Whether symbols have been extracted from this file
    pub symbols_extracted: bool,
}

impl FileEntry {
    pub fn new(rel_path: String, size: u64, modified: DateTime<Utc>) -> Self {
        let language = Language::from_path(Path::new(&rel_path));
        Self {
            rel_path,
            size,
            modified,
            language,
            definition: None,
            marks: Vec::new(),
            symbols_extracted: false,
        }
    }
}

/// recursive node
enum TreeNode {
    File,
    Dir(BTreeMap<String, TreeNode>),
}

/// file tree
#[derive(Serialize, Deserialize)]
pub struct FileTree {
    pub files: HashMap<String, FileEntry>,
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// insert new file, key: relative path, value: FileEntry
    pub fn insert(&mut self, entry: FileEntry) {
        self.files.insert(entry.rel_path.clone(), entry);
    }

    /// remove file, return FileEntry
    pub fn remove(&mut self, rel_path: &str) -> Option<FileEntry> {
        self.files.remove(rel_path).map(|v| v)
    }

    /// get FileEntry by relative path
    pub fn get(&self, rel_path: &str) -> Option<FileEntry> {
        self.files.get(rel_path).map(|r| r.clone())
    }

    /// get total files number
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// stat all files language count
    pub fn language_breakdown(&self) -> Vec<LanguageCount> {
        let mut counts: HashMap<Language, usize> = HashMap::new();
        for (_, v) in self.files.iter() {
            *counts.entry(v.language).or_insert(0) += 1;
        }
        let mut result: Vec<_> = counts
            .into_iter()
            .map(|(language, count)| LanguageCount { language, count })
            .collect();
        result.sort_by(|a, b| b.count.cmp(&a.count));
        result
    }

    /// get all files relative path vec
    pub fn all_paths(&self) -> Vec<String> {
        self.files.iter().map(|(k, _)| k.clone()).collect()
    }

    /// Render a tree-like structure string, similar to the `tree` command.
    /// `depth` limits how many directory levels deep to show (0 = unlimited).
    pub fn render_tree(&self, depth: usize) -> String {
        // Collect all paths into a sorted tree structure
        let mut paths: Vec<String> = self.all_paths();
        paths.sort();

        // Build a tree from paths
        let mut root: BTreeMap<String, TreeNode> = BTreeMap::new();
        for path in &paths {
            let parts: Vec<&str> = path.split('/').collect();
            insert_into_tree(&mut root, &parts, 0);
        }

        let mut output = String::new();
        render_tree_node(&root, &mut output, "", depth, 0);
        output
    }
}

fn insert_into_tree(tree: &mut BTreeMap<String, TreeNode>, parts: &[&str], idx: usize) {
    if idx >= parts.len() {
        return;
    }
    let name = parts[idx].to_string();
    if idx == parts.len() - 1 {
        // Leaf file
        tree.entry(name).or_insert(TreeNode::File);
    } else {
        // Directory
        let node = tree
            .entry(name)
            .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));
        if let TreeNode::Dir(children) = node {
            insert_into_tree(children, parts, idx + 1);
        }
    }
}

fn render_tree_node(
    tree: &BTreeMap<String, TreeNode>,
    output: &mut String,
    prefix: &str,
    max_depth: usize,
    current_depth: usize,
) {
    if max_depth > 0 && current_depth >= max_depth {
        return;
    }

    let entries: Vec<_> = tree.iter().collect();
    for (i, (name, node)) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        match node {
            TreeNode::File => {
                output.push_str(&format!("{}{}{}\n", prefix, connector, name));
            }
            TreeNode::Dir(children) => {
                output.push_str(&format!("{}{}{}\n", prefix, connector, name));
                render_tree_node(
                    children,
                    output,
                    &format!("{}{}", prefix, child_prefix),
                    max_depth,
                    current_depth + 1,
                );
            }
        }
    }
}
