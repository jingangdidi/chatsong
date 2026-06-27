use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::BuiltIn,
};

pub struct GetAllMemory;

impl GetAllMemory {
    pub fn new() -> Self {
        GetAllMemory
    }
}

impl BuiltIn for GetAllMemory {
    /// get tool name
    fn name(&self) -> String {
        "get_all_memory".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Retrieves all extracted long-term memories. Returns the memories as a single string.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {},
            "required": [],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, _args: &str) -> Result<(String, Option<String>), MyError> {
        Ok(("".to_string(), None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
