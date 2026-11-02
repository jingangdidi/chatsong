use std::collections::HashMap;
use std::path::Path;

use chrono::Local;
use serde::Deserialize;
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use tokio::sync::oneshot;
use tracing::{event, Level};
use openai_dive::v1::{
    api::Client,
    resources::{
        chat::{
            ChatMessage,
            ChatCompletionTool,
            ChatCompletionToolType,
            ChatCompletionFunction,
            ChatMessageContent,
            ChatMessageContentPart,
            ImageUrlType,
            ChatCompletionParametersBuilder,
            ChatMessageImageContentPart,
            ChatCompletionResponseFormat,
        },
        shared::ReasoningEffort,
    },
};

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::{
            filesystem::{
                read_file::Params as ReadFileParams,
                read_multiple_files::Params as ReadMultipleFilesParams,
            },
            BuiltIn,
            schedule::{
                SimpleComplex,
                ScheduleConfig,
                ScheduledJob,
                SchedulerCmd,
                ScheduleType,
                get_cmd_tx,
            },
            image::{
                image_generation::image_generation,
                edit_image::edit_image,
            },
            deepseek_search::{
                search::deepseek_search,
                fetch::deepseek_fetch,
            },
        },
        get_tool_hashmap,
        get_file_language,
        SkillParams,
        LARGE_FILE_PROMPT,
        SelectedTools,
    },
    api::handlers::chat::get_complex_info,
    skills::SelectedSkills,
    openai::for_tool::{
        call_tool_not_use_stream,
        CallToolResult,
    },
};

#[cfg(feature = "screen-capture")]
use crate::tools::built_in_tools::filesystem::screen_capture::Params as ScreenCaptureParams;

#[derive(Debug, Deserialize)]
struct Params {
    action: String,                   // create, delete, list
    #[serde(default)]
    name: Option<String>,             // 任务名称
    #[serde(default)]
    prompt: Option<String>,           // 任务的prompt
    #[serde(default)]
    remember_context: Option<bool>,   // 是否记住之前执行的上下文，默认false每次独立
    #[serde(default)]
    schedule: Option<ScheduleConfig>, // 定时任务的参数
    #[serde(default)]
    job_id: Option<String>,           // 删除任务要用
}

pub struct ComplexScheduleTask;

impl ComplexScheduleTask {
    pub fn new() -> Self {
        ComplexScheduleTask
    }
}

