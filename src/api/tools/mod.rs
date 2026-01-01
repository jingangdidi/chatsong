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
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::Sender;
use tracing::{event, Level};

use crate::{
    info::{
        insert_message,
        get_messages,
        get_messages_num,
        DataType,
    },
    openai::{
        for_tool::{
            call_tool_not_use_stream,
            CallToolResult,
        },
        for_chat::not_use_stream,
    },
    parse_paras::PARAS,
    error::MyError,
    api::handlers::chat::{
        MainData,
        MetaData,
    },
};

pub mod built_in_tools;
pub mod external_tools;

use built_in_tools::{BuiltInTools, Group};
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
    fn run(&self, id: &str, args: &str) -> Result<String, MyError>;

    /// get all selected tools name (name format: `name__id`, max name length is 26), description and schema
    fn get_desc_and_schema(&self, selected_tools: Vec<String>) -> Vec<(String, String, Value)>;

    /// select all tools, return uuid vector
    fn select_all_tools(&self) -> Vec<String>;
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
    pub fn run(&self, id: &str, args: &str) -> Result<String, MyError> {
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
}

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
pub async fn run_tools(selected_tools: Option<SelectedTools>, uuid: String, sender: Sender<Vec<u8>>, client: Client, mut para_builder: ChatCompletionParametersBuilder, model: &str) -> Result<(), MyError> {
    // get built-in and external tools schame
    let mut tool_schema = PARAS.tools.get_desc_and_schema(&selected_tools)?;
    // get mcp tools schema
    let mcp_schema = PARAS.mcp_servers.get_desc_and_schema(&selected_tools).await?;
    // prepare buider for call tool
    tool_schema.extend(mcp_schema);
    para_builder.tools(tool_schema);
    let mut history_messages: Vec<ChatMessage> = get_messages(&uuid); // store all messages as context log, this is temp history, will not add to the user's main history
    let mut count = 0; // limit loop

    loop {
        // send query to LLM
        para_builder.messages(history_messages.clone());
        let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
        let answer = call_tool_not_use_stream(&uuid, client.clone(), parameters).await?;
        // if answer is call tool result, continue; else break
        match answer {
            CallToolResult::CallTool((raw_message, call_tool_result)) => { // (ChatMessage, Vec<(tool name, tool args, call tool id, content)>)
                for (i, j) in call_tool_result.into_iter().enumerate() {
                    // call tool
                    let name_id: Vec<&str> = j.0.split("__").collect();
                    let (result, tool_name) = if PARAS.tools.contain_tool_id(name_id[1]) {
                        (PARAS.tools.run(name_id[1], &j.1)?, name_id[0].to_string())
                    } else if PARAS.mcp_servers.contain_server_id(name_id[1]) {
                        (PARAS.mcp_servers.run(&name_id, &j.1).await?, name_id[0].to_string())
                    } else {
                        return Err(MyError::ToolNotExistError{id: name_id[1].to_string(), info: "run_tools".to_string()})
                    };

                    // 1. send call tool result to user page
                    let messages_num = get_messages_num(&uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
                    // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
                    let test_result = if let Some(content) = &j.3 {
                        if content.is_empty() {
                            format!("## 📌 {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", i+1, tool_name, &j.1, result)
                        } else {
                            format!("## 📌 {} call tool\n\n---\n\n{}\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", i+1, content, tool_name, &j.1, result)
                        }
                    } else {
                        format!("## 📌 {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", i+1, tool_name, &j.1, result)
                    };
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, test_result.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "channel send error: {:?}", e);
                        break
                    }

                    // 2. add result to main message history
                    let message = ChatMessage::Assistant{
                        content: Some(ChatMessageContent::Text(test_result)),
                        reasoning_content: None,
                        refusal: None,
                        name: None,
                        audio: None,
                        tool_calls: None,
                    };
                    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
                    insert_message(&uuid, message, None, tmp_time, false, DataType::Normal, None, model, None);

                    // 3. 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                    let meta_data = MetaData::new(uuid.clone(), None);
                    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "channel send error: {:?}", e);
                        break
                    }

                    // add each tool call result to current message history
                    history_messages.push(raw_message.clone());
                    history_messages.push(ChatMessage::Tool{content: result, tool_call_id: j.2});
                }
            },
            CallToolResult::Text(test_result) => { // normal text result, not call tool
                // 1. send to user page
                let messages_num = get_messages_num(&uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
                // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
                if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, test_result.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                    event!(Level::WARN, "channel send error: {:?}", e);
                }
                // 2. add result to main message history
                let message = ChatMessage::Assistant{
                    content: Some(ChatMessageContent::Text(test_result)),
                    reasoning_content: None,
                    refusal: None,
                    name: None,
                    audio: None,
                    tool_calls: None,
                };
                let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
                insert_message(&uuid, message.clone(), None, tmp_time, false, DataType::Normal, None, model, None);

                break
            },
        }
        count += 1;
        if count > 10 {
            event!(Level::WARN, "{} already call tool {} times, stop this loop", uuid, count);
            break
        }
    }
    // page left info
    let meta_data = MetaData::new(uuid.clone(), None);
    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "channel send error: {:?}", e);
    }
    Ok(())
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
            "result": "Successfully read 'data.csv', it contains headers and 100 rows of data.",
        },
        {
            "description": "nalyze the structure of the data to confirm it's tab-separated.",
            "status": "completed",
            "tool_name": null,
            "result": "The file 'data.csv' uses a comma as a delimiter, not a tab. The plan will be updated.",
        },
        {
            "description": "Process the data using a comma delimiter and generate a summary.",
            "status": "in_progress",
            "tool_name": "data_analyzer",
            "result": "",
        },
        {
            "description": "Write the generated summary to a new file named 'summary.txt'.",
            "status": "pending",
            "tool_name": "write_file",
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
  3. **The "Missing Tool" Protocol**: If a necessary step requires an External Action (e.g., "save to file") but no corresponding tool exists in the provided list:
     - You **MUST NOT** pretend you can do it.
     - You **MUST NOT** assign `tool_name: null` to that step.
     - You **MUST** immediately abort the entire planning process and return the **Error JSON**.
  4. **Logical Sequence**: The order of steps must respect dependencies.
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
  - Each step should contains 3 properties:
    - description: clear purpose of this step.
    - status: progress status, include `in_progress`, `pending`, `completed`, `update_plan`, `failed`.
    - tool_name: specify the Tool name precisely if need use Tools.
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
        **Condition**: The `result` contains a successful, valid output. For tool calls, this means the tool returned a meaningful result without errors. For non-tool steps, it means the step's objective was met. The plan's logic still holds.
        **Action**:
            1.  Change the `status` of the current step from `"in_progress"` to `"completed"`.
            2.  Find the **next** step in the array (the one immediately following) whose `status` is `"pending"`.
            3.  Change that next step's `status` to `"in_progress"`.
            4.  Keep `all_steps_completed` as `false`.
            5.  Keep `final_result` as `""`.

    - **Scenario B: Update the Plan**
        **Condition**: The `result` indicates an error, failure, or unexpected outcome. This could be a tool error message, an empty result where data was expected, or information that reveals the original plan is flawed, incomplete, or impossible to proceed with.
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
    description: String,
    status:      Status,
    tool_name:   Option<String>,
    result:      String,
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
        match serde_json::from_str(s) {
            Ok(p) => Ok(p),
            Err(e) => {
                event!(Level::ERROR, "json string to Plan struct: {}", s);
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
    let tool_schema_json = serde_json::to_string(&tool_schema).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
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
            send_and_record_message(&uuid, msg, 0, model, sender.clone()).await?;
            first_step = false;
        }
        if !plan_struct.error_msg.is_empty() {
            let msg = format!("## 🤔 error\n\n---\n\n{}", plan_struct.error_msg);
            send_and_record_message(&uuid, msg, plan_struct.steps.len()+1, model, sender.clone()).await?;
            break
        } else if plan_struct.all_steps_completed {
            let msg = format!("## 📌 final result\n\n---\n\n{}", plan_struct.final_result);
            send_and_record_message(&uuid, msg, plan_struct.steps.len()+1, model, sender.clone()).await?;
            break
        } else if plan_struct.steps.iter().any(|s| s.status == Status::InProgress || s.status == Status::UpdatePlan) {
            for i in 0..plan_struct.steps.len() {
                match &plan_struct.steps[i].status {
                    Status::Pending | Status::Completed | Status::Failed => (),
                    Status::InProgress => {
                        if let Some(t) = &plan_struct.steps[i].tool_name {
                            let step_tools = match tool_schema.iter().find(|item| &item.function.name == t) {
                                Some(s) => vec![s.clone()],
                                None => {
                                    event!(Level::ERROR, "step tool_name not correct: {}", t);
                                    return Err(MyError::PlanModeError{info: format!("step tool_name not correct: {}", t)})
                                },
                            };
                            let step_messages = vec![
                                ChatMessage::User{
                                    content: ChatMessageContent::Text(format!("{}current step subtask: {}", plan_struct.format_steps(i), plan_struct.steps[i].description)),
                                    name: None,
                                }
                            ];
                            plan_struct.steps[i].result = function_calling(
                                i+1,
                                step_tools,
                                step_messages,
                                &uuid,
                                client.clone(),
                                para_builder.clone(),
                                //sender.clone(),
                                //model,
                            ).await?;
                        }
                        plan_string = serde_json::to_string(&plan_struct).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
                        plan_string = make_decision(uuid.clone(), client.clone(), para_builder.clone(), model, history_messages.clone(), &plan_string).await?;
                        let mut plan_struct_new = Plan::from_str(&plan_string)?;
                        if plan_struct_new.steps[i].result.is_empty() {
                            plan_struct_new.steps[i].status = Status::UpdatePlan;
                            plan_string = serde_json::to_string(&plan_struct_new).map_err(|e| MyError::JsonToStringError{error: e.into()})?;
                        } else {
                            let msg = format!("## 📌 step {}\n\n---\n\n### 📝 description\n{}\n\n### ✨ result\n{}", i+1, plan_struct_new.steps[i].description, plan_struct_new.steps[i].result);
                            send_and_record_message(&uuid, msg, i+1, model, sender.clone()).await?;
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
                            send_and_record_message(&uuid, msg, i+1, model, sender.clone()).await?;
                            max_update_plan += 1;
                        }
                        break
                    },
                }
            }
        } else {
            event!(Level::ERROR, "if not all steps completed, must contains \"in_progress\" or \"update_plan\"");
            return Err(MyError::PlanModeError{info: format!("if not all steps completed, must contains \"in_progress\" or \"update_plan\"\n{}", plan_string)})
        }
    }
    Ok(())
}

