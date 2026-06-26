use std::collections::HashMap;
//use std::io::Read;
use std::path::Path;
//use std::process::{Command, Stdio};

use chrono::Local;
use openai_dive::v1::{
    api::Client,
    resources::chat::{
        ChatCompletionFunction,
        ChatCompletionParametersBuilder,
        ChatCompletionTool,
        ChatCompletionToolType,
        ChatMessage,
        ChatMessageContent,
        ChatMessageContentPart,
        ChatMessageImageContentPart,
        ImageUrlType,
    },
};
//use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{json, Value};
use tokio::{
    sync::mpsc::Sender,
    time::{
        sleep,
        Duration,
    },
    //process::Command,
};
use tracing::{event, Level};

use crate::{
    info::{
        insert_message,
        get_messages,
        get_messages_num,
        DataType,
        update_approval,
        approved,
        get_tool_calling_count,
    },
    openai::{
        for_tool::{
            call_tool_not_use_stream,
            CallToolResult,
        },
        for_chat::not_use_stream,
        for_image::image_to_base64, // 图片转base64，返回base64编码的字符串
    },
    parse_paras::PARAS,
    error::MyError,
    api::handlers::{
        chat::{
            MainData,
            MetaData,
        },
        new_instruction::{
            get_new_instruction,
            reset_new_instruction,
        },
        goal::reset_goal,
    },
    skills::SelectedSkills,
    memory::get_relevant_memory,
};

pub mod built_in_tools;
pub mod external_tools;

use built_in_tools::{
    BuiltInTools,
    Group,
    filesystem::{
        edit_file::Params,
        read_file::Params as ReadFileParams,
        read_multiple_files::Params as ReadMultipleFilesParams,
        image_generation::image_generation,
        edit_image::edit_image,
    },
    hacker_news::hacker_news_summaries,
    schedule::run_schedule_task,
    goal::{
        Goal,
        GoalStatus,
    },
};
use external_tools::ExternalTools;

/// html pulldown option selected tools
pub enum SelectedTools {
    All,               // all tools
    AllBuiltIn,        // all built-in tools
    AllExternal,       // all external tools
    Group(String),     // built-in group, start with `built_in_group_`, select all tools of one group
    Single(String),    // single built-in or external tool id
    AllMcp,            // all mcp tools
    McpServer(String), // single mcp server id, start with `mcp_server_`, select all tools of one server
    McpTool(String),   // single mcp tool `name__id`, select by tool name and server id
}

/// trait for built-in tools & external tools
pub trait MyTools {
    /// run tool
    fn run(&self, id: &str, args: &str) -> Result<(String, Option<String>), MyError>;

    /// get all selected tools name (name format: `name__id`, max name length is 26), description and schema
    fn get_desc_and_schema(&self, selected_tools: Vec<String>) -> Vec<(String, String, Value)>;

    /// select all tools, return uuid vector
    fn select_all_tools(&self) -> Vec<String>;

    /// get approval message
    fn get_approval(&self, id: &str, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError>;
}

/// all tools: built-in tools & external tools
pub struct Tools {
    built_in: BuiltInTools,
    external: ExternalTools,
    pub html: String, // html pulldown options
}

impl Tools {
    /// create new Tools, ExternalTools from config file
    pub fn new(external: ExternalTools, english: bool) -> Result<Self, MyError> {
        let built_in = BuiltInTools::new()?;
        let mut options: Vec<String> = Vec::with_capacity(3 + built_in.id_map.len() + built_in.groups.len() * 2 + 2 + external.id_map.len());
        // built-in pulldown options
        let mut groups: Vec<(Group, String)> = built_in.groups.iter().map(|g| (g.clone(), g.to_string())).collect();
        groups.sort_by(|a, b| a.1.cmp(&b.1)); // sort by group name
        if english {
            options.push("<option value='not_select_any_tools' selected>⚪ not using any tools</option>".to_string());
            options.push("                <option value='select_all_tools'>🔴 select all tools</option>".to_string());
            options.push("                <optgroup label='built-in tools'>".to_string());
            options.push("                    <option value='select_all_built_in'>🟢 select all built-in tools</option>".to_string());
        } else {
            options.push("<option value='not_select_any_tools' selected>⚪ 不使用任何工具</option>".to_string());
            options.push("                <option value='select_all_tools'>🔴 选择所有工具</option>".to_string());
            options.push("                <optgroup label='内置工具'>".to_string());
            options.push("                    <option value='select_all_built_in'>🟢 选择所有内置工具</option>".to_string());
        }
        for g in groups {
            // 排除 sub-agent 和 update_goal_status，不在页面下拉选项中显示
            if g.0 == Group::SubAgent || g.0 == Group::UpdateGoalStatus {
                continue
            }
            let mut tools: Vec<(String, String, String)> = built_in.id_map.iter().filter(|(_, v)| v.group == g.0).map(|(k, v)| (k.clone(), v.tool.name(), v.tool.description())).collect();
            tools.sort_by(|a, b| a.1.cmp(&b.1)); // sort by tool name
            options.push(format!("                    <option disabled>--{}--</option>", g.1));
            if english {
                options.push(format!("                    <option value='built_in_group_{}'>🟢 select all {}</option>", g.1, g.1));
            } else {
                options.push(format!("                    <option value='built_in_group_{}'>🟢 选择所有{}</option>", g.1, g.1));
            }
            for t in tools {
                options.push(format!("                    <option value='{}' title=\"{}\">{}</option>", t.0, t.2.replace("\"", "&quot;"), t.1));
            }
        }
        options.push("                </optgroup>".to_string());
        // external pulldown options
        if !external.id_map.is_empty() {
            if english {
                options.push("                <optgroup label='external tools'>".to_string());
                options.push("                    <option value='select_all_external'>🟣 select all external tools</option>".to_string());
            } else {
                options.push("                <optgroup label='外部工具'>".to_string());
                options.push("                    <option value='select_all_external'>🟣 选择所有外部工具</option>".to_string());
            }
            for (k, v) in &external.id_map {
                options.push(format!("                    <option value='{}' title=\"{}\">{}</option>", k, v.description.replace("\"", "&quot;"), v.name));
            }
            options.push("                </optgroup>".to_string());
        }
        // return
        Ok(Self {built_in, external, html: options.join("\n")})
    }

    /// run tool
    pub fn run(&self, id: &str, args: &str) -> Result<(String, Option<String>), MyError> {
        if self.built_in.id_map.contains_key(id) {
            self.built_in.run(id, args)
        } else if self.external.id_map.contains_key(id) {
            self.external.run(id, args)
        } else {
            Err(MyError::ToolNotExistError{id: id.to_string(), info: "Tools::run()".to_string()})
        }
    }

    /// get all selected tools for LLM api function calling
    /// tool name format: `name__id`, max name length is 26
    pub fn get_desc_and_schema(&self, selected_tools: &Option<SelectedTools>) -> Result<Vec<ChatCompletionTool>, MyError> {
        // get selected tools
        let (selected_builtin_tools, selected_external_tools) = match selected_tools {
            Some(s) => match s {
                SelectedTools::All => (self.built_in.select_all_tools(), self.external.select_all_tools()), // all tools
                SelectedTools::AllBuiltIn => (self.built_in.select_all_tools(), Vec::new()), // all built-in tools
                SelectedTools::AllExternal => (Vec::new(), self.external.select_all_tools()), // all external tools
                SelectedTools::Group(group) => (self.built_in.select_tools_by_group(&group)?, Vec::new()), // built-in group
                SelectedTools::Single(id) => { // single built-in or external tool id
                    if self.built_in.id_map.contains_key(id) {
                        (vec![id.to_string()], Vec::new())
                    } else if self.external.id_map.contains_key(id) {
                        (Vec::new(), vec![id.to_string()])
                    } else {
                        return Err(MyError::ToolNotExistError{id: id.to_string(), info: "Tools::select_tools()".to_string()})
                    }
                },
                SelectedTools::AllMcp => (Vec::new(), Vec::new()), // all mcp tools
                SelectedTools::McpServer(_) => (Vec::new(), Vec::new()), // single mcp server id, start with `mcp_server_`, select all tools of one server
                SelectedTools::McpTool(_) => (Vec::new(), Vec::new()), // single mcp tool `name__id`, select by tool name and server id
            },
            None => (Vec::new(), Vec::new()), // not select any tool
        };
        // get name, description and schema
        let mut tools: Vec<ChatCompletionTool> = Vec::new();
        let desc_and_schema_builtin = self.built_in.get_desc_and_schema(selected_builtin_tools);
        let desc_and_schema_external = self.external.get_desc_and_schema(selected_external_tools);
        for tool in desc_and_schema_builtin.into_iter().chain(desc_and_schema_external) {
            tools.push(
                ChatCompletionTool {
                    r#type: ChatCompletionToolType::Function,
                    function: ChatCompletionFunction {
                        name: tool.0,
                        description: Some(tool.1),
                        parameters: tool.2,
                    },
                }
            );
        }
        Ok(tools)
    }

    /// check contain tool id
    pub fn contain_tool_id(&self, id: &str) -> bool {
        if self.built_in.id_map.contains_key(id) {
            true
        } else if self.external.id_map.contains_key(id) {
            true
        } else {
            false
        }
    }

