use std::path::Path;
use std::process::Command;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
//use tokio::process::Command;

use crate::{
    error::MyError,
    tools::built_in_tools::BuiltIn,
    skills::command_exists,
};

/// params for run script
#[derive(Deserialize)]
struct Params {
    script: String,
    args:   Option<Vec<String>>,
}

/// built-in tool
pub struct RunScript;

impl RunScript {
    /// new
    pub fn new() -> Self {
        RunScript
    }
}

impl BuiltIn for RunScript {
    /// get tool name
    fn name(&self) -> String {
        "run_script".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Execute a script (.py, .sh, .r, etc.) and return the standard output and error streams. IMPORTANT: You must CALL this tool (not write it as text) to run a script.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "script": {
                    "type": "string",
                    "description": "The script to execute, IMPORTANT: This parameter only specifies the script name and does not include any parameters.",
                },
                "args": {
                    "type": ["array", "null"],
                    "items": {"type": "string"},
                    "description": "The list of args.",
                },
            },
            "required": ["script"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

        // 获取解释器（根据扩展名自动判断）
        let interpreter = get_interpreter(&params.script)?;

        // prepare command
        let mut tool_cmd = Command::new(&interpreter);
        tool_cmd.arg(&params.script);
        //println!("args: {:?}", &params.args);
        let args_string = if let Some(a) = params.args {
            let tmp_args: Vec<String> = a.into_iter().map(|arg| if arg.contains(" ") {
                format!("\"{}\"", arg)
            } else {
                arg
            }).collect();
            tool_cmd.args(&tmp_args);
            format!(" {}", tmp_args.join(" "))
        } else {
            "".to_string()
        };
        //println!("args_string: `{}`", args_string);

        /*
        tool_cmd
            .stdout(Stdio::piped()) // pipe stdout
            .stderr(Stdio::piped()); // pipe stderr
        let mut tool_cmd = tool_cmd
            .spawn()
            .map_err(|e| MyError::CommandError{info: format!("failed to execute `{} {}{}`: {:?}", interpreter, script, args_string, e)})?;
        // wait and check status
        let status = tool_cmd.wait()?;
        if !status.success() {
            return Err(MyError::CommandError{info: format!("execute `{} {}{}` failed with status: {}", interpreter, script, args_string, status)})
        }
        // stderr
        let stderr: Option<String> = if let Some(mut stderr) = tool_cmd.stderr {
            let mut buffer = String::new();
            stderr.read_to_string(&mut buffer)?;
            if buffer.is_empty() {
                None
            } else {
                Some(format!("stderr:\n{:?}", buffer.trim()))
            }
        } else {
            None
        };
        // stdout
        let stdout: Option<String> = if let Some(mut stdout) = tool_cmd.stdout {
            let mut buffer = String::new();
            stdout.read_to_string(&mut buffer)?;
            if buffer.is_empty() {
                None
            } else {
                Some(format!("stdout:\n{:?}", buffer.trim()))
            }
        } else {
            None
        };

        // result
        Ok(match (stderr, stdout) {
            (Some(e), Some(o)) => format!("{e}\n{o}"),
            (Some(e), None)    => e,
            (None, Some(o))    => o,
            (None, None)       => "".to_string(),
        })
        */

        // 执行并捕获输出
        // 注意：使用 .output() 比 spawn -> wait -> read 更安全，可避免管道缓冲区死锁
        let output = tool_cmd
            .output() // 捕获 stdout 和 stderr
            //.await
            .map_err(|e| MyError::CommandError{info: format!("failed to execute `{} {}{}`: {:?}", interpreter, &params.script, args_string, e)})?;

        // 检查状态
        if !output.status.success() {
            return Err(MyError::CommandError{info: format!("execute `{} {}{}` failed with status: {}", interpreter, &params.script, args_string, output.status)})
        }

        // 格式化结果
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        Ok(match (stderr.is_empty(), stdout.is_empty()) {
            (false, false) => (format!("stdout:\n{}stderr:\n{}", stderr, stdout), None),
            (false, true) => (format!("stderr:\n{}", stderr), None),
            (true, false) => (format!("stdout:\n{}", stdout), None),
            (true, true) => ("successfully run script".to_string(), None),
        })
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let interpreter = get_interpreter(&params.script)?;
        if is_en {
            Ok(Some(format!("Do you allow running this script: {} {} {}?{}", interpreter, params.script, params.args.unwrap_or_default().join(" "), info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用该脚本：{} {} {}？{}", interpreter, params.script, params.args.unwrap_or_default().join(" "), info.unwrap_or_default())))
        }
    }
}

/// 获取调用脚本的程序
fn get_interpreter(script: &str) -> Result<String, MyError> {
    let script_path = Path::new(script);
    match script_path.extension().and_then(|ext| ext.to_str()) {
        Some("sh") | Some("bash") => Ok("bash".to_string()),
        Some("py") => {
            if command_exists("python3") {
                Ok("python3".to_string())
            } else {
                Ok("python".to_string())
            }
        },
        Some("r") => Ok("Rscript".to_string()),
        Some("js") => Ok("node".to_string()),
        Some("rb") => Ok("ruby".to_string()),
        Some("pl") => Ok("perl".to_string()),
        _ => Err(MyError::OtherError{info: format!("Unsupported script type: `{}`", script)}),
    }
}