impl BuiltIn for ComplexScheduleTask {
    /// get tool name
    fn name(&self) -> String {
        "complex_schedule".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Manages complex scheduled AI agent tasks (cron-like jobs). When a task triggers, it executes a prompt-based agent task with access to system-configured tools and skills.

**When creating a task (action='create'), you MUST specify:**
- `name`: a unique task name.
- `prompt`: the instruction/prompt for the agent to execute when the task fires. Describe what the agent should accomplish, including context, goals, and any formatting requirements. The prompt should be self-contained and actionable.
- `schedule` with `job_type` set to 'interval' (runs every N seconds), 'daily' (runs at a fixed time each day), or 'daily_range' (runs every N seconds within a daily time window).
  - For 'interval': provide `secs` (e.g., 60 for 1 minute, 300 for 5 minutes, 3600 for 1 hour).
  - For 'daily': provide `hour` (0–23) and optionally `minute` (0–59, default 0).
  - For 'daily_range': provide `start_hour` (0–23), `end_hour` (0–23, inclusive) and `secs` (the interval in seconds within the window), optionally `minute` (0–59, default 0, alignment anchor). Triggers are aligned to the window start: start, start+secs, ... up to and including the window end. Example: every day 7:00–18:00 run every hour → {job_type='daily_range', start_hour=7, end_hour=18, secs=3600} (fires at 7:00, 8:00, ..., 18:00).

**Optional:**
- `remember_context` (default: false): Set to `true` to keep a persistent conversation context across all executions. Each run will remember what previous runs did (outputs, decisions, state). Set to `false` or omit for independent, stateless runs where each execution starts fresh.

**Example 1:** User says 'Every morning at 7 AM, search for the latest tech news, summarize the top 5 stories, and save the summary to a markdown file'
→ action='create', name='morning_tech_digest', schedule={job_type='daily', hour=7, minute=0}, prompt='Search for the latest technology news. Identify the top 5 most important stories. Write a concise summary for each story (title, 2-3 sentence summary, source URL) and save the result as a markdown file named tech_digest_{date}.md'

**Example 2:** User says 'Every 30 minutes, check the status of the API endpoint and alert me if it is down. Keep track of previous incidents'
→ action='create', name='api_health_check', schedule={job_type='interval', secs=1800}, prompt='Send an HTTP GET request to https://api.example.com/health. If the response status is not 200 or the response takes more than 5 seconds, generate an alert message with the error details. If the API is healthy, log a brief OK status. Always include the timestamp. Reference previous incidents from earlier checks if any.', remember_context=true

**Example 3:** User says 'Every hour, update my learning progress tracker based on what I studied this hour'
→ action='create', name='learning_progress', schedule={job_type='interval', secs=3600}, prompt='Review any new study notes or completed exercises. Update the learning progress tracker with what was learned this hour. Maintain continuity with previous entries.', remember_context=true

**Example 4:** User says 'Every day between 7 and 18 o'clock, check the server metrics every hour'
→ action='create', name='hourly_metrics', schedule={job_type='daily_range', start_hour=7, end_hour=18, secs=3600}, prompt='Check the server metrics and report any anomalies.'

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
                    "description": "A unique name identifying the task. Required when action is 'create'"
                },
                "prompt": {
                    "type": "string",
                    "description": "The instruction/prompt for the AI agent to execute when the task triggers. Should be self-contained, describing the goal, context, and expected output format. Required when action is 'create'"
                },
                "remember_context": {
                    "type": "boolean",
                    "description": "Whether to preserve conversation context across executions. Default false: each run starts with a fresh, independent context. Set to true: the agent remembers outputs and state from previous runs (useful for cumulative/progressive tasks)."
                },
                "schedule": {
                    "type": "object",
                    "description": "Scheduling rules. Required when action is 'create'",
                    "properties": {
                        "job_type": {
                            "type": "string",
                            "enum": ["interval", "daily", "daily_range"],
                            "description": "Schedule type. 'interval' runs every N seconds; 'daily' runs at a fixed time each day; 'daily_range' runs every N seconds within a daily time window"
                        },
                        "secs": {
                            "type": "integer",
                            "description": "Number of seconds between intervals. Required when job_type is 'interval' or 'daily_range'. Example: 60 (1 min), 300 (5 min), 3600 (1 hour)"
                        },
                        "hour": {
                            "type": "integer",
                            "description": "Hour of the day (0–23). Required when job_type is 'daily'"
                        },
                        "minute": {
                            "type": "integer",
                            "description": "Minute of the hour (0–59). Optional, defaults to 0. Used by 'daily' (the fixed trigger time) and 'daily_range' (alignment anchor for the window start/end)"
                        },
                        "start_hour": {
                            "type": "integer",
                            "description": "Hour (0–23) when the daily time window starts. Required when job_type is 'daily_range'"
                        },
                        "end_hour": {
                            "type": "integer",
                            "description": "Hour (0–23, inclusive) when the daily time window ends. Required when job_type is 'daily_range'"
                        }
                    },
                    "required": ["job_type"]
                },
                "job_id": {
                    "type": "string",
                    "description": "The unique ID of the task to delete. Required when action is 'delete'"
                }
            },
            "required": ["action"],
            "type": "object"
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
            Ok(Some(format!("Do you allow calling the complex_schedule tool {} method ?{}", params.action, info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 complex_schedule 工具 {} 方法？{}", params.action, info.unwrap_or_default())))
        }
    }
}

/// 向后台任务调度器发送任务，并等待结果
pub async fn run_complex_schedule_task(uuid: &str, args: &str) -> Result<(String, Option<String>), MyError> {
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
            let schedule = match schedule_cfg.job_type.as_str() {
                "interval" => {
                    let secs = schedule_cfg
                        .secs
                        .ok_or(MyError::OtherError{info: "interval need secs".to_string()})?;
                    ScheduleType::Interval { secs }
                },
                "daily" => {
                    let hour = schedule_cfg
                        .hour
                        .ok_or(MyError::OtherError{info: "daily need hour".to_string()})?;
                    let minute = schedule_cfg.minute.unwrap_or(0);
                    ScheduleType::Daily { hour, minute }
                },
                "daily_range" => {
                    let start_hour = schedule_cfg
                        .start_hour
                        .ok_or(MyError::OtherError{info: "daily_range need start_hour".to_string()})?;
                    let end_hour = schedule_cfg
                        .end_hour
                        .ok_or(MyError::OtherError{info: "daily_range need end_hour".to_string()})?;
                    if start_hour > 23 || end_hour > 23 {
                        return Err(MyError::OtherError{info: format!("daily_range start_hour and end_hour must be 0-23, not {start_hour} and {end_hour}")});
                    }
                    if start_hour > end_hour {
                        return Err(MyError::OtherError{info: format!("daily_range start_hour {start_hour} should not be greater than end_hour {end_hour}")});
                    }
                    let every_secs = schedule_cfg
                        .secs
                        .ok_or(MyError::OtherError{info: "daily_range need secs (interval seconds within the time range)".to_string()})?;
                    if every_secs == 0 {
                        return Err(MyError::OtherError{info: "daily_range secs must be greater than 0".to_string()});
                    }
                    let minute = schedule_cfg.minute.unwrap_or(0);
                    if minute > 59 {
                        return Err(MyError::OtherError{info: format!("daily_range minute must be 0-59, not {minute}")});
                    }
                    ScheduleType::DailyRange { start_hour, end_hour, minute, every_secs }
                },
                other => return Err(MyError::OtherError{info: format!("schedule type only support 'interval', 'daily' and 'daily_range', not {other}")}),
            };

            if let Some(for_complex) = get_complex_info(uuid) {
                let job = ScheduledJob {
                    id: String::new(), // 空字符串，会生成一个 uuid
                    name,
                    schedule,
                    task: SimpleComplex::new_complex(params.prompt.unwrap_or_default(), for_complex, params.remember_context.unwrap_or_default(), Vec::new()),
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
            } else {
                let result = format!("create schedule job failed");
                event!(Level::INFO, "{}", result);
                Ok((result, None))
            }
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
                    match &job.task {
                        SimpleComplex::Simple{tool_name, tool_args, ..} => json!({
                            "id": job.id,
                            "name": job.name,
                            "schedule": job.schedule,
                            "tool_name": tool_name,
                            "tool_args": tool_args,
                            "enabled": job.enabled,
                            "next_run": job.next_run.map(|t| t.with_timezone(&Local)), // 这里将 UTC 时间转为 Local 时间
                        }),
                        SimpleComplex::Complex{prompt, ..} => json!({
                            "id": job.id,
                            "name": job.name,
                            "schedule": job.schedule,
                            "prompt": prompt,
                            "enabled": job.enabled,
                            "next_run": job.next_run.map(|t| t.with_timezone(&Local)), // 这里将 UTC 时间转为 Local 时间
                        }),
                    }
                }).collect();

                let json = serde_json::to_string_pretty(&jobs_view).unwrap_or_default();
                event!(Level::INFO, "all schedule jobs:\n{}", json);
                Ok((format!("all schedule jobs:\n{json}"), None))
            }
        }
        other => Err(MyError::OtherError{info: format!("schedule only support 'create', 'delete' and 'list', not {other}")}),
    }
}