    /// get approval message
    pub fn get_approval(&self, id: &str, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        if self.built_in.id_map.contains_key(id) {
            self.built_in.get_approval(id, args, info, is_en)
        } else if self.external.id_map.contains_key(id) {
            self.external.get_approval(id, args, info, is_en)
        } else {
            Err(MyError::ToolNotExistError{id: id.to_string(), info: "Tools::get_approval()".to_string()})
        }
    }

    /// get tool id by name
    pub fn get_tool_id_by_name(&self, name: &str) -> Option<String> {
        // 从 built-in 工具中查找
        for (k, v) in &self.built_in.id_map {
            if v.tool.name() == name {
                return Some(k.clone())
            }
        }
        // 从 external 工具中查找
        for (k, v) in &self.external.id_map {
            if v.name == name {
                return Some(k.clone())
            }
        }
        None
    }
}

/// 以这些词为起始，则粗略认为是查看或删除任务，不需要把 tool 发给模型
static SCHEDULE_LIST_REMOVE: &[&str; 21] = &[
    "查看", "列出", "显示", "展示", "查询", "当前",
    "移除", "删除", "取消", "删掉", "终止", "停止",
    "list", "show", "display",
    "remove", "delete", "cancel", "terminate", "stop", "kill",
];

/// chat with tools: built-in + external tools
/// This just a simple loop, if not call tool, break the loop, return result
/// +-------------------------------------------------------------+
/// |      user query                                             |
/// |          |                                                  |
/// | +--------+------------+                                     |
/// | |        |            |                                     |
/// | |        V            |                                     |
/// | | context messages <--+--------------------+                |
/// | |        |            |                    |                |
/// | |        V            |                    | add            |
/// | |       LLM           |                    | each           |
/// | |        |            |                    | call           |
/// | |        V            |                    | tool           |
/// | |    call tool ?      |                    | result         |
/// | |        |            |                    | to             |
/// | |    +-------+        |                    | context        |
/// | |    |       |        |                    |                |
/// | |    V       V        |    +------------------------------+ |
/// | |   No      Yes ------+--> | run MCP tools, return result | |
/// | |    |                |    +------------------------------+ |
/// | +----+----------------+                                     |
/// |      |                                                      |
/// |      V                                                      |
/// |   result                                                    |
/// +-------------------------------------------------------------+
/// parameters:
/// +--------------------------------------------------------------------+
/// | selected_tools: html pulldown selected tools                       |
/// | uuid:           current chat uuid                                  |
/// | sender:         channel for send final result to user page         |
/// | client:         OpenAI api Client for send request                 |
/// | para_builder:   update history messages before send request to LLM |
/// | model:          model name                                         |
/// +--------------------------------------------------------------------+
/// function calling:
/// https://api-docs.deepseek.com/zh-cn/guides/function_calling
/// https://docs.bigmodel.cn/cn/guide/capabilities/function-calling
/// https://platform.moonshot.cn/docs/api/tool-use
pub async fn run_tools(
    selected_tools: Option<SelectedTools>,
    selected_skills: Option<SelectedSkills>,
    uuid: String,
    sender: Sender<Vec<u8>>,
    client: Client,
    mut para_builder: ChatCompletionParametersBuilder,
    model: &str,
    raw_goal: Option<String>,
    is_local: bool,
) -> Result<(), MyError> {
    // 初始化 Goal
    let (mut my_goal, goal_mode) = if let Some(g) = raw_goal {
        (Some(Goal::new_goal(g)), true)
    } else {
        (None, false)
    };
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
    {
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
    // goal 模式加上 update_goal_status
    if goal_mode {
        let update_goal_status: Vec<_> = PARAS
            .tools
            .built_in
            .id_map
            .iter()
            .filter(|(_, v)| v.tool.name() == "update_goal_status")
            .map(|(k, v)| (format!("{}__{}", v.tool.name(), k), v.tool.description(), v.tool.schema()))
            .collect();
        tool_schema.push(
            ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: ChatCompletionFunction {
                    name: update_goal_status[0].0.clone(),
                    description: Some(update_goal_status[0].1.clone()),
                    parameters: update_goal_status[0].2.clone(),
                },
            }
        );
    }
    // prepare buider for call tool
    tool_schema.extend(mcp_schema);
    // 如果只选择了 schedule_task 这一个 tool，则从问题中提取是否有指定要调用的其他 tool，如果有，则获取这些 tool 的 schema，作为单独一条信息发送给模型，如果没有指定，则把所有其他 tool 单独作为一条信息发送给模型
    let other_tools = if tool_schema.len() == 1 && tool_schema[0].function.name.starts_with("schedule_task__") {
        let mut all_tools = PARAS.tools.get_desc_and_schema(&Some(SelectedTools::All))?;
        all_tools.retain(|t| !t.function.name.starts_with("schedule_task__")); // 排除 schedule_task
        Some(all_tools)
    } else {
        None
    };
    // tool名称加了`_uuid第一部分`保证不重复，但是上下文很长时可能导致模型返回的要调用的模型丢失了`_uuid第一部分`，导致调用模型失败
    // 这里把不含`_uuid第一部分`的tool名称作为key，将对应的完整tool名称作为value，存入HashMap，当`_uuid第一部分`丢失，也能确认具体要调用的模型
    let tool_map = get_tool_hashmap(&tool_schema);
    let para_builder_for_sub_agent = para_builder.clone();
    para_builder.tools(tool_schema.clone());
    let mut history_messages: Vec<ChatMessage> = get_messages(&uuid); // store all messages as context log, this is temp history, will not add to the user's main history
    // 如果只有用户提问，则提取用户的问题，获取记忆并注入
    if history_messages.iter().all(|m| matches!(m, ChatMessage::User { .. })) {
        let mut user_msg = Vec::new();
        for m in &history_messages {
            if let &ChatMessage::User{content: ct, ..} = &m {
                if let ChatMessageContent::Text(c) = ct {
                    user_msg.push(c.clone());
                }
            }
        }
        if !user_msg.is_empty() {
            let query = user_msg.join("\n");
            let memory = {
                match get_relevant_memory(&uuid, &query, 10, is_local) {
                    Some(memory) => Some(memory),
                    None => if is_local {
                        // 再尝试从 memory_old.json 中获取
                        get_relevant_memory("local", &query, 10, false)
                    } else {
                        None
                    }
                }
            };
            if let Some(m) = memory {
                event!(Level::INFO, "{} insert memory prompt: {}\n", uuid, m);
                history_messages.push(
                    ChatMessage::User{
                        content: ChatMessageContent::Text(m),
                        name: None,
                    }
                );
            }
        }
    }
    // 插入 schedule_task 要调用的其他 tool
    let mut schedule_list_remove = false; // 是否仅查询或删除定时任务，此时不需要插入其他 tool，节省 token
    if let Some(other_all_tools) = other_tools {
        // 从 history_messages 最后连续的提问中获取指定要用的 tool
        let mut final_tools = Vec::new();
        for m in history_messages.iter().rev() {
            if let &ChatMessage::User{content: ct, ..} = &m {
                if let ChatMessageContent::Text(c) = ct {
                    if !schedule_list_remove {
                        schedule_list_remove = SCHEDULE_LIST_REMOVE.iter().any(|x| c.starts_with(x));
                    }
                    for t in &other_all_tools {
                        if c.contains(t.function.name.split_once("__").unwrap().0) && !final_tools.contains(t) {
                            final_tools.push(t.clone())
                        }
                    }
                }
            } else {
                break
            }
        }
        // 将 schedule_task 要用的工具单独一条信息发送给模型
        if !schedule_list_remove {
            event!(Level::INFO, "{} add {} tools for schedule_task: {}",
                uuid,
                if final_tools.is_empty() {
                    other_all_tools.len()
                } else {
                    final_tools.len()
                },
                if final_tools.is_empty() {
                    other_all_tools.iter().map(|t| t.function.name.clone()).collect::<Vec<String>>().join(", ")
                } else {
                    final_tools.iter().map(|t| t.function.name.clone()).collect::<Vec<String>>().join(", ")
                },
            );
            history_messages.push(
                ChatMessage::User{
                    content: ChatMessageContent::Text(format!(
                        "Below are all the available tools for schedule_task:\n\n{:?}",
                        if final_tools.is_empty() {
                            other_all_tools
                        } else {
                            final_tools
                        }
                    )),
                    name: None,
                }
            );
        }
    }
    if let Some(sele_skills) = selected_skills {
        let skill_prompt = match sele_skills {
            SelectedSkills::All => PARAS.skills.get_all_available_skills_prompt(),
            SelectedSkills::Group(group) => PARAS.skills.get_group_available_skills_prompt(group),
            SelectedSkills::Single(idx) => PARAS.skills.get_single_available_skill_prompt(idx),
        };
        history_messages.push(
            ChatMessage::User{
                content: ChatMessageContent::Text(skill_prompt),
                name: None,
            }
        );
    }
    let mut call_tool_count = get_tool_calling_count(&uuid); // call tool count
    let call_tool_limit = call_tool_count+100; // 调用工具的次数限制
    let mut try_count = 0;
    let mut is_first: bool;
    let mut real_name: String; // 要调用的工具的真实名称，含有`_uuid第一部分`后缀

    // 进入循环前，如果当前 uuid 的新指令不为 None，则设为 None
    reset_new_instruction(&uuid);
    //'outer: loop {
    loop {
        // 每次循环都检查下是否有新指令，有则插入到当前 history_messages 中
        loop {
            if let Some(instruction_msg) = get_new_instruction(&uuid) {
                if instruction_msg == "wait" {
                    event!(Level::INFO, "{} waiting new instruction ...", uuid);
                    sleep(Duration::from_secs(5)).await;
                } else {
                    history_messages.push(
                        ChatMessage::User{
                            content: ChatMessageContent::Text(instruction_msg),
                            name: None,
                        }
                    );
                    break
                }
            } else {
                break
            }
        }
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
                        &uuid,
                        &name_id,
                        &j.1,
                        j.3.clone(),
                        sender.clone(),
                        model,
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
                            if let MyError::PlanModeError{ref info} = e {
                                if info.starts_with("skip, this tool has not been executed: ") {
                                    (info.clone(), "text".to_string(), false)
                                } else {
                                    event!(Level::WARN, "{} call tool {}, raw args: {}", uuid, name_id[0], j.1);
                                    //event!(Level::WARN, "{} call tool {}, safe args: {}", uuid, name_id[0], safe_args);
                                    return Err(e)
                                }
                            } else {
                                event!(Level::WARN, "{} call tool {}, raw args: {}", uuid, name_id[0], j.1);
                                //event!(Level::WARN, "{} call tool {}, safe args: {}", uuid, name_id[0], safe_args);
                                return Err(e)
                            }
                        },
                    };
                    call_tool_count += 1;

