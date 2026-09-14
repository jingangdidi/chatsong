use chrono::Local;
use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use uuid::Uuid;

use crate::{
    error::MyError,
    info::{
        DATA,
        Info,
    },
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::{
            BuiltIn,
            filesystem::utils::read_file_helper,
        },
    },
};

/// params for integer load_chat_log
#[derive(Deserialize)]
pub struct Params {
    pub file_path: String,
}

pub struct LoadChatLog;

impl LoadChatLog {
    pub fn new() -> Self {
        LoadChatLog
    }
}

impl BuiltIn for LoadChatLog {
    /// get tool name
    fn name(&self) -> String {
        "load_chat_log".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Load chat log from a file. Use this tool when you need to load chat log.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path of the file to load.",
                },
            },
            "required": ["file_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        let content = read_file_helper(&params.file_path.replace("\\", "/"))?;
        let mut chat_info = serde_json::from_str::<Info>(&content).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let uuid = Uuid::new_v4().to_string();
        chat_info.uuid = uuid.clone();
        chat_info.chat_name = "loaded chat".to_string();
        chat_info.msg_len = chat_info.messages.iter().filter(|m| !m.data.is_hide()).count();
        chat_info.file = format!("{}/{}.log", chat_info.uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
        chat_info.num_q.0 = chat_info.messages.len();
        chat_info.num_q.1 = chat_info.get_qa_num_by_idx(chat_info.messages.len()-1).0;
        chat_info.qa_msg_p = (1, 0, false);
        chat_info.save = true;
        chat_info.update_qa_msg_idx();

        let mut data = DATA.lock().unwrap();
        data.insert(uuid.clone(), chat_info);

        Ok((format!("The chat log has been loaded to the output path, uuid: {uuid}"), None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
