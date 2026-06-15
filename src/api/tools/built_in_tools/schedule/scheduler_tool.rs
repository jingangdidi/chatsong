use chrono::Local;
use serde::Deserialize;
use serde_json::{json, Value, to_string}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use tokio::sync::oneshot;
use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::{
            BuiltIn,
            schedule::{
                ScheduledJob,
                SchedulerCmd,
                ScheduleType,
                get_cmd_tx,
            }
        },
    },
};

/// 定时任务的参数
#[derive(Debug, Deserialize)]
struct ScheduleConfig {
    //#[serde(rename = "jobType")]
    job_type: String,      // 类型
    #[serde(default)]
    hour:     Option<u8>,  // 时，指定时间执行
    #[serde(default)]
    minute:   Option<u8>,  // 分，指定时间执行
    #[serde(default)]
    secs:     Option<u64>, // 秒，间隔几秒运行一次
}

#[derive(Debug, Deserialize)]
struct Params {
    action:    String,                 // create, delete, list
    #[serde(default)]
    name:      Option<String>,         // 任务名称
    #[serde(default)]
    schedule:  Option<ScheduleConfig>, // 定时任务的参数
    #[serde(default)]
    //#[serde(rename = "toolName")]
    tool_name: Option<String>,         // 要调用的工具
    #[serde(default)]
    //#[serde(rename = "toolArgs")]
    tool_args: Option<Value>,          // 要调用的工具参数
    #[serde(default)]
    //#[serde(rename = "toolId")]
    job_id:    Option<String>,         // 删除任务要用
}

pub struct ScheduleTask;

impl ScheduleTask {
    pub fn new() -> Self {
        ScheduleTask
    }
}