/// 复杂问题
pub async fn run_tools(
    uuid: &str,
    prompt: String,
    model: String,
    api_key: String,
    endpoint: String,
    reasoning: bool,
    effort: ReasoningEffort,
    selected_tools: Option<SelectedTools>,
    selected_skills: Option<SelectedSkills>,
    mut history_messages: Vec<ChatMessage>,
) -> Result<(String, Vec<ChatMessage>), MyError> {
    // 使用api key初始化
    let mut client = Client::new(api_key.clone());
    client.set_base_url(&endpoint); // 从0.7.0开始舍弃了new_with_base
    // 准备参数
    let mut para_builder = ChatCompletionParametersBuilder::default();
    para_builder.model(model.clone());
    para_builder.response_format(ChatCompletionResponseFormat::Text);
    let lowercase_model = model.to_lowercase();
    if reasoning { // 对思维链模型设置effort
        para_builder.reasoning_effort(effort.clone()); // 设置使用思维链，Low（思考的少，简单问答）, Medium（思考适中，多步骤推理）, High（思考更多，复杂逻辑推导）
        // 开启思考，不同模型思考的设置不同
        if lowercase_model.starts_with("deepseek") {
            // deepseek: https://api-docs.deepseek.com/
            para_builder.extra_body(json!({"thinking": {"type": "enabled"}}));
        } else if lowercase_model.starts_with("qwen") {
            // Qwen: https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions#05cfceb898csa
            para_builder.extra_body(json!({"enable_thinking": true}));
        } else if lowercase_model.starts_with("kimi") {
            // kimi: https://platform.kimi.com/docs/api/models-overview
            para_builder.extra_body(json!({"thinking": {"type": "enabled"}}));
        } else if lowercase_model.starts_with("glm") {
            // glm: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction
            para_builder.extra_body(json!({"thinking": {"type": "enabled"}}));
        } else if lowercase_model.starts_with("minimax") {
            // minimax: https://platform.minimaxi.com/docs/api-reference/text-openai-api
            // 目前不支持关闭thinking：https://github.com/MiniMax-AI/MiniMax-M2/issues/68
            //para_builder.extra_body(json!({"reasoning_split": true}));
        }
    } else {
        // 关闭思考，不同模型思考的设置不同
        if lowercase_model.starts_with("deepseek") {
            // deepseek: https://api-docs.deepseek.com/
            para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
        } else if lowercase_model.starts_with("qwen") {
            if endpoint.starts_with("http://") { // local model
                // https://modelscope.cn/models/Qwen/Qwen3.5-397B-A17B
                // https://modelscope.cn/models/Qwen/Qwen3.6-35B-A3B#instruct-or-non-thinking-mode
                para_builder.extra_body(json!({"chat_template_kwargs": {"enable_thinking": false}}));
            } else {
                // Qwen: https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions#05cfceb898csa
                para_builder.extra_body(json!({"enable_thinking": false}));
            }
        } else if lowercase_model.starts_with("kimi") {
            // kimi: https://platform.kimi.com/docs/api/models-overview
            para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
        } else if lowercase_model.starts_with("glm") {
            // glm: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction
            para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
        } else if lowercase_model.starts_with("minimax") {
            // minimax，目前不支持关闭thinking：https://github.com/MiniMax-AI/MiniMax-M2/issues/68
            //para_builder.extra_body(json!({"reasoning_split": false}));
        }
    }
    // get built-in and external tools schame
    let mut tool_schema = PARAS.tools.get_desc_and_schema(&selected_tools)?;
    // get mcp tools schema
    let mcp_schema = PARAS.mcp_servers.get_desc_and_schema(&selected_tools).await?;
    // 如果指定了skills，则加入一个激活指定skill的tool
    if selected_skills.is_some() {
        tool_schema.push(
            ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: ChatCompletionFunction {
                    name: "activate_skill".to_string(),
                    description: Some("Activate an agent skill to load its full instructions. IMPORTANT: You must CALL this tool (not write it as text) to activate a skill. Use this when you see a relevant skill in the available skills list and need its detailed instructions to complete a task.".to_string()),
                    parameters: json!({
                        "properties": {
                            "skill_name": {
                                "type": "string",
                                "description": "The name of the skill to activate.",
                            },
                        },
                        "required": ["skill_name"],
                        "type": "object",
                    }),
                },
            }
        );
        // 如果没有选择`run_command`，则自动添加这个工具
        if tool_schema.iter().any(|t| !t.function.name.starts_with("run_command")) {
            let run_command_schema: Vec<_> = PARAS
                .tools
                .built_in
                .id_map
                .iter()
                .filter(|(_, v)| v.tool.name() == "run_command")
                .map(|(k, v)| (format!("{}__{}", v.tool.name(), k), v.tool.description(), v.tool.schema()))
                .collect();
            tool_schema.push(
                ChatCompletionTool {
                    r#type: ChatCompletionToolType::Function,
                    function: ChatCompletionFunction {
                        name: run_command_schema[0].0.clone(),
                        description: Some(run_command_schema[0].1.clone()),
                        parameters: run_command_schema[0].2.clone(),
                    },
                }
            );
        }
        // 如果没有选择`run_script`，则自动添加这个工具
        if tool_schema.iter().any(|t| !t.function.name.starts_with("run_script")) {
            let run_command_schema: Vec<_> = PARAS
                .tools
                .built_in
                .id_map
                .iter()
                .filter(|(_, v)| v.tool.name() == "run_script")
                .map(|(k, v)| (format!("{}__{}", v.tool.name(), k), v.tool.description(), v.tool.schema()))
                .collect();
            tool_schema.push(
                ChatCompletionTool {
                    r#type: ChatCompletionToolType::Function,
                    function: ChatCompletionFunction {
                        name: run_command_schema[0].0.clone(),
                        description: Some(run_command_schema[0].1.clone()),
                        parameters: run_command_schema[0].2.clone(),
                    },
                }
            );
        }
    }
    // 加上 sub_agent 工具
    if !tool_schema.is_empty() || !mcp_schema.is_empty() {
        let sub_agent_schema: Vec<_> = PARAS
            .tools
            .built_in
            .id_map
            .iter()
            .filter(|(_, v)| v.tool.name() == "sub_agent")
            .map(|(k, v)| (format!("{}__{}", v.tool.name(), k), v.tool.description(), v.tool.schema()))
            .collect();
        tool_schema.push(
            ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: ChatCompletionFunction {
                    name: sub_agent_schema[0].0.clone(),
                    description: Some(sub_agent_schema[0].1.clone()),
                    parameters: sub_agent_schema[0].2.clone(),
                },
            }
        );
    }
    // prepare buider for call tool
    tool_schema.extend(mcp_schema);
    // tool名称加了`_uuid第一部分`保证不重复，但是上下文很长时可能导致模型返回的要调用的模型丢失了`_uuid第一部分`，导致调用模型失败
    // 这里把不含`_uuid第一部分`的tool名称作为key，将对应的完整tool名称作为value，存入HashMap，当`_uuid第一部分`丢失，也能确认具体要调用的模型
    let tool_map = get_tool_hashmap(&tool_schema);
    let para_builder_for_sub_agent = para_builder.clone();
    if !tool_schema.is_empty() {
        para_builder.tools(tool_schema.clone());
    }
    // store all messages as context log, this is temp history, will not add to the user's main history
    history_messages.push(ChatMessage::User{
        content: ChatMessageContent::Text(prompt),
        name: None,
    });
    /*
    let mut history_messages = vec![ChatMessage::User{
        content: ChatMessageContent::Text(prompt),
        name: None,
    }];
    */
    if let Some(sele_skill) = selected_skills {
        if let SelectedSkills::Single(idx) = sele_skill {
            history_messages.push(
                ChatMessage::User{
                    content: ChatMessageContent::Text(PARAS.skills.get_single_available_skill_prompt(idx)),
                    name: None,
                }
            );
        }
    }
    let mut call_tool_count = 0; // call tool count
    let call_tool_limit = call_tool_count+100; // 调用工具的次数限制
    let mut try_count = 0;
    let mut is_first: bool;
    let mut real_name: String; // 要调用的工具的真实名称，含有`_uuid第一部分`后缀
    let final_result: String; // 最终结果

    //'outer: loop {
    loop {
        // send query to LLM
        para_builder.messages(history_messages.clone());
        let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
        let answer = call_tool_not_use_stream(uuid, client.clone(), parameters).await?;
        // if answer is call tool result, continue; else break
        match answer {
            CallToolResult::CallTool((raw_message, call_tool_result)) => { // (ChatMessage, Vec<(tool name, tool args, call tool id, content)>)
                is_first = true;
                for j in call_tool_result {
                    // call tool
                    let mut name_id: Vec<&str> = j.0.split("__").collect();
                    if name_id.len() < 2 && name_id[0] != "activate_skill" {
                        if let Some(tools_vec) = tool_map.get(&j.0) {
                            if tools_vec.len() > 1 {
                                event!(Level::WARN, "{} call tool error, llm only return tool name prefix `{}`, but this prefix have multiple tools: {}", uuid, j.0, tools_vec.join(", "));
                            } else {
                                real_name = tools_vec[0].clone();
                                name_id = real_name.split("__").collect();
                                event!(Level::WARN, "{} call real tool `{}` by model returned `{}`", uuid, real_name, j.0);
                            }
                        } else {
                            event!(Level::WARN, "{} can't find real tool by model returned `{}`", uuid, j.0);
                        }
                    }
                    let (mut result, language, is_image) = match try_call_tool(
                        uuid,
                        &name_id,
                        &j.1,
                        //j.3.clone(),
                        &model,
                        Some(tool_schema.clone()),
                        Some(client.clone()),
                        Some(para_builder_for_sub_agent.clone()),
                        true, // is main agent
                    ).await {
                        Ok(inner_result) => {
                            match inner_result {
                                Ok((result, file_option)) => {
                                    try_count = 0;
                                    if name_id[0].starts_with("load_image") {
                                        (result, get_file_language(file_option), true)
                                    } else {
                                        (result, get_file_language(file_option), false)
                                    }
                                },
                                Err(e) => {
                                    try_count += 1;
                                    if is_first {
                                        if try_count >= 3 {
                                            return Err(e)
                                        } else {
                                            event!(Level::WARN, "{} call tool {} error, try again {}\n{}\nargs: {}", uuid, name_id[0], try_count, e, j.1);
                                            //continue 'outer
                                            (format!("call tool {} error: {}", name_id[0], e), "text".to_string(), false)
                                        }
                                    } else {
                                        event!(Level::WARN, "{} call tool {} error, send this error to LLM\n{}\nargs: {}", uuid, name_id[0], e, j.1);
                                        try_count = 0;
                                        (format!("call tool {} error: {}", name_id[0], e), "text".to_string(), false)
                                    }
                                },
                            }
                        },
                        Err(e) => {
                            event!(Level::WARN, "{} call tool {}, raw args: {}", uuid, name_id[0], j.1);
                            return Err(e)
                        },
                    };
                    call_tool_count += 1;

                    // 1. print call tool result
                    let test_result = if is_image {
                        "load image successful".to_string()
                    } else if result.contains("```") {
                        result.clone()
                    } else {
                        format!("```{}\n{}\n```", language, result)
                    };
                    let _test_result = if let Some(content) = &j.3 {
                        if content.is_empty() {
                            format!("## 📌 {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", call_tool_count, name_id[0], &j.1, test_result)
                        } else {
                            format!("## 📌 {} call tool\n\n---\n\n{}\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", call_tool_count, content, name_id[0], &j.1, test_result)
                        }
                    } else {
                        format!("## 📌 {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", call_tool_count, name_id[0], &j.1, test_result)
                    };

                    // 2. add result to main message history
                    let mut call_tool_vec = Vec::new();
                    if is_first {
                        call_tool_vec.push(raw_message.clone());
                    }
                    call_tool_vec.push(ChatMessage::Tool{content: ChatMessageContent::Text(result.clone()), tool_call_id: j.2.clone()});

                    // 绘图
                    if name_id[0] == "image_generation" || name_id[0] == "edit_image" {
                        result = if name_id[0] == "image_generation" {
                            format!("create image successfull: {}", result)
                        } else if name_id[0] == "edit_image" {
                            format!("edit image successfull: {}", result)
                        } else {
                            result
                        };
                    }

                    // add each tool call result to current message history
                    if is_first {
                        history_messages.push(raw_message.clone());
                        is_first = false;
                    }
                    if is_image {
                        history_messages.push(ChatMessage::Tool{
                            content: ChatMessageContent::ContentPart(vec![ChatMessageContentPart::Image(
                                ChatMessageImageContentPart {
                                    r#type: "image_url".to_string(),
                                    image_url: ImageUrlType {
                                        url: result, // Either a URL of the image or the base64 encoded image data
                                        detail: None,
                                    },
                                },
                            )]),
                            tool_call_id: j.2,
                        });
                    } else {
                        history_messages.push(ChatMessage::Tool{content: ChatMessageContent::Text(result), tool_call_id: j.2});
                    }
                }
            },
            CallToolResult::Text(test_result) => { // normal text result, not call tool
                final_result = test_result;
                break
            },
        }
        if call_tool_count > call_tool_limit {
            final_result = format!("already call tool {} times, stop this loop", call_tool_count);
            event!(Level::WARN, "{} already call tool {} times, stop this loop", uuid, call_tool_count);
            break
        }
    }
    Ok((final_result, history_messages))
}