                    // 更新 goal 状态
                    if goal_mode && name_id[0] == "update_goal_status" {
                        if let Some(ref mut goal) = my_goal {
                            goal.update_goal_status(GoalStatus::from_str(&result)?)?;
                            result = format!("successfully update goal status to '{}'", result)
                        }
                    }

                    // 1. send call tool result to user page
                    let messages_num = get_messages_num(&uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
                    // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
                    let test_result = if is_image {
                        "load image successful".to_string()
                    } else if result.contains("```") {
                        result.clone()
                    } else {
                        format!("```{}\n{}\n```", language, result)
                    };
                    let test_result = if let Some(content) = &j.3 {
                        if content.is_empty() {
                            format!("## 📌 {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", call_tool_count, name_id[0], &j.1, test_result)
                        } else {
                            format!("## 📌 {} call tool\n\n---\n\n{}\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", call_tool_count, content, name_id[0], &j.1, test_result)
                        }
                    } else {
                        format!("## 📌 {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", call_tool_count, name_id[0], &j.1, test_result)
                    };
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, test_result.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0), None, name_id[0] == "edit_file")?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "channel send error: {:?}", e);
                        break
                    }

                    // 2. add result to main message history
                    let message = ChatMessage::Assistant{
                        content: Some(ChatMessageContent::Text(test_result)),
                        reasoning: None,
                        reasoning_content: None,
                        refusal: None,
                        name: None,
                        audio: None,
                        tool_calls: None,
                    };
                    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
                    insert_message(&uuid, message, None, tmp_time, false, DataType::Normal, None, model, None);

                    // 如果是绘图，则把图片显示在页面
                    if name_id[0] == "image_generation" || name_id[0] == "edit_image" {
                        let img_name = result.split("/").last().unwrap();
                        let image_b64 = image_to_base64(&uuid, img_name)?;
                        if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num+1, image_b64.clone(), true, true, false, false, false, None, Some(0), None, false)?).await { // 传递数据以`data: `起始，以`\n\n`终止
                            event!(Level::WARN, "channel send error: {:?}", e);
                            break
                        }

                        let message = ChatMessage::Assistant{
                            content: Some(ChatMessageContent::Text(result.clone())),
                            reasoning: None,
                            reasoning_content: None,
                            refusal: None,
                            name: None,
                            audio: None,
                            tool_calls: None,
                        };
                        let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
                        insert_message(&uuid, message, None, tmp_time, false, DataType::Image(image_b64), None, model, None);

                        result = if name_id[0] == "image_generation" {
                            format!("create image successfull: {}", result)
                        } else if name_id[0] == "edit_image" {
                            format!("edit image successfull: {}", result)
                        } else {
                            result
                        };
                    }

                    // 3. 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                    let meta_data = MetaData::new(uuid.clone(), None, false);
                    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "channel send error: {:?}", e);
                        break
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
                // 1. send to user page
                let messages_num = get_messages_num(&uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
                // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
                if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, test_result.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0), None, false)?).await { // 传递数据以`data: `起始，以`\n\n`终止
                    event!(Level::WARN, "channel send error: {:?}", e);
                }
                // 2. add result to main message history
                let message = ChatMessage::Assistant{
                    content: Some(ChatMessageContent::Text(test_result)),
                    reasoning: None,
                    reasoning_content: None,
                    refusal: None,
                    name: None,
                    audio: None,
                    tool_calls: None,
                };
                let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
                insert_message(&uuid, message, None, tmp_time.clone(), false, DataType::Normal, None, model, None);

                if let Some(ref mut goal) = my_goal {
                    if goal.is_active() { // 没有完成 goal，继续
                        event!(Level::INFO, "{} continue goal", uuid);
                        let message = ChatMessage::User{
                            content: ChatMessageContent::Text(goal.take_continuation_prompt()),
                            name: None,
                        };
                        insert_message(&uuid, message, None, tmp_time, false, DataType::Normal, None, model, None);
                    } else {
                        reset_goal(&uuid); // 客户端开启的指定 uuid 的 goal 设为 None
                        break
                    }
                } else {
                    break
                }
            },
        }
        if !goal_mode && call_tool_count > call_tool_limit { // goal 模式不限制
            event!(Level::WARN, "{} already call tool {} times, stop this loop", uuid, call_tool_count);
            break
        }
    }
    // page left info
    let meta_data = MetaData::new(uuid.clone(), None, false);
    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "channel send error: {:?}", e);
    }
    Ok(())
}

/// sub-agent
/// 不能再开启 sub-agent，避免无限递归调用，会自动把 sub_agent 工具过滤掉
/// 也不能更新 goal 的状态，会自动把 update_goal_status 工具过滤掉
pub async fn sub_agent(
    uuid: String,
    prompt: String,
    sender: Sender<Vec<u8>>,
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
                        j.3.clone(),
                        sender.clone(),
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
                            if let MyError::PlanModeError{ref info} = e {
                                if info.starts_with("sub-agent skip, this tool has not been executed: ") {
                                    (info.clone(), "text".to_string(), false)
                                } else {
                                    event!(Level::WARN, "{} sub-agent call tool {}, raw args: {}", uuid, name_id[0], j.1);
                                    //event!(Level::WARN, "{} call tool {}, safe args: {}", uuid, name_id[0], safe_args);
                                    return Err(e)
                                }
                            } else {
                                event!(Level::WARN, "{} sub-agent call tool {}, raw args: {}", uuid, name_id[0], j.1);
                                //event!(Level::WARN, "{} call tool {}, safe args: {}", uuid, name_id[0], safe_args);
                                return Err(e)
                            }
                        },
                    };
                    call_tool_count += 1;

                    // 如果是绘图，则把图片显示在页面
                    if name_id[0] == "image_generation" || name_id[0] == "edit_image" {
                        result = if name_id[0] == "image_generation" {
                            format!("create image successfull: {}", result)
                        } else if name_id[0] == "edit_image" {
                            format!("edit image successfull: {}", result)
                        } else {
                            result
                        };
                    }

                    // 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                    let meta_data = MetaData::new(uuid.clone(), None, false);
                    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "channel send error: {:?}", e);
                        break
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
    // page left info
    let meta_data = MetaData::new(uuid.clone(), None, false);
    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "sub-agent channel send error: {:?}", e);
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
    sender: Sender<Vec<u8>>,
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
                sender.clone(),
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

const LARGE_FILE_PROMPT: &str = r###"The main agent is delegating this work to preserve its context window. The requested work may require reading a large file, multiple files, long logs, generated output, structured data, source code, documentation, or other context-heavy material.

Your job is to inspect the relevant material using the available tools and return a compact, evidence-backed report. Do not include unnecessary raw content. Do not solve unrelated parts of the task.

Original main agent task:
"###;

/// 调用的skill
#[derive(Deserialize)]
struct SkillParams {
    skill_name: String,
}

