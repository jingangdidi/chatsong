use std::process::Command;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
//use tokio::process::Command;

use crate::{
    error::MyError,
    tools::built_in_tools::BuiltIn,
};

/// params for run command
#[derive(Deserialize)]
struct Params {
    command: String,
    args:    Option<Vec<String>>,
}

/// built-in tool
pub struct RunCommand;

impl RunCommand {
    /// new
    pub fn new() -> Self {
        RunCommand
    }
}

impl BuiltIn for RunCommand {
    /// get tool name
    fn name(&self) -> String {
        "run_command".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        format!("Execute a {} command in the system shell and return the standard output and error streams. Supports common shell operations such as 'cargo build', 'mkdir tmp', etc. Returns execution status, stdout, and stderr for debugging and validation. IMPORTANT: You must CALL this tool (not write it as text) to run a command.", std::env::consts::OS)
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute, IMPORTANT: This parameter only specifies the command name and does not include any parameters.",
                },
                "args": {
                    "type": ["array", "null"],
                    "items": {"type": "string"},
                    "description": "The list of args.",
                },
            },
            "required": ["command"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

        // prepare command
        let mut tool_cmd = Command::new(&params.command);

        // 处理参数
        let args_string = if let Some(a) = &params.args {
            tool_cmd.args(a);
            format!(
                "{}",
                a.iter().map(|arg| if arg.contains(" ") {
                        format!("\"{}\"", arg)
                    } else {
                        arg.clone()
                    }
                ).collect::<Vec<_>>().join(" "),
            )
        } else {
            String::new()
        };

        // 执行并捕获输出
        // 注意：使用 .output() 比 spawn -> wait -> read 更安全，可避免管道缓冲区死锁
        let output = tool_cmd
            .output()
            //.await
            .map_err(|e| MyError::CommandError{info: format!("failed to execute `{}{}`: {:?}", &params.command, args_string, e)})?;

        // 检查状态
        if !output.status.success() {
            return Err(MyError::CommandError{info: format!("execute `{}{}` failed with status: {}", &params.command, args_string, output.status)})
        }

        // 格式化结果
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        Ok(match (stderr.is_empty(), stdout.is_empty()) {
            (false, false) => (format!("stdout:\n{}stderr:\n{}", stderr, stdout), None),
            (false, true) => (format!("stderr:\n{}", stderr), None),
            (true, false) => (format!("stdout:\n{}", stdout), None),
            (true, true) => ("successfully run command".to_string(), None),
        })
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        if is_en {
            Ok(Some(format!("Do you allow running this command: {} {}?{}", params.command, params.args.unwrap_or_default().join(" "), info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用该命令：{} {}？{}", params.command, params.args.unwrap_or_default().join(" "), info.unwrap_or_default())))
        }
    }
}