/// sub-agent
/// 不能再开启 sub-agent，避免无限递归调用，会自动把 sub_agent 工具过滤掉
/// 也不能更新 goal 的状态，会自动把 update_goal_status 工具过滤掉
async fn sub_agent(
    uuid: String,
    prompt: String,
    client: Client,
    mut para_builder: ChatCompletionParametersBuilder,
    model: &str,
    tool_map: HashMap<String, Vec<String>>,
) -> Result<String, MyError> {
    let mut history_messages: Vec<ChatMessage> = vec![ChatMessage::User{
        content: ChatMessageContent::Text(prompt),
        name: None,
    }];

    let mut call_tool_count = 0;
    let call_tool_limit = call_tool_count+100; // 调用工具的次数限制
    let mut try_count = 0;
    let mut is_first: bool;
    let mut real_name: String; // 要调用的工具的真实名称，含有`_uuid第一部分`后缀

    //'outer: loop {
    loop {
        // send query to LLM
        para_builder.messages(history_messages.clone());
        let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
        let answer = call_tool_not_use_stream(&uuid, client.clone(), parameters).await?;
        // if answer is call tool result, continue; else break
        match answer {
            CallToolResult::CallTool((raw_message, call_tool_result)) => { // (ChatMessage, Vec<(tool name, tool args, call tool id, content)>)
                is_first = true;
                for j in call_tool_result {
                    // call tool
                    let mut name_id: Vec<&str> = j.0.split("__").collect();
                    if name_id.len() < 2 && name_id[0] != "activate_skill" {
                        if let Some(tools_vec) = tool_map.get(&j.0) {
                            if tools_vec.len() > 1 {
                                event!(Level::WARN, "{} sub-agent call tool error, llm only return tool name prefix `{}`, but this prefix have multiple tools: {}", uuid, j.0, tools_vec.join(", "));
                            } else {
                                real_name = tools_vec[0].clone();
                                name_id = real_name.split("__").collect();
                                event!(Level::WARN, "{} sub-agent call real tool `{}` by model returned `{}`", uuid, real_name, j.0);
                            }
                        } else {
                            event!(Level::WARN, "{} sub-agent can't find real tool by model returned `{}`", uuid, j.0);
                        }
                    }
                    let (mut result, _, is_image) = match Box::pin(try_call_tool(
                        &uuid,
                        &name_id,
                        &j.1,
                        //j.3.clone(),
                        model,
                        None,
                        None,
                        None,
                        false, // is main agent
                    )).await {
                        Ok(inner_result) => {
                            match inner_result {
                                Ok((result, file_option)) => {
                                    try_count = 0;
                                    if name_id[0].starts_with("load_image") {
                                        (result, get_file_language(file_option), true)
                                    } else {
                                        (result, get_file_language(file_option), false)
                                    }
                                },
                                Err(e) => {
                                    try_count += 1;
                                    if is_first {
                                        if try_count >= 3 {
                                            return Err(e)
                                        } else {
                                            event!(Level::WARN, "{} sub-agent call tool {} error, try again {}\n{}\nargs: {}", uuid, name_id[0], try_count, e, j.1);
                                            //continue 'outer
                                            (format!("sub-agent call tool {} error: {}", name_id[0], e), "text".to_string(), false)
                                        }
                                    } else {
                                        event!(Level::WARN, "{} sub-agent call tool {} error, send this error to LLM\n{}\nargs: {}", uuid, name_id[0], e, j.1);
                                        try_count = 0;
                                        (format!("sub-agent call tool {} error: {}", name_id[0], e), "text".to_string(), false)
                                    }
                                },
                            }
                        },
                        Err(e) => {
                            event!(Level::WARN, "{} sub-agent call tool {}, raw args: {}", uuid, name_id[0], j.1);
                            return Err(e)
                        },
                    };
                    call_tool_count += 1;

                    // 绘图
                    if name_id[0] == "image_generation" || name_id[0] == "edit_image" {
                        result = if name_id[0] == "image_generation" {
                            format!("create image successfull: {}", result)
                        } else if name_id[0] == "edit_image" {
                            format!("edit image successfull: {}", result)
                        } else {
                            result
                        };
                    }

                    // add each tool call result to current message history
                    if is_first {
                        history_messages.push(raw_message.clone());
                        is_first = false;
                    }
                    if is_image {
                        history_messages.push(ChatMessage::Tool{
                            content: ChatMessageContent::ContentPart(vec![ChatMessageContentPart::Image(
                                ChatMessageImageContentPart {
                                    r#type: "image_url".to_string(),
                                    image_url: ImageUrlType {
                                        url: result, // Either a URL of the image or the base64 encoded image data
                                        detail: None,
                                    },
                                },
                            )]),
                            tool_call_id: j.2,
                        });
                    } else {
                        history_messages.push(ChatMessage::Tool{content: ChatMessageContent::Text(result), tool_call_id: j.2});
                    }
                }
            },
            CallToolResult::Text(test_result) => { // normal text result, not call tool
                // return result to main agent
                return Ok(test_result)
            },
        }
        if call_tool_count > call_tool_limit {
            event!(Level::WARN, "{} sub-agent already call tool {} times, stop this loop", uuid, call_tool_count);
            break
        }
    }
    Err(MyError::OtherError{info: "sub-agent error".to_string()})
}