/// try run tool, if error not from call tool, return Err(), else return Ok(Ok()) or Ok(Err())
async fn try_call_tool(
    uuid: &str,
    name_id: &[&str],
    paras: &str,
    info: Option<String>,
    sender: Sender<Vec<u8>>,
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
        if PARAS.approval_all {
            Ok(PARAS.tools.run(name_id[1], paras))
        } else {
            match PARAS.tools.get_approval(name_id[1], paras, info, PARAS.english)? {
                Some(approval_msg) => {
                    let approval_msg = if name_id[0] == "edit_file" {
                        /*
                        let mut params: Params = match serde_json::from_str(paras) {
                            Ok(p) => p,
                            Err(e) => return Ok(Err(MyError::SerdeJsonFromStrError{error: e})),
                        };
                        */
                        let mut params: Params = match parse_tool_args(paras, ArgFixSpec{ array_fields: Some(vec!["edits".to_string()]), object_fields: None }) {
                            Ok(p) => p,
                            Err(e) => return Ok(Err(e)),
                        };

                        params.dry_run = Some(true);
                        let dry_run_para = match serde_json::to_string(&params) {
                            Ok(d) => d,
                            Err(e) => return Ok(Err(MyError::JsonToStringError{error: e.into()})),
                        };
                        match PARAS.tools.run(name_id[1], &dry_run_para) {
                            Ok(r) => r.0,
                            Err(e) => return Ok(Err(e)),
                        }
                    } else if name_id[0] == "write_file" {
                        approval_msg.chars().take(100).collect() // 截取显示100个字符，否则弹窗很高，无法点击同意或拒绝
                    } else {
                        approval_msg
                    };
                    match ask_approval(uuid, approval_msg, name_id[0] == "edit_file", sender.clone()).await?.as_ref() {
                        "true" => { // 允许
                            if name_id[0] == "image_generation" {
                                match PARAS.tools.run(name_id[1], paras) {
                                    Ok((image_prompt, _)) => {
                                        match image_generation(uuid, image_prompt, "gpt-image-2").await {
                                            Ok(image_path) => Ok(Ok((image_path, None))),
                                            Err(e) => Ok(Err(e)),
                                        }
                                    },
                                    Err(e) => Ok(Err(e)),
                                }
                            } else if name_id[0] == "edit_image" {
                                match PARAS.tools.run(name_id[1], paras) {
                                    Ok((facial_prompt_image, _)) => {
                                        let parts: Vec<&str> = facial_prompt_image.splitn(3, "---srx---").collect(); // [是否强调面部特征, prompt, 图片路径]
                                        match edit_image(uuid, parts[0] == "true", parts[2].split("---srx---").map(|img| img.to_string()).collect::<Vec<String>>(), parts[1], "gpt-image-2").await {
                                            Ok(image_path) => Ok(Ok((image_path, None))),
                                            Err(e) => Ok(Err(e)),
                                        }
                                    },
                                    Err(e) => Ok(Err(e)),
                                }
                            } else if name_id[0] == "schedule_task" {
                                match PARAS.tools.run(name_id[1], paras) {
                                    Ok(_) => Ok(run_schedule_task(paras).await),
                                    Err(e) => Ok(Err(e)),
                                }
                            } else if name_id[0] == "sub_agent" {
                                run_sub_agent(
                                    uuid,
                                    name_id,
                                    paras,
                                    model,
                                    tool_schema,
                                    para_builder,
                                    client,
                                    sender,
                                    false,
                                ).await
                            } else {
                                Ok(PARAS.tools.run(name_id[1], paras))
                            }
                        },
                        "false" => return Err(MyError::PlanModeError{info: format!("Not allowed to call this tool: {}", name_id[0])}), // 不允许
                        "skip" => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}", name_id[0])}), // 跳过
                        new_prompt => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}\n{}", name_id[0], new_prompt)}), // 跳过的新指示
                    }
                },
                None => if name_id[0] == "hacker_news" {
                    match PARAS.tools.run(name_id[1], paras) {
                        Ok((save_html, _)) => {
                            match hacker_news_summaries(&uuid, save_html == "true", model).await {
                                Ok(hn_summaries) => Ok(Ok((hn_summaries, None))),
                                Err(e) => Ok(Err(e)),
                            }
                        },
                        Err(e) => Ok(Err(e)),
                    }
                } else if is_main_agent && name_id[0] == "read_file" { // 读取大文件时转为调用 sub-agent
                    let read_file_para: ReadFileParams = parse_tool_args(paras, ArgFixSpec{ array_fields: None, object_fields: None })?;
                    // 小文件（<4000）和 md 文件可以直接读取，大文件则通过 sub_agent 读取
                    let file_path = Path::new(&read_file_para.file_path);
                    let ext = if let Some(ext) = file_path.extension() {
                        Some(ext.to_ascii_lowercase().to_str().unwrap().to_string())
                    } else {
                        None
                    };
                    let metadata = file_path.metadata()?;
                    if metadata.len() < 4000 || if let Some(e) = ext { e == "md" } else { false } { // 直接读取
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
                            sender,
                            true,
                        ).await
                    }
                } else if is_main_agent && name_id[0] == "read_multiple_files" { // 读取多个文件时转为调用 sub-agent
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
                        sender,
                        true,
                    ).await
                } else {
                    Ok(PARAS.tools.run(name_id[1], paras))
                }
            }
        }
    } else if PARAS.mcp_servers.contain_server_id(name_id[1]) {
        Ok(PARAS.mcp_servers.run(&name_id, paras).await)
    } else {
        return Ok(Err(MyError::ToolNotExistError{id: name_id[1].to_string(), info: "run_tools".to_string()}))
    }
}

/// https://github.com/microsoft/TaskWeaver/blob/main/taskweaver/planner/planner_prompt.yaml
/// https://github.com/x1xhlol/system-prompts-and-models-of-ai-tools/blob/main/Augment%20Code/claude-4-sonnet-agent-prompts.txt
/// https://github.com/x1xhlol/system-prompts-and-models-of-ai-tools/blob/main/Augment%20Code/gpt-5-agent-prompts.txt
/// https://github.com/x1xhlol/system-prompts-and-models-of-ai-tools/blob/main/Open%20Source%20prompts/Codex%20CLI/openai-codex-cli-system-prompt-20250820.txt
/// https://github.com/x1xhlol/system-prompts-and-models-of-ai-tools/blob/main/Qoder/Quest%20Action.txt
/// https://github.com/potpie-ai/potpie/blob/main/app/modules/intelligence/agents/chat_agents/pydantic_multi_agent.py
/// https://github.com/potpie-ai/potpie/blob/main/app/modules/intelligence/agents/custom_agents/custom_agents_service.py
/// https://github.com/evalstate/fast-agent/blob/main/src/fast_agent/agents/workflow/iterative_planner.py
/// https://github.com/evalstate/fast-agent/blob/main/src/fast_agent/agents/workflow/orchestrator_prompts.py

const PLAN_FORMAT_PROMPT: &str = r###"
# Output Format
**Always** respond in strict JSON format, structured as follows:

{
    "steps": [
        {
            "description": "string",
            "status": "string", // 'pending', 'in_progress', 'completed', 'update_plan', 'failed'
            "tool_name": "string | null",
            "ask_approval": "boolean",
            "result": "string"
        }
        // ... more steps
    ],
    "all_steps_completed": "boolean",
    "final_result": "string",
    "error_msg": ""
}

Example response (step 1 and 2 have been completed, next step is steps 3, step 4 is planned but pending):

{
    "steps": [
        {
            "description": "Read the content of the source data file.",
            "status": "completed",
            "tool_name": "read_file",
            "ask_approval": false,
            "result": "Successfully read 'data.csv', it contains headers and 100 rows of data.",
        },
        {
            "description": "analyze the structure of the data to confirm it's tab-separated.",
            "status": "completed",
            "tool_name": null,
            "ask_approval": false,
            "result": "The file 'data.csv' uses a comma as a delimiter, not a tab. The plan will be updated.",
        },
        {
            "description": "Process the data using a comma delimiter and generate a summary.",
            "status": "in_progress",
            "tool_name": "data_analyzer",
            "ask_approval": false,
            "result": "",
        },
        {
            "description": "Write the generated summary to a new file named 'summary.txt'.",
            "status": "pending",
            "tool_name": "write_file",
            "ask_approval": true,
            "result": "",
        },
    ],
    "all_steps_completed": false,
    "final_result": "",
    "error_msg": "",
}

# Error Response Format
If ANY step cannot be executed due to missing tools or capabilities, return:

{
    "steps": [],
    "all_steps_completed": false,
    "final_result": "",
    "error_msg": "Cannot complete the task because [specific step description] requires [required capability/tool] which is not available."
}

Example debug code plan:

{
    "steps": [
        {
            "description": "Read the content of the source code file to examine its current code.",
            "status": "in_progress",
            "tool_name": "read_file",
            "ask_approval": false,
            "result": "content of source code file",
        },
        {
            "description": "Analyze the source code to identify any syntax errors, logical flaws, or runtime issues.",
            "status": "pending",
            "tool_name": "null",
            "ask_approval": false,
            "result": "errors, logical flaws, or runtime issues of source code.",
        },
        {
            "description": "Generate a corrected version of code with the identified issues fixed.",
            "status": "pending",
            "tool_name": "null",
            "ask_approval": false,
            "result": "issues fixed code.",
        },
        {
            "description": "Update the source code file with the corrected code using the edit_file tool.",
            "status": "pending",
            "tool_name": edit_file,
            "ask_approval": true,
            "result": "",
        },
    ],
    "all_steps_completed": false,
    "final_result": "",
    "error_msg": "",
}

"###;

const UPDATE_PLAN_PROMPT: &str = r###"
# Update Previous Plans

You do not create new plans, you intelligently modify the current one based on new information.

A step in the current plan has just been marked with `status: "update_plan"`. This is your signal to intervene.

You must analyze the reason for the failure and intelligently revise the existing plan.

1. **Analyze the Failure**:
    - Locate the step in the `steps` array where `status` is `"update_plan"`.
    - Read its `result` field carefully. This field contains the critical reason why the original plan failed (e.g., a tool error, unexpected data, a dead end).

2. **Diagnose and Revise the Plan**:
    - Based on the failure reason and previous step results, evaluate the rest of the plan. How does this failure impact subsequent steps?
    - You may need to perform one or more of the following actions on the `steps` array:
        - **Modify**: Change the `description` or `tool_name` of a future step to accommodate the new reality.
        - **Insert**: Add one or more new steps before the failed step to rectify the issue (e.g., add a data cleaning step if the data format is wrong).
        - **Delete**: Remove steps that are no longer relevant or possible.
        - **Re-order**: Change the sequence of steps if a different approach is now required.
    - Your revision must directly address the failure and create a viable path towards the original goal.