/// call LLM
async fn call_llm(messages: Vec<ChatMessage>, uuid: String, client: Client, mut para_builder: ChatCompletionParametersBuilder, model: &str) -> Result<String, MyError> {
    para_builder.messages(messages);
    let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
    not_use_stream(uuid, client, parameters, model, false, false).await
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
    step_num: usize,
    step_tools: Vec<ChatCompletionTool>,
    mut step_messages: Vec<ChatMessage>,
    uuid: &str,
    client: Client,
    mut para_builder: ChatCompletionParametersBuilder,
    //sender: Sender<Vec<u8>>,
    //model: &str
) -> Result<String, MyError> {
    let mut final_result = "".to_string();
    let mut count = 0; // limit loop
    para_builder.tools(step_tools);
    loop {
        // send query to LLM
        para_builder.messages(step_messages.clone());
        let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
        let answer = call_tool_not_use_stream(uuid, client.clone(), parameters).await?;
        // if answer is call tool result, continue; else break
        match answer {
            CallToolResult::CallTool((raw_message, call_tool_result)) => { // (ChatMessage, Vec<(tool name, tool args, call tool id, content)>)
                for i in call_tool_result {
                    // call tool
                    let name_id: Vec<&str> = i.0.split("__").collect();
                    let (result, _tool_name) = if PARAS.tools.contain_tool_id(name_id[1]) {
                        (PARAS.tools.run(name_id[1], &i.1)?, name_id[0].to_string())
                    } else if PARAS.mcp_servers.contain_server_id(name_id[1]) {
                        (PARAS.mcp_servers.run(&name_id, &i.1).await?, name_id[0].to_string())
                    } else {
                        return Err(MyError::ToolNotExistError{id: name_id[1].to_string(), info: "run_tools".to_string()})
                    };

                    /*
                    // 1. send call tool result to user page
                    let messages_num = get_messages_num(uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
                    // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
                    let test_result = if let Some(content) = &i.3 {
                        if content.is_empty() {
                            format!("## ⏳ step {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", step_num, tool_name, &i.1, result)
                        } else {
                            format!("## ⏳ step {} call tool\n\n---\n\n{}\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", step_num, content, tool_name, &i.1, result)
                        }
                    } else {
                        format!("## ⏳ step {} call tool\n\n---\n\n### 🛠 run tool\n{}({})\n\n### 💡 result\n{}", step_num, tool_name, &i.1, result)
                    };
                    if let Err(e) = sender.send(MainData::prepare_sse(uuid, messages_num, test_result.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "step {} channel send error: {:?}", step_num, e);
                        break
                    }

                    // 2. add result to main message history
                    let message = ChatMessage::Assistant{
                        content: Some(ChatMessageContent::Text(test_result)),
                        reasoning_content: None,
                        refusal: None,
                        name: None,
                        audio: None,
                        tool_calls: None,
                    };
                    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
                    insert_message(uuid, message, None, tmp_time, false, DataType::Normal, None, model, None);
                    */

                    /*
                    let mut meta_data = MetaData::new(uuid.to_string(), None);
                    meta_data.update_token(*total_in_out_tokens);
                    if let Err(e) = sender.send(meta_data.prepare_sse(uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        event!(Level::WARN, "step {} channel send error: {:?}", step_num, e);
                        break
                    }
                    */

                    // add each tool call result to current message history
                    step_messages.push(raw_message.clone());
                    step_messages.push(ChatMessage::Tool{content: result, tool_call_id: i.2});
                }
            },
            CallToolResult::Text(test_result) => { // normal text result, not call tool
                final_result = test_result;
                break
            },
        }
        count += 1;
        if count > 10 {
            event!(Level::WARN, "{} step {} already call tool {} times, stop this loop", step_num, uuid, count);
            break
        }
    }
    Ok(final_result)
}