/// 调用 sub-agent
async fn run_sub_agent(
    uuid: &str,
    name_id: &[&str],
    paras: &str,
    model: &str,
    tool_schema: Option<Vec<ChatCompletionTool>>,
    mut para_builder: Option<ChatCompletionParametersBuilder>,
    client: Option<Client>,
    indirect: bool, // 是否主 agent 间接调用，比如主 agent 调用读取大文件，改为通过 sub-agent 间接调用
) -> Result<Result<(String, Option<String>), MyError>, MyError> {
    match PARAS.tools.run(name_id[1], paras) {
        Ok((prompt_tools, _)) => {
            let parts: Vec<&str> = prompt_tools.split("---srx---").collect(); // [prompt, tool1, tool2, ...]
            let tool_map = if parts.len() > 1 {
                let mut sub_agent_tools: Vec<ChatCompletionTool> = Vec::with_capacity(parts.len()-1);
                for t in &parts[1..] {
                    let tmp_name = match t.split_once("__") {
                        Some((t_name, _)) => format!("{}__", t_name),
                        None => format!("{}__", t),
                    };
                    if tmp_name == "sub_agent__" || tmp_name == "update_goal_status__" {
                        // 禁止 sub-agent 再开启新的 sub-agent
                        // 禁止 sub-agent 更新 goal 的状态
                        event!(Level::WARN, "{} skip tool `{}` in sub-agent", uuid, tmp_name);
                        continue
                    }
                    if let Some(ref tools) = tool_schema {
                        for t in tools {
                            if t.function.name.starts_with(&tmp_name) {
                                sub_agent_tools.push(t.clone());
                                break
                            }
                        }
                    }
                }
                let tool_map = get_tool_hashmap(&sub_agent_tools);
                if let Some(ref mut p_b) = para_builder {
                    p_b.tools(sub_agent_tools);
                }
                tool_map
            } else {
                HashMap::new()
            };
            match Box::pin(sub_agent(
                uuid.to_string(),
                parts[0].to_string(),
                client.unwrap(),
                para_builder.unwrap(),
                model,
                tool_map,
            )).await {
                Ok(sub_agent_result) => Ok(Ok((if indirect { format!("Complete by calling the sub_agent:\n{}", sub_agent_result) } else { sub_agent_result }, None))),
                Err(e) => Ok(Err(e)),
            }
        },
        Err(e) => Ok(Err(e)),
    }
}