3.  **Reset Execution Status**:
    - Change the failed step's `status` from `"update_plan"` to `failed`. This serves as a record of what went wrong. Ensure its `result` clearly states the failure reason.
    - Identify the **new next step to be executed** in your revised plan.
    - Set the `status` of this new next step to `"in_progress"`.
    - Ensure all subsequent steps in your revised plan have their `status` set to `"pending"`.
    - Keep `all_steps_completed` as `false`.
    - Keep `final_result` as `""`.

# Revision Principles

- **Minimal Change**: Make the smallest possible modification that will fix the plan. Avoid wholesale rewrites unless absolutely necessary.
- **Goal Alignment**: Your revised plan must still aim to solve the original user task.
- **Logical Flow**: The new sequence of steps must be logical and coherent.

# Input

<chatsong:current-plan-status>
--current-plans--
</chatsong:current-plan-status>
"###;

const PLANER_PROMPT: &str = r###"
You are an advanced task planning specialist who break down complex multi-step task into comprehensive sequential plans.

# Planning Principles (STRICT ENFORCEMENT)
When creating or updating a plan, you must adhere to the following strict rules. **Violation of these rules will result in plan failure.**
  1. **Strict Tool Dependency**: You act within a "Closed World" regarding external actions. You **cannot** perform any Input/Output (I/O) operations (e.g., reading files, writing files, saving data, searching the web, sending emails) unless a specific tool is provided in `<chatsong:available-tools>`.
  2. **Internal vs. External Actions**:
     - **Internal Actions (Tool = null)**: You MAY perform pure reasoning, text summarization, formatting, or logic calculation using your internal knowledge.
     - **External Actions (Tool MUST exist)**: You MUST verify a tool exists for any step involving file manipulation, data retrieval, or persistent storage.
  3. **The "Missing Tool" Protocol**: If a necessary step requires an External Action (e.g., "save to file") but no corresponding tool exists in the provided list `<chatsong:available-tools>`:
     - You **MUST NOT** pretend you can do it.
     - You **MUST NOT** assign `tool_name: null` to that step.
     - You **MUST** immediately abort the entire planning process and return the **Error JSON**.
  4. **Predictive Logical Sequence (CRITICAL)**:
     - The order of steps must respect dependencies.
     - You must generate a **comprehensive initial plan** that covers the entire lifecycle of the task.
     - **Do NOT stop** the plan after information gathering (e.g., "list directories" or "search").
     - **Always anticipate the next steps**: If a step gathers information (e.g., finding a file path), you **MUST** immediately follow it with steps that use that information (e.g., reading that file), assuming the information will be successfully retrieved.
     - Example: If the task is "Fix a bug", the plan must include: 1. Find file -> 2. Read file -> 3. Modify file -> 4. Save file.
  5. **Atomicity**: Each step should be focused on a single, clear, and indivisible action.
  6. **Tool Matching**: Specify a `tool_name` only when a step genuinely requires it, and ensure the tool name is correct and exists.
  7. **Status-Independent Tooling**: The `tool_name` for a step is determined at the planning stage based on the step's intrinsic need for a tool. You MUST specify the correct `tool_name` for any step that requires a tool, regardless of whether its status is `pending`, `in_progress`, or `completed`. A `pending` status only means "waiting to be executed," not "tool undetermined".

# Capability Boundaries
**CRITICAL**: You are a text-processing engine. You do NOT have a file system, you do NOT have a browser, and you do NOT have a command line unless a Tool provides that interface.
  - **Example of Impossible Action**: "Save the result to `abc.txt`" -> If no `save_file` or `write_file` tool is provided, this step is **IMPOSSIBLE**. You cannot "just do it."
  - **Example of Possible Action**: "Summarize the text read in step 1" -> This is **POSSIBLE** as an internal action (`tool_name: null`).

# Persona and Objective
  - The content of your plan should not involve doing anything that you aren't capable of doing.
  - Do not use plans for simple or single-step queries that you can just do or answer immediately.
  - Each step should contains 5 properties:
    - description: clear purpose of this step.
    - status: progress status, include `in_progress`, `pending`, `completed`, `update_plan`, `failed`.
    - tool_name: specify the Tool name precisely if need use Tools.
    - ask_approval: a boolean flag indicating whether user confirmation is required before executing this step.
      - Set to `true` ONLY if the operation involves modifying system state, specifically: creating, modifying, or deleting files.
      - Set to `false` for read-only operations (e.g., reading files, searching, querying), computational tasks, or steps that do not involve any tool usage.
    - result: result of this step.
  - Set the `status` of the **first step** to `in_progress`.
  - Set the `status` of **all other steps** to `pending`.

--update-plan--

# Step Attribute Definitions
- `description`: **String**. A concise, clear description of the step's purpose and action.
- `status`: **String**. The current state of the step. Must be one of the following:
    - `pending`: Waiting to be executed. Used for steps that are planned but not yet started.
    - `in_progress`: Currently being executed. Only one step may have this status at any time.
    - `completed`: Successfully finished. The `result` field should contain the outcome of the execution.
    - `update_plan`: Execution failed or based on previous step results, update plans. The `result` field should contain the specific reason.
- `tool_name`: **String or Null**. The exact name of the tool to be called for this step. Use `null` if no tool is required.
- `result`: **String**. The outcome or status note of the step.
    - When `status` is `in_progress` or `pending`, this field must be an empty string `""`.
    - When `status` is `completed`, this field records the step's output (e.g., file content, data analysis summary).
    - When `status` is `update_plan` or `failed`, this field records the reason.

# Available Tools and Constraints
<chatsong:available-tools>
--tools--
</chatsong:available-tools>


**CRITICAL WARNING**:
  - You **MUST** and **CAN ONLY** use tool names that are **EXACTLY** as listed in the `<chatsong:available-tools>` section above.
  - Do not use non-existent tools or modify tool names. The plan will fail if you do.
  - If the user asks to "save", "write", or "download" something, check the tools list. If no matching tool is found, you MUST return the `error_msg`.
--plan-format-prompt--
"###;

const CONTINUE_OR_UPDATE_PROMPT: &str = r###"
You are a Task Execution Controller, an AI specialist responsible for managing the execution of a multi-step plan. Your sole function is to receive the current status of a plan, evaluate the result of the most recently completed step, and make one of three decisions: **CONTINUE** to the next step, **UPDATE** the plan, or **TERMINATE** the plan as complete.

You are a logical and deterministic decision-maker. You do not add commentary; you only update the provided JSON state based on the rules below.

# Core Decision Logic

You will receive a JSON plan containing one step with `status: "in_progress"`. Your primary task is to analyze results from the previous steps already executed, and evaluate the `result` of that specific step and decide the next action.

1.  **Locate the Current Step**: Find the step in the `steps` array where `status` is `"in_progress"`.

2.  **Evaluate and Decide**: Analyze the `result` of the current step.

    - **Scenario A: Continue to Next Step**
        **Condition**: The `result` meets the objective of the current step, and the plan remains viable.
        **Detailed Logic**:
            1.  **For Analysis/Check Steps**: If the step is to "analyze", "check", "review", or "investigate", a result that lists "errors", "bugs", "issues", or "critical problems" is considered a **SUCCESSFUL OUTPUT**. Finding these issues implies the analysis step worked correctly and the next step (likely a fix) should proceed. Do **NOT** treat the discovery of bugs as a reason to update the plan unless the plan lacks a step to fix them.
            2.  **For Tool Calls**: The tool returned meaningful data without execution failures (e.g., timeouts, API errors, or "File not found").
            3.  **For General Steps**: The step's objective was achieved, even if the outcome reveals issues that need to be addressed in subsequent steps.
        **Action**:
            1.  Change the `status` of the current step from `"in_progress"` to `"completed"`.
            2.  Find the **next** step in the array (the one immediately following) whose `status` is `"pending"`.
            3.  Change that next step's `status` from `"pending"` to `"in_progress"`.
            4.  Keep `all_steps_completed` as `false`.
            5.  Keep `final_result` as `""`.

    - **Scenario B: Update the Plan**
        **Condition**: The `result` indicates a **failure to perform the current step**, or reveals that the **subsequent plan is logically impossible or unnecessary**.
        **Detailed Logic**:
            1.  **Execution Failure**: The tool crashed, timed out, returned a system error, or could not be accessed (e.g., "File not found" when trying to read).
            2.  **Empty/Unexpected Data**: Data was required to proceed, but the result was empty or in a completely unexpected format that the next step cannot handle.
            3.  **Plan Obsolescence**: The result shows that the goal is already achieved (e.g., "File is already correct" before a fix step), making the next steps unnecessary.
            4.  **Crucial Distinction**: If the result simply describes problems *within the target* (e.g., "Syntax error found in script"), but the current step was just to *analyze* it, this is **NOT** a reason to update the plan. This is a successful analysis. Only update if the *act of analyzing* failed or the next step cannot be performed.
        **Action**:
            1.  Change the `status` of the current step from `"in_progress"` to `"update_plan"`.
            2.  Ensure the `result` field clearly and concisely describes the reason for the update (e.g., "Tool 'search_web' failed with API error.", "Result was empty, cannot proceed.", "File content shows a different format than expected, requires new plan.").
            3.  Keep all other steps' status as they are.
            4.  Keep `all_steps_completed` as `false`.
            5.  Keep `final_result` as `""`.

    - **Scenario C: Terminate Plan (All Steps Completed)**
        - **Condition**: This is a special case of Scenario A. After successfully completing the final step of the plan, there are no more `"pending"` steps.
        - **Action**:
            1.  Change the status of the final step from `"in_progress"` to `"completed"`.
            2.  Set `all_steps_completed` to `true`.
            3.  Synthesize the `result` from all completed steps into a comprehensive final answer and place it in the `final_result` field.

