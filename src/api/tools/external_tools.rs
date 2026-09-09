use std::collections::HashMap;
use std::process::{Command, Stdio};

use serde::Deserialize;
use serde_json::Value;
use tracing::{event, Level};
use uuid::Uuid;

use crate::{
    error::MyError,
    tools::MyTools,
    tools::{
        parse_tool_args,
        ArgFixSpec,
    },
};

/// single external tool
#[derive(Deserialize)]
pub struct SingleExternalTool {
    pub name:        String,
    pub command:     String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args:        Vec<String>,
    pub description: String,
    pub schema:      Value, // for LLM api function calling
    #[serde(default)]
    pub approval:    bool, // ask for approval
}

impl SingleExternalTool {
    /// run command
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //println!("\n\n{}\n{:?}\n{}\n\n", self.command, self.args, args);
        // prepare command
        let mut tool_cmd = Command::new(&self.command);
        if !self.args.is_empty() {
            tool_cmd.args(&self.args);
        }
        if !args.is_empty() {
            let mut args_vec: Vec<String> = Vec::new();
            let mut value: String;
            let json_value: Value = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
            //let json_value: Value = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
            if let Value::Object(map) = json_value {
                for (k, v) in map {
                    // get value
                    match v {
                        Value::Null => continue,
                        Value::Bool(b) => {
                            if b {
                                value = "".to_string();
                            } else {
                                continue
                            }
                        },
                        Value::Number(n) => value = n.to_string(),
                        Value::String(s) => value = s,
                        Value::Array(_) => return Err(MyError::OtherError{info: format!("external tool args value only support string, number, boolean, not Array: {}", args)}),
                        Value::Object(_) => return Err(MyError::OtherError{info: format!("external tool args value only support string, number, boolean, not Object: {}", args)}),
                    }
                    // push to vec
                    if k.starts_with("positional") { // 位置参数，没有`--`或`-`
                        args_vec.push(value);
                    } else {
                        args_vec.push(format!("--{k}"));
                        if !value.is_empty() {
                            args_vec.push(value);
                        }
                    }
                }
            } else {
                return Err(MyError::OtherError{info: format!("external tool args `{:?}` must be object", args)})
            }
            event!(Level::INFO, "external tool raw json args: {}, args: {:?}", args, args_vec);
            tool_cmd.args(&args_vec);
        }
        tool_cmd
            .stdout(Stdio::piped()) // pipe stdout
            .stderr(Stdio::piped()) // pipe stderr
            // 强制 Python 的标准输入输出使用 UTF-8
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1");
        let tool_cmd = tool_cmd
            .spawn()
            .map_err(|e| MyError::CommandError{info: format!("failed to execute {} ({}): {:?}", &self.name, &self.command, e)})?
            .wait_with_output()
            .map_err(|e| MyError::CommandError{info: format!("failed while waiting for {} ({}): {}", self.name, self.command, e)})?;
        // stderr
        let stderr: Option<String> = {
            let buffer = String::from_utf8_lossy(&tool_cmd.stderr);
            if buffer.trim().is_empty() {
                None
            } else {
                Some(format!("stderr:\n{}", buffer.trim()))
            }
        };
        // stdout
        let stdout: Option<String> = {
            let buffer = String::from_utf8_lossy(&tool_cmd.stdout);
            if buffer.trim().is_empty() {
                None
            } else {
                Some(format!("stdout:\n{}", buffer.trim()))
            }
        };
        // 在收集 stderr/stdout 后再检查失败状态
        if !tool_cmd.status.success() {
            return Err(MyError::CommandError {
                info: format!(
                    "execute {} ({}) failed with status: {}\n{}\n{}",
                    &self.name,
                    &self.command,
                    tool_cmd.status,
                    stdout.as_deref().unwrap_or(""),
                    stderr.as_deref().unwrap_or(""),
                ),
            });
        }
        // result
        Ok(match (stderr, stdout) {
            (Some(e), Some(o)) => (format!("{e}\n{o}"), None),
            (Some(e), None)    => (e, None),
            (None, Some(o))    => (o, None),
            (None, None)       => ("".to_string(), None),
        })
    }
}

/// all external tools
pub struct ExternalTools {
    pub id_map: HashMap<String, SingleExternalTool>, // key: tool id, value: SingleExternalTool
}

impl ExternalTools {
    pub fn new(external_tools: Vec<SingleExternalTool>) -> Self {
        let tmp_uuid = Uuid::new_v4().to_string();
        Self {id_map: external_tools.into_iter().map(|t| (tmp_uuid[0..8].to_string(), t)).collect::<HashMap<String, SingleExternalTool>>()}
    }
}

impl MyTools for ExternalTools {
    /// run command
    fn run(&self, id: &str, args: &str) -> Result<(String, Option<String>), MyError> {
        match self.id_map.get(id) {
            Some(tool) => tool.run(args),
            None => Err(MyError::ToolNotExistError{id: id.to_string(), info: "ExternalTools::run()".to_string()}),
        }
    }

    /// get all selected tools name (name format: `name__id`, max name length is 26), description and schema
    fn get_desc_and_schema(&self, selected_tools: Vec<String>) -> Vec<(String, String, Value)> {
        self.id_map.iter().filter(|(k, _)| selected_tools.contains(&k)).map(|(k, v)| (format!("{}__{}", v.name, k), v.description.clone(), v.schema.clone())).collect()
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
            Some(tool) => if tool.approval {
                if is_en {
                   Ok(Some(format!("Do you allow calling the {} ({}) tool?{}", tool.name, args, info.unwrap_or_default())))
                } else {
                   Ok(Some(format!("是否允许调用 {} ({}) 工具？{}", tool.name, args, info.unwrap_or_default())))
                }
            } else {
                Ok(None)
            },
            None => Err(MyError::ToolNotExistError{id: id.to_string(), info: "ExternalTools::get_approval()".to_string()}),
        }
    }
}