/// try run tool, if error not from call tool, return Err(), else return Ok(Ok()) or Ok(Err())
async fn try_call_tool(
    uuid: &str,
    name_id: &[&str],
    paras: &str,
    //info: Option<String>,
    model: &str,
    tool_schema: Option<Vec<ChatCompletionTool>>,
    client: Option<Client>,
    para_builder: Option<ChatCompletionParametersBuilder>,
    is_main_agent: bool,
) -> Result<Result<(String, Option<String>), MyError>, MyError> {
    if name_id[0] == "activate_skill" {
        //let params: SkillParams = serde_json::from_str(paras).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: SkillParams = parse_tool_args(paras, ArgFixSpec{ array_fields: None, object_fields: None })?;
        Ok(Ok((PARAS.skills.get_skill_full_content(&params.skill_name)?, None)))
    } else if name_id.len() < 2 {
        return Ok(Err(MyError::ToolNotExistError{id: name_id[0].to_string(), info: "run_tools".to_string()}))
    } else if PARAS.tools.contain_tool_id(name_id[1]) {
        // 调用工具
        match name_id[0] {
            "image_generation" => match PARAS.tools.run(name_id[1], paras) {
                Ok((image_prompt, _)) => {
                    match image_generation(uuid, image_prompt/*, "gpt-image-2"*/).await {
                        Ok(image_path) => Ok(Ok((image_path, None))),
                        Err(e) => Ok(Err(e)),
                    }
                },
                Err(e) => Ok(Err(e)),
            },
            "edit_image" => match PARAS.tools.run(name_id[1], paras) {
                Ok((facial_prompt_image, _)) => {
                    let parts: Vec<&str> = facial_prompt_image.splitn(3, "---srx---").collect(); // [是否强调面部特征, prompt, 图片路径]
                    match edit_image(uuid, parts[0] == "true", parts[2].split("---srx---").map(|img| img.to_string()).collect::<Vec<String>>(), parts[1]/*, "gpt-image-2"*/).await {
                        Ok(image_path) => Ok(Ok((image_path, None))),
                        Err(e) => Ok(Err(e)),
                    }
                },
                Err(e) => Ok(Err(e)),
            },
            "sub_agent" => run_sub_agent(
                uuid,
                name_id,
                paras,
                model,
                tool_schema,
                para_builder,
                client,
                false,
            ).await,
            #[cfg(feature = "screen-capture")]
            "screen_capture" => {
                let params: ScreenCaptureParams = match parse_tool_args::<ScreenCaptureParams>(paras, ArgFixSpec{ array_fields: None, object_fields: None }) {
                    Ok(mut p) => {
                        p.path = format!("{}/screen_capture.png", PARAS.outpath); // 将输出路径固定到当前uuid输出路径
                        p
                    },
                    Err(e) => return Ok(Err(e)),
                };
                let new_run_para = match serde_json::to_string(&params) {
                    Ok(n) => n,
                    Err(e) => return Ok(Err(MyError::JsonToStringError{error: e.into()})),
                };
                Ok(PARAS.tools.run(name_id[1], &new_run_para))
            },
            "read_file" if is_main_agent => { // 读取大文件时转为调用 sub-agent
                let read_file_para: ReadFileParams = parse_tool_args(paras, ArgFixSpec{ array_fields: None, object_fields: None })?;
                // 小文件（<500k）和 md 文件可以直接读取，大文件则通过 sub_agent 读取
                let file_path = Path::new(&read_file_para.file_path);
                let ext = if let Some(ext) = file_path.extension() {
                    Some(ext.to_ascii_lowercase().to_str().unwrap().to_string())
                } else {
                    None
                };
                let metadata = file_path.metadata()?;
                if metadata.len() < 500000 || if let Some(e) = ext { e == "md" } else { false } { // 直接读取
                    Ok(PARAS.tools.run(name_id[1], paras))
                } else { // 通过 sub_agent 读取
                    event!(Level::INFO, "{} main agent read_file by sub-agent", uuid);
                    let sub_agent_id = PARAS.tools.get_tool_id_by_name("sub_agent").unwrap();
                    run_sub_agent(
                        uuid,
                        &["sub_agent", &sub_agent_id],
                        &format!("{{\"prompt\": \"{}read file: {}\", \"tools\": [\"{}\"]}}", LARGE_FILE_PROMPT.replace("\n", "\\n"), read_file_para.file_path.replace("\\", "\\\\"), name_id[0]), // 这里自己构建 json 字符串，换行符、双引号、文件路径的`\`都需要转义
                        model,
                        tool_schema,
                        para_builder,
                        client,
                        true,
                    ).await
                }
            },
            "read_multiple_files" if is_main_agent => { // 读取多个文件时转为调用 sub-agent
                let read_multiple_files_para: ReadMultipleFilesParams = parse_tool_args(paras, ArgFixSpec{ array_fields: Some(vec!["paths".to_string()]), object_fields: None })?;
                let sub_agent_id = PARAS.tools.get_tool_id_by_name("sub_agent").unwrap();
                event!(Level::INFO, "{} main agent read_multiple_files by sub-agent", uuid);
                run_sub_agent(
                    uuid,
                    &["sub_agent", &sub_agent_id],
                    &format!("{{\"prompt\": \"{}read files:\\n{}\", \"tools\": [\"{}\"]}}", LARGE_FILE_PROMPT.replace("\n", "\\n"), read_multiple_files_para.paths.join("\\n").replace("\\", "\\\\"), name_id[0]), // 这里自己构建 json 字符串，换行符、双引号、文件路径的`\`都需要转义
                    model,
                    tool_schema,
                    para_builder,
                    client,
                    true,
                ).await
            },
            "web_search" => match PARAS.tools.run(name_id[1], paras) {
                Ok((query_count, _)) => {
                    let parts: Vec<&str> = query_count.splitn(2, "---srx---").collect(); // [搜索问题, 返回数量]
                    let count = match parts[1].parse::<u8>() {
                        Ok(c) => c,
                        Err(_) => 10,
                    };
                    match deepseek_search(parts[0], count).await {
                        Ok(search_result) => Ok(Ok((search_result, None))),
                        Err(e) => Ok(Err(e)),
                    }
                },
                Err(e) => Ok(Err(e)),
            },
            "fetch_url" => match PARAS.tools.run(name_id[1], paras) {
                Ok((url, _)) => {
                    match deepseek_fetch(&url).await {
                        Ok(fetch_result) => Ok(Ok((fetch_result, None))),
                        Err(e) => Ok(Err(e)),
                    }
                },
                Err(e) => Ok(Err(e)),
            },
            _ => Ok(PARAS.tools.run(name_id[1], paras)),
        }
    } else if PARAS.mcp_servers.contain_server_id(name_id[1]) {
        Ok(PARAS.mcp_servers.run(&name_id, paras).await)
    } else {
        return Ok(Err(MyError::ToolNotExistError{id: name_id[1].to_string(), info: "run_tools".to_string()}))
    }
}