# Input

You will receive the following context to make your decision:

<chatsong:current-plan-status>
--current-plans--
</chatsong:current-plan-status>

# Output
return the updated JSON
"###;

/// step status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pending,    // waiting to be executed
    InProgress, // in progress
    Completed,  // completed step
    UpdatePlan, // update plan because this step
    Failed,     // failed step
}

impl Status {
    /// convert to string
    fn to_string(&self) -> String {
        match self {
            Self::Pending    => "pending",
            Self::InProgress => "in progress",
            Self::Completed  => "completed",
            Self::UpdatePlan => "update plan",
            Self::Failed     => "failed",
        }.to_string()
    }
}

/// single step
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Step {
    description:  String,
    status:       Status,
    tool_name:    Option<String>,
    #[serde(default)]
    ask_approval: bool,
    result:       String,
}

impl Step {
    fn format_step(&self, num: usize) -> String {
        format!("step{} subtask: {}\nstep{} result: {}", num, self.description, num, self.result)
    }
}

/// plan
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plan {
    steps:               Vec<Step>,
    all_steps_completed: bool,
    final_result:        String,
    error_msg:           String,
}

impl Plan {
    /// convert json string to Plan
    fn from_str(s: &str) -> Result<Self, MyError> {
        let json_str = match (s.starts_with("```json"), s.ends_with("```")) {
            (true, true) => s.strip_prefix("```json").unwrap().strip_suffix("```").unwrap(),
            (true, false) => s.strip_prefix("```json").unwrap(),
            (false, true) => s.strip_suffix("```").unwrap(),
            (false, false) => s,
        };
        match serde_json::from_str(json_str.trim()) {
            Ok(p) => Ok(p),
            Err(e) => {
                event!(Level::ERROR, "json string to Plan struct: {}", json_str);
                Err(MyError::SerdeJsonFromStrError{error: e})
            },
        }
    }

    /// format plan
    fn format_plan(&self, init: bool) -> String {
        let mut result: Vec<String> = Vec::new();
        for (i, p) in self.steps.iter().enumerate() {
            match &p.tool_name {
                Some(t) => {
                    let name_id: Vec<&str> = t.split("__").collect();
                    if init {
                        result.push(format!("{}. [tool: {}] {}", i+1, name_id[0], p.description));
                    } else {
                        result.push(format!("{}. [status: {}] [tool: {}] {}", i+1, p.status.to_string(), name_id[0], p.description));
                    }
                },
                None => {
                    if init {
                        result.push(format!("{}. {}", i+1, p.description));
                    } else {
                        result.push(format!("{}. [status: {}] {}", i+1, p.status.to_string(), p.description));
                    }
                }
            }
        }
        result.join("\n")
    }

    /// get each step description and result for next step
    fn format_steps(&self, idx: usize) -> String {
        let mut result: Vec<String> = Vec::new();
        for (i, s) in self.steps[0..idx].iter().enumerate() {
            result.push(s.format_step(i+1));
        }
        let final_result = result.join("\n\n");
        if final_result.is_empty() {
            final_result
        } else {
            final_result+"\n\n"
        }
    }
}

/// first make a plan, then chat with tools: built-in + external tools
/// 1. make plan
/// 2. post each plan step to LLM api
/// 3. based on result, continue or update plan
/// 4. all plan steps have been finished, send final result to uer, record this message
/// Only the final result will be recorded, the intermediate context messages will be discarded.
/// +-----------------------------------------------------------------------------------+
/// |                     question                                                      |
/// |                        |                                                          |
/// |                        V                                                          |
/// |                 make/update plan <----------------------------------------------. |
/// |                        |                                                        | |
/// | +----------------------+--------------------+                                   | |
/// | |                      |                    |                                   | |
/// | |                      V                    |                                   | |
/// | | .-------------> step by step              |                                   | |
/// | | |                    |                    |                                   | |
/// | | |                    V                    |                                   | |
/// | | |                   LLM <-----------------+------------------.                | |
/// | | |                    |                    |                  |                | |
/// | | |                    V                    |                  |                | |
/// | | |                call tool ?              |                  |                | |
/// | | |                    |                    |                  |                | |
/// | | |                +-------+                |                  |                | |
/// | | |                |       |                |    +----------------------------+ | |
/// | | |                V       V                |    | run tool, push function    | | |
/// | | |                No      Yes -------------+--> | calling result to function | | |
/// | | |                |                        |    | calling history            | | |
/// | | |                V                        |    +----------------------------+ | |
/// | | |          update plan ?                  |                                   | |
/// | | |                |                        |                                   | |
/// | | |            +-------+                    |                                   | |
/// | | |            No      Yes -----------------+-----------------------------------' |
/// | | |            |                            |                                     |
/// | | |            V                            |                                     |
/// | | |    +----------------------------------+ |                                     |
/// | | '--- | push current step result to main | |                                     |
/// | |      | history, continue next step      | |                                     |
/// | |      +----------------------------------+ |                                     |
/// | +-------------------------------------------+                                     |
/// |                        |                                                          |
/// |                        V                                                          |
/// |          +---------------------------+                                            |
/// |          | return final result, push |                                            |
/// |          | result to global history  |                                            |
/// |          +---------------------------+                                            |
/// +-----------------------------------------------------------------------------------+
/// parameters:
/// +--------------------------------------------------------------------+
/// | selected_tools: html pulldown selected tools                       |
/// | uuid:           current chat uuid                                  |
/// | sender:         channel for send final result to user page         |
/// | client:         OpenAI api Client for send request                 |
/// | para_builder:   update history messages before send request to LLM |
/// | model:          model name                                         |
/// +--------------------------------------------------------------------+
/// function calling:
/// https://api-docs.deepseek.com/zh-cn/guides/function_calling
/// https://docs.bigmodel.cn/cn/guide/capabilities/function-calling
/// https://platform.moonshot.cn/docs/api/tool-use
pub async fn run_tools_with_plan(selected_tools: Option<SelectedTools>, uuid: String, sender: Sender<Vec<u8>>, client: Client, para_builder: ChatCompletionParametersBuilder, model: &str) -> Result<(), MyError> {
    // get built-in and external tools schame
    let mut tool_schema = PARAS.tools.get_desc_and_schema(&selected_tools)?;
    // get mcp tools schema
    let mcp_schema = PARAS.mcp_servers.get_desc_and_schema(&selected_tools).await?;

    tool_schema.extend(mcp_schema);
    //let tool_schema_json = serde_json::to_string(&tool_schema).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
    let tool_schema_json = tool_schema.iter().map(|t| format!("tool: {}, description: {:?}", t.function.name, t.function.description)).collect::<Vec<String>>().join("\n");
    let plan_prompt = PLANER_PROMPT.replace("--tools--", &tool_schema_json).replace("--plan-format-prompt--", PLAN_FORMAT_PROMPT);
    let history_messages: Vec<ChatMessage> = get_messages(&uuid); // store all messages as context log, this is temp history, will not add to the user's main history

    // make a new plan
    let mut plan_string = make_update_plan(uuid.clone(), client.clone(), para_builder.clone(), model, history_messages.clone(), &plan_prompt, None).await?;
    let mut first_step = true;
    let mut plan_struct: Plan;
    let mut max_update_plan = 0;
    //println!("\n{:?}\n", plan_string);

    loop {
        plan_struct = Plan::from_str(&plan_string)?;
        if first_step {
            let msg = format!("## 🚩 make plan\n\n---\n\n{}", plan_struct.format_plan(true));
            send_and_record_message(&uuid, msg, 0, model, sender.clone(), false).await?;
            first_step = false;
        }
        if plan_struct.all_steps_completed && plan_struct.steps.iter().any(|s| s.status == Status::Pending) {
            plan_struct.all_steps_completed = false;
        }
        if !plan_struct.error_msg.is_empty() {
            let msg = format!("## 🤔 error\n\n---\n\n{}", plan_struct.error_msg);
            send_and_record_message(&uuid, msg, plan_struct.steps.len()+1, model, sender.clone(), false).await?;
            break
        } else if plan_struct.all_steps_completed || plan_struct.steps.iter().all(|s| s.status == Status::Completed) {
            let msg = format!("## 📌 final result\n\n---\n\n{}", plan_struct.final_result);
            send_and_record_message(&uuid, msg, plan_struct.steps.len()+1, model, sender.clone(), false).await?;
            break
        } else {
            if !plan_struct.steps.iter().any(|s| s.status == Status::InProgress || s.status == Status::UpdatePlan) {
                if plan_struct.steps.iter().any(|s| s.status == Status::Pending) {
                    event!(Level::WARN, "all steps not contain \"in_progress\" or \"update_plan\", change first \"Pending\" to \"InProgress\"");
                    for i in 0..plan_struct.steps.len() {
                        if plan_struct.steps[i].status == Status::Pending {
                            plan_struct.steps[i].status = Status::InProgress;
                        }
                    }
                }
            }
            for i in 0..plan_struct.steps.len() {
                match &plan_struct.steps[i].status {
                    Status::Pending | Status::Completed | Status::Failed => (),
                    Status::InProgress => {
                        let mut tool_name = "".to_string();
                        let mut edit_file_result = "".to_string();
                        if let Some(t) = &plan_struct.steps[i].tool_name {
                            tool_name = t.clone();
                            let step_tools = match tool_schema.iter().find(|item| &item.function.name == t) {
                                Some(s) => vec![s.clone()],
                                None => {
                                    match tool_schema.iter().find(|item| item.function.name.starts_with(&format!("{}__", t))) {
                                        Some(s) => {
                                            event!(Level::WARN, "step use tool: {} ({})", t, s.function.name);
                                            vec![s.clone()]
                                        },
                                        None => {
                                            event!(Level::ERROR, "step tool_name not correct: {} ({})", t, tool_schema.iter().map(|iterm| iterm.function.name.clone()).collect::<Vec<_>>().join(", "));
                                            return Err(MyError::PlanModeError{info: format!("step tool_name not correct: {}", t)})
                                        },
                                    }
                                },
                            };
                            let step_messages = vec![
                                ChatMessage::User{
                                    content: ChatMessageContent::Text(format!("{}current step subtask: {}", plan_struct.format_steps(i), plan_struct.steps[i].description)),
                                    name: None,
                                }
                            ];
                            for j in 0..3 {
                                match function_calling(
                                    step_tools.clone(),
                                    step_messages.clone(),
                                    &uuid,
                                    client.clone(),
                                    para_builder.clone(),
                                    sender.clone(),
                                    plan_struct.steps[i].ask_approval,
                                    //model,
                                ).await? {
                                    Ok(r) => {
                                        if t == "edit_file" || t.starts_with("edit_file__") {
                                            plan_struct.steps[i].result = format!("successfully edit file:\n{}", r);
                                            edit_file_result = r;
                                        } else {
                                            plan_struct.steps[i].result = r;
                                        }
                                        break
                                    },
                                    Err(e) => {
                                        if j == 2 {
                                            return Err(e)
                                        } else {
                                            event!(Level::WARN, "{} call tool {} error, try again {}", uuid, t, j+1);
                                        }
                                    },
                                }
                            }
                        }
                        plan_string = serde_json::to_string(&plan_struct).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
                        plan_string = make_decision(uuid.clone(), client.clone(), para_builder.clone(), model, history_messages.clone(), &plan_string).await?;
                        let mut plan_struct_new = Plan::from_str(&plan_string)?;
                        if plan_struct_new.steps[i].result.is_empty() {
                            plan_struct_new.steps[i].status = Status::UpdatePlan;
                            plan_string = serde_json::to_string(&plan_struct_new).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
                        } else {
                            let msg = if plan_struct_new.steps[i].result.contains("```") {
                                if edit_file_result.is_empty() {
                                    plan_struct_new.steps[i].result.clone()
                                } else {
                                    edit_file_result
                                }
                            } else {
                                format!("```\n{}\n```", if edit_file_result.is_empty() { &plan_struct_new.steps[i].result } else { &edit_file_result })
                            };
                            let msg = format!("## 📌 step {}\n\n---\n\n### 📝 description\n{}\n\n### ✨ result\n{}", i+1, plan_struct_new.steps[i].description, msg);
                            send_and_record_message(&uuid, msg, i+1, model, sender.clone(), tool_name == "edit_file" || tool_name.starts_with("edit_file__")).await?;
                        }
                    },
                    Status::UpdatePlan => {
                        if max_update_plan > 5 {
                            event!(Level::WARN, "already update {} times plan, stop this loop", max_update_plan);
                            plan_struct.error_msg = format!("already update {} times plan, stop this loop", max_update_plan);
                            plan_string = serde_json::to_string(&plan_struct).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
                        } else {
                            plan_string = make_update_plan(uuid.clone(), client.clone(), para_builder.clone(), model, history_messages.clone(), &plan_prompt, None).await?;
                            let plan_struct_new = Plan::from_str(&plan_string)?;
                            let msg = format!("## 📌 step {} update plan\n\n---\n\n{}", i+1, plan_struct_new.format_plan(false));
                            send_and_record_message(&uuid, msg, i+1, model, sender.clone(), false).await?;
                            max_update_plan += 1;
                        }
                        break
                    },
                }
            }
        }
    }
    Ok(())
}

