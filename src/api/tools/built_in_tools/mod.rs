use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::MyError,
    tools::MyTools,
};

//mod sum;
//mod subtract;
pub mod filesystem;

//use sum::Sum;
//use subtract::Subtract;
use filesystem::{
    CalculateDirectorySize,
    CreateDirectory,
    DirectoryTree,
    EditFile,
    GetFileInfo,
    ListAllowedDirectories,
    ListDirectory,
    ListDirectoryWithSizes,
    HeadFile,
    MoveFile,
    ReadFile,
    ReadMultipleFiles,
    SearchFiles,
    SearchFilesContent,
    TailFile,
    UnzipFile,
    WriteFile,
    ZipDirectory,
    ZipFiles,
};

/// trait for built-in tools
pub trait BuiltIn: Send + Sync {
    /// get tool name
    fn name(&self) -> String;

    /// get tool description
    fn description(&self) -> String;

    /// get tool schema
    fn schema(&self) -> Value;

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError>;

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError>;
}

/// tool group
#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Group {
    //Calculate,
    FileSystem,
}

impl Group {
    pub fn to_string(&self) -> String {
        match self {
            //Group::Calculate => "calculate".to_string(),
            Group::FileSystem => "file system".to_string(),
        }
    }

    fn from_str(g: &str) -> Result<Self, MyError> {
        match g {
            //"calculate" => Ok(Group::Calculate),
            "file system" => Ok(Group::FileSystem),
            _ => Err(MyError::OtherError{info: format!("can not convert \"{}\" to Group", g)})
        }
    }
}

/// single built-in tool
pub struct SingleBuiltInTool {
    pub tool: Arc<dyn BuiltIn>, // struct impl BuiltIn
    pub group: Group,
}

/// all built-in tools
pub struct BuiltInTools {
    pub id_map: HashMap<String, SingleBuiltInTool>, // key: tool id, value: SingleBuiltInTool
    pub groups: HashSet<Group>, // tool groups
}

impl BuiltInTools {
    /// create BuiltInTools
    pub fn new() -> Result<Self, MyError> {
        let tools: Vec<(Arc<dyn BuiltIn>, Group)> = vec![
            //(Arc::new(Sum::new()), Group::Calculate),
            //(Arc::new(Subtract::new()), Group::Calculate),
            // file system
            (Arc::new(CalculateDirectorySize::new()), Group::FileSystem),
            (Arc::new(CreateDirectory::new()), Group::FileSystem),
            (Arc::new(DirectoryTree::new()), Group::FileSystem),
            (Arc::new(EditFile::new()), Group::FileSystem),
            (Arc::new(GetFileInfo::new()), Group::FileSystem),
            (Arc::new(ListAllowedDirectories::new()), Group::FileSystem),
            (Arc::new(ListDirectory::new()), Group::FileSystem),
            (Arc::new(ListDirectoryWithSizes::new()), Group::FileSystem),
            (Arc::new(HeadFile::new()), Group::FileSystem),
            (Arc::new(MoveFile::new()), Group::FileSystem),
            (Arc::new(ReadFile::new()), Group::FileSystem),
            (Arc::new(ReadMultipleFiles::new()), Group::FileSystem),
            (Arc::new(SearchFiles::new()), Group::FileSystem),
            (Arc::new(SearchFilesContent::new()), Group::FileSystem),
            (Arc::new(TailFile::new()), Group::FileSystem),
            (Arc::new(UnzipFile::new()), Group::FileSystem),
            (Arc::new(WriteFile::new()), Group::FileSystem),
            (Arc::new(ZipDirectory::new()), Group::FileSystem),
            (Arc::new(ZipFiles::new()), Group::FileSystem),
        ];
        let mut id_map: HashMap<String, SingleBuiltInTool> = HashMap::new(); // key: tool id, value: SingleBuiltInTool
        let mut groups: HashSet<Group> = HashSet::new(); // tool groups
        for (tool, group) in tools {
            // check name length must <= 26
            if tool.name().chars().count() > 26 {
                return Err(MyError::OtherError{info: format!("tool name \"{}\" length must <= 26", tool.name())})
            }
            id_map.insert(Uuid::new_v4().to_string(), SingleBuiltInTool{tool, group: group.clone()});
            groups.insert(group);
        }
        Ok(Self{id_map, groups})
    }

    /// select tools by group
    pub fn select_tools_by_group(&self, group: &str) -> Result<Vec<String>, MyError> {
        let group = Group::from_str(group)?;
        let mut selected_tools: Vec<String> = Vec::new();
        for (id, value) in &self.id_map {
            if value.group == group {
                selected_tools.push(id.clone());
            }
        }
        Ok(selected_tools)
    }
}

impl MyTools for BuiltInTools {
    /// run function
    fn run(&self, id: &str, args: &str) -> Result<String, MyError> {
        match self.id_map.get(id) {
            Some(t) => t.tool.run(args),
            None => Err(MyError::ToolNotExistError{id: id.to_string(), info: "BuiltInTools::run()".to_string()}),
        }
    }

    /// get all selected tools name (name format: `name__id`, max name length is 26), description and schema
    fn get_desc_and_schema(&self, selected_tools: Vec<String>) -> Vec<(String, String, Value)> {
        self.id_map.iter().filter(|(k, _)| selected_tools.contains(&k)).map(|(k, v)| (format!("{}__{}", v.tool.name(), k), v.tool.description(), v.tool.schema())).collect()
    }

    /// select all tools, return uuid vector
    fn select_all_tools(&self) -> Vec<String> {
        let mut selected_tools: Vec<String> = Vec::new();
        for id in self.id_map.keys() {
            selected_tools.push(id.clone());
        }
        selected_tools
    }

    /// get approval message
    fn get_approval(&self, id: &str, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        match self.id_map.get(id) {
            Some(t) => t.tool.get_approval(args, info, is_en),
            None => Err(MyError::ToolNotExistError{id: id.to_string(), info: "BuiltInTools::get_approval()".to_string()}),
        }
    }
}