/// send message to page, insert to main message history
async fn send_and_record_message(uuid: &str, msg: String, step_num: usize, model: &str, sender: Sender<Vec<u8>>) -> Result<(), MyError> {
    // 1. send to user page
    let messages_num = get_messages_num(uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
    // uuid, id, content, is_left, is_img, is_voice, is_history, is_web, time_model, current_token
    if let Err(e) = sender.send(MainData::prepare_sse(uuid, messages_num, msg.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "step {} channel send error: {:?}", step_num, e);
        return Err(MyError::PlanModeError{info: format!("step {} channel send error: {:?}", step_num, e)})
    }
    // 2. add result to main message history
    let message = ChatMessage::Assistant{
        content: Some(ChatMessageContent::Text(msg)),
        reasoning_content: None,
        refusal: None,
        name: None,
        audio: None,
        tool_calls: None,
    };
    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
    insert_message(uuid, message, None, tmp_time, false, DataType::Normal, None, model, None);

    // 3. page left info
    let meta_data = MetaData::new(uuid.to_string(), None);
    if let Err(e) = sender.send(meta_data.prepare_sse(&uuid)?).await { // 传递数据以`data: `起始，以`\n\n`终止
        event!(Level::WARN, "step {} channel send error: {:?}", step_num, e);
        return Err(MyError::PlanModeError{info: format!("step {} channel send error: {:?}", step_num, e)})
    }
    Ok(())
}