/// call LLM
async fn call_llm(messages: Vec<ChatMessage>, uuid: String, client: Client, mut para_builder: ChatCompletionParametersBuilder, model: &str) -> Result<String, MyError> {
    para_builder.messages(messages);
    let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
    Ok(not_use_stream(uuid, client, parameters, model, false).await?.0)
}

/// make a new plan or update provided plan
async fn make_update_plan(uuid: String, client: Client, para_builder: ChatCompletionParametersBuilder, model: &str, mut history_messages: Vec<ChatMessage>, plan_prompt: &str, previous_plan: Option<String>) -> Result<String, MyError> {
    let new_plan_prompt = ChatMessage::User{
        content: ChatMessageContent::Text(match previous_plan {
            Some(plan) => plan_prompt.replace("--update-plan--", &UPDATE_PLAN_PROMPT.replace("--current-plans--", &plan)),
            None => plan_prompt.replace("--update-plan--\n", ""),
        }),
        name: None,
    };
    history_messages.push(new_plan_prompt);
    let result = call_llm(history_messages, uuid, client, para_builder, model).await?;
    Ok(result)
}

/// decision-maker
async fn make_decision(uuid: String, client: Client, para_builder: ChatCompletionParametersBuilder, model: &str, mut history_messages: Vec<ChatMessage>, plan: &str) -> Result<String, MyError> {
    let decision_prompt = ChatMessage::User{
        content: ChatMessageContent::Text(CONTINUE_OR_UPDATE_PROMPT.replace("--current-plans--", plan)),
        name: None,
    };
    history_messages.push(decision_prompt);
    let result = call_llm(history_messages, uuid, client, para_builder, model).await?;
    Ok(result)
}

/// function calling
async fn function_calling(
    step_tools: Vec<ChatCompletionTool>,
    step_messages: Vec<ChatMessage>,
    uuid: &str,
    client: Client,
    mut para_builder: ChatCompletionParametersBuilder,
    sender: Sender<Vec<u8>>,
    step_ask_approval: bool,
    //model: &str
) -> Result<Result<String, MyError>, MyError> {
    let mut final_result = "".to_string();
    // send query to LLM
    para_builder.tools(step_tools);
    para_builder.messages(step_messages.clone());
    let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
    let answer = call_tool_not_use_stream(uuid, client.clone(), parameters).await?;
    match answer {
        CallToolResult::CallTool((_raw_message, call_tool_result)) => { // (ChatMessage, Vec<(tool name, tool args, call tool id, content)>)
            for i in call_tool_result {
                // call tool
                let name_id: Vec<&str> = i.0.split("__").collect();
                let result = if PARAS.tools.contain_tool_id(name_id[1]) {
                    if PARAS.approval_all {
                        match PARAS.tools.run(name_id[1], &i.1) {
                            Ok(r) => r.0,
                            Err(e) => return Ok(Err(e)),
                        }
                    } else if let Some(approval_msg) = PARAS.tools.get_approval(name_id[1], &i.1, i.3.clone(), PARAS.english)? {
                        let approval_msg = if name_id[0] == "edit_file" {
                            /*
                            let mut params: Params = match serde_json::from_str(&i.1) {
                                Ok(p) => p,
                                Err(e) => return Ok(Err(MyError::SerdeJsonFromStrError{error: e})),
                            };
                            */
                            let mut params: Params = match parse_tool_args(&i.1, ArgFixSpec{ array_fields: Some(vec!["edits".to_string()]), object_fields: None }) {
                                Ok(p) => p,
                                Err(e) => return Ok(Err(e)),
                            };
                            params.dry_run = Some(true);
                            let dry_run_para = match serde_json::to_string(&params) {
                                Ok(d) => d,
                                Err(e) => return Ok(Err(MyError::JsonToStringError{error: e.into()})),
                            };
                            match PARAS.tools.run(name_id[1], &dry_run_para) {
                                Ok(r) => r.0,
                                Err(e) => return Ok(Err(e)),
                            }
                        } else {
                            approval_msg
                        };
                        match ask_approval(uuid, approval_msg, name_id[0] == "edit_file", sender.clone()).await?.as_ref() {
                            "true" => { // 允许
                                match PARAS.tools.run(name_id[1], &i.1) {
                                    Ok(r) => r.0,
                                    Err(e) => return Ok(Err(e)),
                                }
                            },
                            "false" => return Err(MyError::PlanModeError{info: format!("Not allowed to call this tool: {}", name_id[0])}), // 不允许
                            "skip" => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}", name_id[0])}), // 跳过
                            new_prompt => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}\n{}", name_id[0], new_prompt)}), // 跳过的新指示
                        }
                    } else {
                        if step_ask_approval {
                            let approval_msg = if PARAS.english {
                                format!("Do you allow calling the {} tool?{}\n{:?}", name_id[0], i.3.clone().unwrap_or_default(), i.1)
                            } else {
                                format!("是否允许调用 {} 工具？{}\n{:?}", name_id[0], i.3.clone().unwrap_or_default(), i.1)
                            };
                            match ask_approval(uuid, approval_msg, false, sender.clone()).await?.as_ref() {
                                "true" => { // 允许
                                    match PARAS.tools.run(name_id[1], &i.1) {
                                        Ok(r) => r.0,
                                        Err(e) => return Ok(Err(e)),
                                    }
                                },
                                "false" => return Err(MyError::PlanModeError{info: format!("Not allowed to call this tool: {}", name_id[0])}), // 不允许
                                "skip" => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}", name_id[0])}), // 跳过
                                new_prompt => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}\n{}", name_id[0], new_prompt)}), // 跳过的新指示
                            }
                        } else {
                            match PARAS.tools.run(name_id[1], &i.1) {
                                Ok(r) => r.0,
                                Err(e) => return Ok(Err(e)),
                            }
                        }
                    }
                } else if PARAS.mcp_servers.contain_server_id(name_id[1]) {
                    if !PARAS.approval_all && step_ask_approval {
                        let approval_msg = if PARAS.english {
                            format!("Do you allow calling the {} tool?{}\n{:?}", name_id[0], i.3.clone().unwrap_or_default(), i.1)
                        } else {
                            format!("是否允许调用 {} 工具？{}\n{:?}", name_id[0], i.3.clone().unwrap_or_default(), i.1)
                        };
                        match ask_approval(uuid, approval_msg, false, sender.clone()).await?.as_ref() {
                            "true" => { // 允许
                                match PARAS.mcp_servers.run(&name_id, &i.1).await {
                                    Ok(r) => r.0,
                                    Err(e) => return Ok(Err(e)),
                                }
                            },
                            "false" => return Err(MyError::PlanModeError{info: format!("Not allowed to call this tool: {}", name_id[0])}), // 不允许
                            "skip" => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}", name_id[0])}), // 跳过
                            new_prompt => return Err(MyError::PlanModeError{info: format!("skip, this tool has not been executed: {}\n{}", name_id[0], new_prompt)}), // 跳过的新指示
                        }
                    } else {
                        match PARAS.mcp_servers.run(&name_id, &i.1).await {
                            Ok(r) => r.0,
                            Err(e) => return Ok(Err(e)),
                        }
                    }
                } else {
                    return Err(MyError::ToolNotExistError{id: name_id[1].to_string(), info: "run_tools".to_string()})
                };
                final_result = result;
                break
            }
        },
        CallToolResult::Text(test_result) => { // normal text result, not call tool
            final_result = test_result;
        },
    }
    Ok(Ok(final_result))
}