impl BuiltIn for ScheduleTask {
    /// get tool name
    fn name(&self) -> String {
        "schedule_task".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Manages scheduled tasks (cron-like jobs). Use this tool to create, delete, or list periodic tasks that will automatically call another tool at the specified time or interval.

**Important:** When creating a task (action='create'), you MUST specify:
- `name`: a unique task name.
- `schedule` with `job_type` set to either 'interval' (runs every N seconds) or 'daily' (runs at a fixed time each day).
   - For 'interval': provide `secs` (e.g., 60 for 1 minute, 300 for 5 minutes, 3600 for 1 hour).
   - For 'daily': provide `hour` (0-23) and optionally `minute` (0-59, default 0).
- `tool_name`: the exact name of the tool to invoke when the task fires. This tool must already exist in the system.
- `tool_args` (optional): a JSON object with arguments to pass to that tool.

**Example 1:** User says 'Write a poem every minute and save it to poem.txt'
→ You should set action='create', name='write_poem_every_min', schedule={job_type='interval', secs=60}, tool_name='write_poem_to_file', tool_args={\"filename\":\"poem.txt\"}

**Example 2:** User says 'Run check.py every 5 minutes'
→ action='create', name='run_check', schedule={job_type='interval', secs=300}, tool_name='run_python_script', tool_args={\"script\":\"check.py\"}

**Example 3:** User says 'Daily summary of Hacker News at 6:00 AM'
→ action='create', name='hn_summary', schedule={job_type='daily', hour=6, minute=0}, tool_name='summarize_hackernews', tool_args={}

**For deletion:** action='delete', job_id='<the task's ID>'
**For listing:** action='list'
".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "delete", "list"],
                    "description": "Type of operation. Must be one of 'create', 'delete', or 'list'"
                },
                "name": {
                    "type": "string",
                    "description": "The name of the task. Required when action is 'create'"
                },
                "schedule": {
                    "type": "object",
                    "description": "Scheduling rules. Required when action is 'create'",
                    "properties": {
                        "job_type": {
                            "type": "string",
                            "enum": ["interval", "daily"],
                            "description": "Schedule job type. 'interval' runs every N seconds; 'daily' runs at a fixed time each day"
                        },
                        "secs": {
                            "type": "integer",
                            "description": "Number of seconds between intervals. Required when job type is 'interval'"
                        },
                        "hour": {
                            "type": "integer",
                            "description": "Hour (0–23). Required when type is 'daily'"
                        },
                        "minute": {
                            "type": "integer",
                            "description": "Minute (0–59, optional, defaults to 0) when type is 'daily'"
                        }
                    },
                    "required": ["job_type"]
                },
                "tool_name": {
                    "type": "string",
                    "description": "The name of the tool to call when the task triggers. Required when action is 'create'"
                },
                "tool_args": {
                    "type": "object",
                    "description": "A JSON object containing arguments to pass to the tool. Optional, only used when action is 'create'"
                },
                "job_id": {
                    "type": "string",
                    "description": "The ID of the task to delete. Required when action is 'delete'"
                }
            },
            "required": ["action"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //let _params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let _params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        Ok(("".to_string(), None))
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        if is_en {
            Ok(Some(format!("Do you allow calling the schedule_task tool {} method ?{}", params.action, info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 schedule_task 工具 {} 方法？{}", params.action, info.unwrap_or_default())))
        }
    }
}

/// 向后台任务调度器发送任务，并等待结果
pub async fn run_schedule_task(args: &str) -> Result<(String, Option<String>), MyError> {
    let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
    let cmd_tx = get_cmd_tx().unwrap();

    match params.action.as_str() {
        "create" => {
            let name = params
                .name
                .ok_or(MyError::OtherError{info: "create schedule job need a name".to_string()})?;
            let schedule_cfg = params
                .schedule
                .ok_or(MyError::OtherError{info: "create schedule job need schedule config".to_string()})?;
            let tool_name = params
                .tool_name
                .ok_or(MyError::OtherError{info:"create schedule job need tool_name".to_string()})?;
            let schedule = match schedule_cfg.job_type.as_str() {
                "interval" => {
                    let secs = schedule_cfg
                        .secs
                        .ok_or(MyError::OtherError{info: "interval need secs".to_string()})?;
                    ScheduleType::Interval { secs }
                }
                "daily" => {
                    let hour = schedule_cfg
                        .hour
                        .ok_or(MyError::OtherError{info: "daily need hour".to_string()})?;
                    let minute = schedule_cfg.minute.unwrap_or(0);
                    ScheduleType::Daily { hour, minute }
                }
                other => return Err(MyError::OtherError{info: format!("schedule type only support 'interval' and 'daily', not {other}")}),
            };

            let (tool_name, tool_id) = match tool_name.split_once("__") {
                Some((t_name, t_id)) => (t_name.to_string(), t_id.to_string()), // 获取工具 id，`工具名__后缀`
                None => match PARAS.tools.get_tool_id_by_name(&tool_name) {
                    Some(t_id) => (tool_name, t_id),
                    None => return Err(MyError::OtherError{info: format!("schedule can not find tool '{tool_name}', not exist")}), // 没有后缀，仅含有工具名，则通过工具名获取 id
                },
            };

            let job = ScheduledJob {
                id: String::new(), // 空字符串，会生成一个 uuid
                name,
                schedule,
                tool_name,
                tool_id,
                tool_args: params.tool_args.and_then(|v| to_string(&v).ok()).unwrap_or_default(),
                enabled: true,
                next_run: None,
            };

            let (reply_tx, reply_rx) = oneshot::channel();
            cmd_tx
                .send(SchedulerCmd::AddJob { job, reply: reply_tx })
                .await
                .unwrap();
            let job_id = reply_rx.await.unwrap()?;

            event!(Level::INFO, "create schedule job successfull, ID: {}", job_id);
            Ok((format!("create schedule job successfull, ID: {job_id}"), None))
        }
        "delete" => {
            let id = params
                .job_id
                .ok_or(MyError::OtherError{info: "delete schedule job need job_id".to_string()})?;
            let (reply_tx, reply_rx) = oneshot::channel();
            cmd_tx
                .send(SchedulerCmd::RemoveJob { id: id.clone(), reply: reply_tx })
                .await
                .unwrap();
            reply_rx.await.unwrap()?;

            event!(Level::INFO, "delete schedule job successfull, ID: {}", id);
            Ok((format!("delete schedule job successfull, ID: {id}"), None))
        }
        "list" => {
            let (reply_tx, reply_rx) = oneshot::channel();
            cmd_tx
                .send(SchedulerCmd::ListJobs { reply: reply_tx })
                .await
                .unwrap();
            let jobs = reply_rx.await.unwrap();
            if jobs.is_empty() {
                event!(Level::INFO, "No schedule job");
                Ok(("No schedule job".to_string(), None))
            } else {
                let jobs_view: Vec<Value> = jobs.iter().map(|job| {
                    json!({
                        "id": job.id,
                        "name": job.name,
                        "schedule": job.schedule,
                        "tool_name": job.tool_name,
                        "tool_args": job.tool_args,
                        "enabled": job.enabled,
                        "next_run": job.next_run.map(|t| t.with_timezone(&Local)), // 这里将 UTC 时间转为 Local 时间
                    })
                }).collect();

                let json = serde_json::to_string_pretty(&jobs_view).unwrap_or_default();
                event!(Level::INFO, "all schedule jobs:\n{}", json);
                Ok((format!("all schedule jobs:\n{json}"), None))
            }
        }
        other => Err(MyError::OtherError{info: format!("schedule only support 'create', 'delete' and 'list', not {other}")}),
    }
}