/// send message to page, insert to main message history
async fn send_and_record_message(uuid: &str, msg: String, step_num: usize, model: &str, sender: Sender<Vec<u8>>, is_diff: bool) -> Result<(), MyError> {
    // 1. send to user page
    let messages_num = get_messages_num(uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
    // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
    if let Err(e) = sender.send(MainData::prepare_sse(uuid, messages_num, msg.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0), None, is_diff)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "step {} channel send error: {:?}", step_num, e);
        return Err(MyError::PlanModeError{info: format!("step {} channel send error: {:?}", step_num, e)})
    }
    // 2. add result to main message history
    let message = ChatMessage::Assistant{
        content: Some(ChatMessageContent::Text(msg)),
        reasoning: None,
        reasoning_content: None,
        refusal: None,
        name: None,
        audio: None,
        tool_calls: None,
    };
    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
    insert_message(uuid, message, None, tmp_time, false, DataType::Normal, None, model, None);

    // 3. page left info
    let meta_data = MetaData::new(uuid.to_string(), None, false);
    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "step {} channel send error: {:?}", step_num, e);
        return Err(MyError::PlanModeError{info: format!("step {} channel send error: {:?}", step_num, e)})
    }
    Ok(())
}

/// ask approval
async fn ask_approval(uuid: &str, msg: String, is_diff: bool, sender: Sender<Vec<u8>>) -> Result<String, MyError> {
    let messages_num = get_messages_num(uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
    if let Err(e) = sender.send(MainData::prepare_sse(uuid, messages_num, "".to_string(), true, false, false, false, false, None, Some(0), Some(msg.replace("\n", "srxtzn")), is_diff)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "ask approval error: {:?}", e);
        return Err(MyError::PlanModeError{info: format!("ask approval error: {:?}", e)})
    }
    let tmp_approved: String; // false: 不允许, true: 允许, skip: 跳过, 其他信息: 跳过的新指示
    update_approval(uuid, None);
    loop {
        if let Some(apv) = approved(uuid) {
            tmp_approved = apv;
            update_approval(uuid, None);
            break
        }
        // sleep 2 seconds
        sleep(Duration::from_secs(2)).await;
    }
    Ok(tmp_approved)
}

/// markdown代码块标注语言，未包含的格式后缀直接转为小写即可
const FILE_EXT_LANGUAGE: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("js", "javascript"),
    ("ts", "typescript"),
    ("rb", "ruby"),
    ("cs", "csharp"),
    ("kt", "kotlin"),
    ("sh", "bash"),
    ("md", "markdown"),
    ("txt", "text"),
];

/// 根据文件后缀获取所属语言
fn get_file_language(file: Option<String>) -> String {
    if let Some(f) = file {
        let path = Path::new(&f);
        match path.extension() {
            Some(ext) => {
                let ext_str = ext.to_string_lossy().to_lowercase();
                FILE_EXT_LANGUAGE
                    .iter()
                    .find(|(k, _)| k == &ext_str)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| ext_str)
            }
            None => "text".to_string(),
        }
    } else {
        "text".to_string()
    }
}

/// tool名称加了`_uuid第一部分`保证不重复，但是上下文很长时可能导致模型返回的要调用的模型丢失了`_uuid第一部分`，导致调用模型失败
/// 这里把不含`_uuid第一部分`的tool名称作为key，将对应的完整tool名称作为value，存入HashMap，当`_uuid第一部分`丢失，也能确认具体要调用的模型
fn get_tool_hashmap(tools: &Vec<ChatCompletionTool>) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for t in tools {
        let real_name = t.function.name.clone();
        let name_prefix = real_name.split("__").next().unwrap().to_string();
        out.entry(name_prefix)
            .or_insert_with(Vec::new)
            .push(real_name);
    }
    out
}

// 调用 tool 时，模型返回的 tool 的 json 参数可能类型不对
// 比如 numbers 需要是 list: `{"numnbers": [1, 2, 3]}`，但模型返回的是字符串: `{"numbers": "[1, 2, 3]"}`，导致解析为结构体时失败
// 不要去修改 json 字符串，先转为 Value 再去修改指定字段

/// 指定哪些字段允许被纠正类型
/// 例如模型可能把数组字段返回成字符串：`"numbers": "[1, 2, 3]"`
/// 这时可以把 `numbers` 放进 `array_fields`，函数会只修这个字段
#[derive(Debug, Default)]
pub struct ArgFixSpec {
    /// 这些字段期望是 array，比如 Vec<T>
    pub array_fields: Option<Vec<String>>,

    /// 这些字段期望是 object，比如某个 struct/map
    pub object_fields: Option<Vec<String>>,
}

/// 解析模型返回的 tool arguments
/// 支持两类兼容修复：
/// 1. arguments 整体被包成 JSON string
/// 2. 某些白名单字段被错误地包成 string，例如 "[1,2,3]"
pub fn parse_tool_args<T>(raw: &str, fix_spec: ArgFixSpec) -> Result<T, MyError>
where
    T: DeserializeOwned,
{
    // 第一步：把模型返回的 arguments 解析成 JSON Value
    let mut value: Value = serde_json::from_str(raw).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

    // 第二步：兼容 arguments 整体被多包了一层字符串的情况
    // 有些接口/模型可能返回：
    // "{\"file_path\":\"Cargo.toml.txt\",\"content\":\"...\"}"
    // 外层解析后是 Value::String，需要再解析一次
    if let Value::String(inner_json) = value {
        value = serde_json::from_str(&inner_json).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
    }

    // 第三步：只修复明确声明的字段
    // 例如字段 numbers 期望是数组，但模型给了字符串：
    // {"numbers":"[1,2,3]"}
    // 修复后变成：
    // {"numbers":[1,2,3]}
    if let Value::Object(map) = &mut value {
        // 把 array 从多封装了一层的字符串中解析出来
        if let Some(array_fields) = fix_spec.array_fields {
            for field in array_fields {
                fix_string_field_to_json_type(map, &field, |v| v.is_array());
            }
        }

        // 把 object 从多封装了一层的字符串中解析出来
        if let Some(object_fields) = fix_spec.object_fields {
            for field in object_fields {
                fix_string_field_to_json_type(map, &field, |v| v.is_object());
            }
        }
    }

    // 第四步：把修复后的 JSON Value 反序列化成结构体
    // 如果字段类型仍然不匹配，这里会正常报错
    serde_json::from_value::<T>(value).map_err(|e| MyError::StructToJsonValueError{error: e})
}

/// 如果某个字段现在是 string，并且这个 string 本身又是合法 JSON，
/// 就尝试把它解析成真正的 JSON array/object
/// `accept` 用来限制只接受目标类型：
/// array 字段只接受 array，object 字段只接受 object
fn fix_string_field_to_json_type(
    map: &mut serde_json::Map<String, Value>,
    field: &str,
    accept: impl Fn(&Value) -> bool,
) {
    // 提取 field 对应的字符串
    let Some(Value::String(s)) = map.get(field) else {
        return;
    };

    // 将提取的字符串转为 Value
    let Ok(parsed) = serde_json::from_str::<Value>(s) else {
        return;
    };

    // 如果解析得到了期望的类型则原位更新
    if accept(&parsed) {
        map.insert(field.to_string(), parsed);
    }
}
