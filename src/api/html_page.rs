use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::{
    info::{
        get_log_for_display, // 获取指定uuid最新问答记录，提取字符串，用于在chat页面显示
        get_token, // 获取指定uuid问题和答案的总token数
        get_prompt_name, // 获取当前uuid的prompt名称
        //pop_message_before_end, // 在保存指定uuid的chat记录之前，先去指定uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
        DisplayInfo, // 将之前问答记录显示到页面
        is_incognito, // 是否无痕模式
    },
    graph::get_all_related_uuid, // 获取与指定uuid相关的所有uuid
    parse_paras::PARAS, // 存储命令行参数的全局变量
};

/// 将svg图片编码为base64使用，注意要加上“data:image/svg+xml;base64,”前缀，notepad++设置编码为“以UTF-8无BOM格式编码”
/// https://base64.run/
const ICON_SHORTCUT: &str = include_str!("../../assets/image/robot-7.txt");
const ICON_USER: &str = include_str!("../../assets/image/user-icon-1.txt");
const ICON_CHATGPT: &str = include_str!("../../assets/image/robot-1.txt");
const ICON_DOWNLOAD: &str = include_str!("../../assets/image/download-square-svgrepo-com.txt");
const ICON_UPLOAD: &str = include_str!("../../assets/image/attachment-2-svgrepo-com.txt");
const ICON_HELP: &str = include_str!("../../assets/image/help-svgrepo-com.txt");
const ICON_SEND: &str = include_str!("../../assets/image/icon_send.txt");
const ICON_STOP: &str = include_str!("../../assets/image/stop-circle-svgrepo-com-3.txt");
const ICON_SETTING: &str = include_str!("../../assets/image/setting.txt");
//const ICON_DELETE: &str = include_str!("../../assets/image/delete.txt");
const ICON_DELETE: &str = include_str!("../../assets/image/delete-svgrepo-com.svg");
const ICON_INCOGNITO1: &str = include_str!("../../assets/image/incognito-svgrepo-com-1.txt");
const ICON_INCOGNITO2: &str = include_str!("../../assets/image/incognito-svgrepo-com-2.txt");
const ICON_COMPRESS: &str = include_str!("../../assets/image/format-space-less-svgrepo-com.txt");

/// 将marked.min.js下载下来，不需要每次联网加载
const MARKED_MIN_JS: &str = include_str!("../../assets/js/marked.min.js");

/// 将PrismJS代码高亮下载下来，不需要每次联网加载
const PRISM_MIN_JS: &str = include_str!("../../assets/js/Prism_min.js");
const PRISM_MIN_CSS: &str = include_str!("../../assets/css/Prism_min.css");

/// chat页面ch和en共用的css代码
const CSS_CODE: &str = include_str!("../../assets/css/style.css");

/// 下载页面用的css代码
const CSS_CODE_DOWNLOAD: &str = include_str!("../../assets/css/style_for_download.css");

/// diff2html generates pretty HTML diffs from git diff or unified diff output
const DIFF2HTML_JS: &str = include_str!("../../assets/js/diff2html.min.js");
const DIFF2HTML_CSS: &str = include_str!("../../assets/css/diff2html.min.css");

/// 页面显示的信息，true是英文，false是中文，创建页面时填充进去
static PAGE: Lazy<RwLock<HashMap<bool, PageInfo>>> = Lazy::new(|| RwLock::new(HashMap::from([(true, PageInfo::new(true)), (false, PageInfo::new(false))])));

/// 页面左侧参数信息
struct LeftInfo {
    label:       String,                                // 参数名称
    title:       String,                                // hover时显示的提示信息
    disabled:    Option<String>,                        // 下拉第一项显示的信息
    option:      Option<Vec<(String, Option<String>)>>, // 下拉选项信息，以及对应的hover显示的title
    placeholder: Option<String>,                        // 输入框内提示信息
}

/// 页面语言，中文或英文
struct PageInfo {
    prompt:       LeftInfo,    // 指定prompt开启新对话
    name:         LeftInfo,    // 可选填的新对话名称
    tool:         LeftInfo,    // call tools
    plan_mode:    LeftInfo,    // plan mode
    model:        LeftInfo,    // 选择要用的模型
    message:      LeftInfo,    // 上下文消息数
    web:          LeftInfo,    // 网络搜索
    prompt_name:  LeftInfo,    // 当前prompt名称
    uuid_current: LeftInfo,    // 当前uuid
    input:        LeftInfo,    // 输入的总token数
    output:       LeftInfo,    // 输出的总token数
    context_len:  LeftInfo,    // context tokens
    cot:          LeftInfo,    // 思考的深度
    uuid_input:   LeftInfo,    // 要跳转的uuid
    uuid_drop:    LeftInfo,    // 下拉相关uuid
    temp:         LeftInfo,    // 温度
    top_p:        LeftInfo,    // top-p
    stream:       LeftInfo,    // 流式输出
    voice:        LeftInfo,    // 声音
    copy:         String,      // 点击头像复制
    delete:       [String; 2], // 删除问题和回答
    m_qa_token:   [String; 4], // 显示信息数、Q&A对数、token数，4部分，用具体数值拼接
    upload:       String,      // 上传文件的title
    textarea:     String,      // 输入框内的提示信息
    button:       [String; 4], // 左下角设置、下载、使用说明、压缩总结这4个按钮的title
    incognito:    [String; 3], // 左下角无痕模式按钮开启和关闭2个状态的title，以及开启的前2个字符
    wait:         [String; 3], // 发送问题后等待时，输入框内显示的内容：等待回答、等待搜索、发送问题
}

impl PageInfo {
    fn new(is_en: bool) -> Self {
        if is_en {
            PageInfo {
                prompt: LeftInfo{ // 指定prompt开启新对话
                    label:       "start new chat".to_string(),
                    title:       "Select a &quot;Prompt&quot; to initiate a new conversation; choose &quot;keep current chat&quot; to continue with the existing dialogue without starting afresh".to_string(),
                    disabled:    Some("select prompt".to_string()),
                    option:      Some(vec![("keep current chat".to_string(), None), ("no prompt".to_string(), None)]),
                    placeholder: None,
                },
                name: LeftInfo{ // 可选填的新对话名称
                    label:       "current chat name (optional)".to_string(),
                    title:       "Feel free to designate a specific name for current conversation, facilitating easier selection within the &quot;Related UUIDs&quot; section".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("chat name (optional)".to_string()),
                },
                tool: LeftInfo{ // call tools
                    label:       "call tools".to_string(),
                    title:       "Choose one or more tools to solve complex problems. When using tools, a plan will be created first, and then implemented item by item. After each execution is completed, it will be judged whether the plan needs to be updated, and finally the final result will be returned. ⚪ not using any tools, 🔴 select all tools, 🟢 select built-in tools, 🟣 select all custom external tools, 🟡 select MCP tools, while other options indicate the selection of a single tool".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                plan_mode: LeftInfo{ // plan mode
                    label:       "plan mode".to_string(),
                    title:       "Effective when invoking &quot;call tools&quot;, the planning mode is activated to first devise a strategy, breaking down the problem into multiple sub-tasks, which are then addressed sequentially—ideal for handling complex tasks.".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                model: LeftInfo{ // 选择要用的模型
                    label:       "models".to_string(),
                    title:       "Currently supported models, permit the use of varying models within the same conversation for inquiries".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                message: LeftInfo{ // 上下文消息数
                    label:       "contextual messages".to_string(),
                    title:       "Opting to include the maximum number of Q&A pairs or messages in each inquiry can conserve tokens".to_string(),
                    disabled:    Some("select number".to_string()),
                    option:      Some(vec![("unlimit".to_string(), None), ("1 Q&A pair".to_string(), None), ("2 Q&A pairs".to_string(), None), ("3 Q&A pairs".to_string(), None), ("4 Q&A pairs".to_string(), None), ("5 Q&A pairs".to_string(), None), ("prompt + 1 Q&A pair".to_string(), None), ("prompt + 2 Q&A pairs".to_string(), None), ("prompt + 3 Q&A pairs".to_string(), None), ("prompt + 4 Q&A pairs".to_string(), None), ("prompt + 5 Q&A pairs".to_string(), None), ("1 message".to_string(), None), ("2 messages".to_string(), None), ("3 messages".to_string(), None), ("4 messages".to_string(), None), ("5 messages".to_string(), None), ("prompt + 1 message".to_string(), None), ("prompt + 2 messages".to_string(), None), ("prompt + 3 messages".to_string(), None), ("prompt + 4 messages".to_string(), None), ("prompt + 5 messages".to_string(), None)]),
                    placeholder: None,
                },
                web: LeftInfo{ // 网络搜索
                    label:       "web search".to_string(),
                    title:       "Conduct online research based on the proposed query and respond accordingly; alternatively, analyze the specified URL and provide answers derived from the parsed results".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                prompt_name: LeftInfo{ // 当前prompt名称
                    label:       "current prompt".to_string(),
                    title:       "current prompt".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                uuid_current: LeftInfo{ // 当前uuid
                    label:       "current uuid".to_string(),
                    title:       "current uuid，remember this UUID, you may revisit and inquire about it at any time thereafter".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                input: LeftInfo{ // 输入的总token数
                    label:       "total input tokens".to_string(),
                    title:       "The total input tokens used in the current conversation".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                output: LeftInfo{ // 输出的总token数
                    label:       "total output tokens".to_string(),
                    title:       "The total output tokens used in the current conversation".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                context_len: LeftInfo{ // context tokens
                    label:       "context usage".to_string(),
                    title:       "The total number of tokens of prompt_tokens and completion_tokens in the last request, used to evaluate context usage, note that it will only be updated after each request".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                cot: LeftInfo{ // 思考的深度
                    label:       "reasoning effort".to_string(),
                    title:       "effort on reasoning for reasoning models and the visibility of the reasoning process, applicable solely to the reasoning models".to_string(),
                    disabled:    Some("select effort".to_string()),
                    option:      Some(vec![
                        ("Display the reasoning process".to_string(), Some("favors speed and economical token usage".to_string())),
                        ("Hide the reasoning process".to_string(), Some("favors speed and economical token usage".to_string())),
                        ("Display the reasoning process".to_string(), Some("a balance between speed and reasoning accuracy".to_string())),
                        ("Hide the reasoning process".to_string(), Some("a balance between speed and reasoning accuracy".to_string())),
                        ("Display the reasoning process".to_string(), Some("favors more complete reasoning".to_string())),
                        ("Hide the reasoning process".to_string(), Some("favors more complete reasoning".to_string())),
                    ]),
                    placeholder: None,
                },
                uuid_input: LeftInfo{ // 要跳转的uuid
                    label:       "uuid".to_string(),
                    title:       "input the UUID of the previous conversation to review its content and to proceed with your inquiry".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("uuid for log".to_string()),
                },
                uuid_drop: LeftInfo{ // 下拉相关uuid
                    label:       "related UUIDs".to_string(),
                    title:       "implement seamless transitions and reuse across related conversations, enabling fluid navigation between distinct dialogues".to_string(),
                    disabled:    Some("select uuid".to_string()),
                    option:      None,
                    placeholder: None,
                },
                temp: LeftInfo{ // 温度
                    label:       "temperature".to_string(),
                    title:       "What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("temperature".to_string()),
                },
                top_p: LeftInfo{ // top-p
                    label:       "top-p".to_string(),
                    title:       "An alternative to sampling with temperature, called nucleus sampling, where the model considers the results of the tokens with top_p probability mass. So 0.1 means only the tokens comprising the top 10% probability mass are considered".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("top-p".to_string()),
                },
                stream: LeftInfo{ // 流式输出
                    label:       "stream".to_string(),
                    title:       "partial messages will be sent, like in ChatGPT".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                voice: LeftInfo{ // 声音
                    label:       "voice".to_string(),
                    title:       "Select the timbre for the generated speech".to_string(),
                    disabled:    Some("select speech voice".to_string()),
                    option:      Some(vec![("Alloy".to_string(), None), ("Echo".to_string(), None), ("Fable".to_string(), None), ("Onyx".to_string(), None), ("Nova".to_string(), None), ("Shimmer".to_string(), None)]),
                    placeholder: None,
                },
                copy:       "click to copy".to_string(), // 点击头像复制
                delete:     ["delete this question".to_string(), "delete this answer".to_string()], // 删除问题和回答
                m_qa_token: ["message ".to_string(), ", Q&A pair ".to_string(), ", ".to_string(), " tokens".to_string()], // 显示信息数、Q&A对数、token数，4部分，用具体数值拼接
                upload:     "upload files".to_string(), // 上传文件的title
                textarea:   "Input your query (Press Shift+Enter for line breaks)".to_string(), // 输入框内的提示信息
                button:     ["switch parameter bar settings".to_string(), "save current chat log".to_string(), "usage".to_string(), "Summarize and compress message records within the specified range of context messages for the current conversation".to_string()], // 左下角设置、下载、使用说明、压缩总结这4个按钮的title
                incognito:  ["Activate incognito mode, where the current conversation will not be locally preserved upon program termination and shall be irrevocably discarded, refreshing or reopening the current page will also erase the conversation history".to_string(), "Disable the incognito mode, and your current conversation will be preserved locally upon exiting the application, allowing you to resume seamlessly during your next session".to_string(), "Ac".to_string()], // 左下角无痕模式按钮开启和关闭2个状态的title，以及开启的前2个字符
                wait:       ["Waiting for answer".to_string(), "Waiting for search".to_string(), "Sending query".to_string()], // 发送问题后等待时，输入框内显示的内容：等待回答、等待搜索、发送问题
            }
        } else {
            PageInfo {
                prompt: LeftInfo{ // 指定prompt开启新对话
                    label:       "开启新对话".to_string(),
                    title:       "选择prompt开启新对话，“保持当前会话”表示不开启新对话，基于当前对话继续提问".to_string(),
                    disabled:    Some("选择开启新会话的prompt".to_string()),
                    option:      Some(vec![("保持当前对话".to_string(), None), ("无prompt".to_string(), None)]),
                    placeholder: None,
                },
                name: LeftInfo{ // 可选填的新对话名称
                    label:       "当前对话名称（可选）".to_string(),
                    title:       "可以给当前对话指定一个名称，这样在“相关uuid”中方便选择".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("chat name (optional)".to_string()),
                },
                tool: LeftInfo{ // call tools
                    label:       "调用工具".to_string(),
                    title:       "选择一个或多个工具解决复杂问题。使用工具时会先制定计划，然后逐条实现，并在每条执行结束后判断是否需要更新计划，最后返回最终结果。⚪表示不使用任何工具，🔴表示选择所有工具，🟢表示选择内置工具，🟣表示选择所有自定义的外部工具，🟡表示选择MCP工具，其他选项表示单选一个工具".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                plan_mode: LeftInfo{ // plan mode
                    label:       "计划模式".to_string(),
                    title:       "调用工具时有效，开启计划模式时，会先制定计划，将问题拆分为多个子问题，然后逐个完成，适用于复杂任务".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                model: LeftInfo{ // 选择要用的模型
                    label:       "模型".to_string(),
                    title:       "当前支持的模型，同一个对话可以使用不同模型进行提问".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                message: LeftInfo{ // 上下文消息数
                    label:       "上下文消息数".to_string(),
                    title:       "选择每次提问包含的最多问答对或消息数量，可以节省token".to_string(),
                    disabled:    Some("选择数量".to_string()),
                    option:      Some(vec![("不限制".to_string(), None), ("1对Q&A".to_string(), None), ("2对Q&A".to_string(), None), ("3对Q&A".to_string(), None), ("4对Q&A".to_string(), None), ("5对Q&A".to_string(), None), ("prompt + 1对Q&A".to_string(), None), ("prompt + 2对Q&A".to_string(), None), ("prompt + 3对Q&A".to_string(), None), ("prompt + 4对Q&A".to_string(), None), ("prompt + 5对Q&A".to_string(), None), ("1条信息".to_string(), None), ("2条信息".to_string(), None), ("3条信息".to_string(), None), ("4条信息".to_string(), None), ("5条信息".to_string(), None), ("prompt + 1条信息".to_string(), None), ("prompt + 2条信息".to_string(), None), ("prompt + 3条信息".to_string(), None), ("prompt + 4条信息".to_string(), None), ("prompt + 5条信息".to_string(), None)]),
                    placeholder: None,
                },
                web: LeftInfo{ // 网络搜索
                    label:       "网络搜索".to_string(),
                    title:       "使用提出的问题进行网络搜索，然后基于搜索结果进行回答；或解析指定url，然后基于解析结果进行回答".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                prompt_name: LeftInfo{ // 当前prompt名称
                    label:       "当前prompt".to_string(),
                    title:       "当前对话的prompt".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                uuid_current: LeftInfo{ // 当前uuid
                    label:       "当前uuid".to_string(),
                    title:       "当前对话的uuid，记住该uuid，之后可再次查看并提问".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                input: LeftInfo{ // 输入的总token数
                    label:       "输入的总token".to_string(),
                    title:       "当前对话提问的总token".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                output: LeftInfo{ // 输出的总token数
                    label:       "输出的总token".to_string(),
                    title:       "当前对话回答的总token".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                context_len: LeftInfo{ // context tokens
                    label:       "上下文使用量".to_string(),
                    title:       "上次提问发送的总token数+模型回答的token数，用于评估模型上下文使用量，注意每次回答之后才会更新".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                cot: LeftInfo{ // 思考的深度
                    label:       "思考的深度".to_string(),
                    title:       "选择思考的深度和是否显示思考过程，仅对CoT（chain of thought）模型有效".to_string(),
                    disabled:    Some("选择思考的深度".to_string()),
                    option:      Some(vec![
                        ("显示思考过程".to_string(), Some("简单问答，显示思考过程".to_string())),
                        ("不显示思考过程".to_string(), Some("简单问答，不显示思考过程".to_string())),
                        ("显示思考过程".to_string(), Some("多步骤推理，显示思考过程".to_string())),
                        ("不显示思考过程".to_string(), Some("多步骤推理，不显示思考过程".to_string())),
                        ("显示思考过程".to_string(), Some("复杂逻辑推导，显示思考过程".to_string())),
                        ("不显示思考过程".to_string(), Some("复杂逻辑推导，不显示思考过程".to_string())),
                    ]),
                    placeholder: None,
                },
                uuid_input: LeftInfo{ // 要跳转的uuid
                    label:       "uuid".to_string(),
                    title:       "输入对话的uuid，查看对话内容以及继续提问".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("uuid for log".to_string()),
                },
                uuid_drop: LeftInfo{ // 下拉相关uuid
                    label:       "相关uuid".to_string(),
                    title:       "与当前对话直接相关的其他对话，实现不同对话间跳转复用".to_string(),
                    disabled:    Some("选择uuid".to_string()),
                    option:      None,
                    placeholder: None,
                },
                temp: LeftInfo{ // 温度
                    label:       "温度".to_string(),
                    title:       "控制模型生成文本的随机性，取值范围为0~2。温度越高，生成的文本越随机、越发散；温度越低，生成的文本越保守、越集中。即通过调整token生成的概率分布来控制输出的随机性".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("temperature".to_string()),
                },
                top_p: LeftInfo{ // top-p
                    label:       "核采样".to_string(),
                    title:       "控制模型生成文本的随机性，取值范围为0~1。将候选token按照概率从高到低排序，当累积概率超过设定的top-p累积概率阈值时，剩下的候选token将被舍弃，答案将从保留的token中选择。即通过限制模型考虑的token范围来控制输出的随机性".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: Some("top-p".to_string()),
                },
                stream: LeftInfo{ // 流式输出
                    label:       "流式输出".to_string(),
                    title:       "流式输出边生成边显示，否则得到完整答案后一次性显示全部".to_string(),
                    disabled:    None,
                    option:      None,
                    placeholder: None,
                },
                voice: LeftInfo{ // 声音
                    label:       "声音".to_string(),
                    title:       "选择生成speech的音色".to_string(),
                    disabled:    Some("选择speech声音".to_string()),
                    option:      Some(vec![("Alloy".to_string(), None), ("Echo".to_string(), None), ("Fable".to_string(), None), ("Onyx".to_string(), None), ("Nova".to_string(), None), ("Shimmer".to_string(), None)]),
                    placeholder: None,
                },
                copy:       "点击复制".to_string(), // 点击头像复制
                delete:     ["删除该问题".to_string(), "删除该回答".to_string()], // 删除问题和回答
                m_qa_token: ["第".to_string(), "条信息，第".to_string(), "对问答，".to_string(), "个token".to_string()], // 显示信息数、Q&A对数、token数，4部分，用具体数值拼接
                upload:     "上传文件".to_string(), // 上传文件的title
                textarea:   "输入你的问题 (Shift+Enter换行)".to_string(), // 输入框内的提示信息
                button:     ["切换参数栏设置".to_string(), "保存当前对话html页面".to_string(), "查看使用说明".to_string(), "对当前对话指定&quot;上下文消息数&quot;范围内的消息记录进行总结压缩".to_string()], // 左下角设置、下载、使用说明、压缩总结这4个按钮的title
                incognito:  ["开启无痕模式，关闭程序时，当前对话不会被保存在本地，直接舍弃，刷新或重新打开当前页面也将丢弃对话记录".to_string(), "关闭无痕模式，关闭程序时，当前对话会被保存在本地，下次可以接着提问".to_string(), "开启".to_string()], // 左下角无痕模式按钮开启和关闭2个状态的title，以及开启的前2个字符
                wait:       ["等待回答".to_string(), "等待搜索".to_string(), "发送问题".to_string()], // 发送问题后等待时，输入框内显示的内容：等待回答、等待搜索、发送问题
            }
        }
    }
}

/// 生成主页html字符串，css和js都写在html中
/// v: api版本，例如：`/v1`
pub fn create_main_page(uuid: &str, v: String) -> String {
    // 获取当前uuid的问题和答案的总token数
    let token = get_token(uuid);
    // 获取当前uuid的prompt名称
    let prompt_name = get_prompt_name(uuid);
    // 获取与当前uuid相关的所有uuid
    let related_uuid_prompt = get_all_related_uuid(uuid);
    // 是否无痕模型
    let is_incognito = is_incognito(uuid);
    // 页面信息
    let page_data_locked = PAGE.read().unwrap();
    let page_data = page_data_locked.get(&PARAS.english).unwrap();

    // 创建包含css和js，并插入chat记录的html页面
    let mut result = r###"<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="author" content="srx">
    <title>chatsong</title>
    <!-- <link href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/5.13.0/css/all.min.css" rel="stylesheet" /> -->
    <!-- <link href="{{ v }}/templates/css/all.min.css" rel="stylesheet" /> -->
    <!-- <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script> -->
    <!-- <script src="{{ v }}/templates/js/marked.min.js"></script> -->
    <!-- <script srx="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script> -->
    <!-- <script src="{{ v }}/templates/js/highlight.min.js"></script> -->
    <!-- <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/diff2html/bundles/css/diff2html.min.css" /> -->
    <!-- <script src="https://cdn.jsdelivr.net/npm/diff2html/bundles/js/diff2html.min.js"></script> -->
"###.to_string();
    //result += &format!("    <link rel='shortcut icon' href='{}/templates/images/robot-7.svg' type='image/x-icon'>\n", v);
    result += &format!("    <link rel='shortcut icon' href='{}' type='image/x-icon'>\n", ICON_SHORTCUT);
    result += "</head>\n";

    result += "<style type='text/css'>\n";
    result += CSS_CODE;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += PRISM_MIN_CSS;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += DIFF2HTML_CSS;
    result += "</style>\n";

    result += r###"<body>
    <!-- setting -->
    <div id="left-part" class="side-nav">

        <!-- select create a new chat -->
"###;
    let tmp_option = page_data.prompt.option.as_ref().unwrap();
    result += &format!("        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-prompt' class='left_para for_focus' name='prompt'>
                <option disabled>--{}--</option>
                <option value='-1' selected>{}</option>
                <option value='0'>{}</option>\n", page_data.prompt.title, page_data.prompt.label, page_data.prompt.disabled.as_ref().unwrap(), tmp_option[0].0, tmp_option[1].0);
    result += &PARAS.api.pulldown_prompt;
    result += r###"            </select>
        </div>

        <!-- 对话名称 -->
"###;
    result += &format!("        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='input-chat-name' class='left_para' type='text' name='chat-name' placeholder='{}'>
        </div>

        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-tool' class='left_para for_focus' name='tool'>
                {}
                {}
            </select>
        </div>

        <div class='top_add_space switch-toggle' title='{}'>
            <label>{}</label>
            <input id='select-plan' class='left_para for_focus' type='checkbox' name='plan'>
            <label for='select-plan'></label>
        </div>

        <!-- select model -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-model' class='left_para for_focus' name='model'>\n", page_data.name.title, page_data.name.label, page_data.name.placeholder.as_ref().unwrap(), page_data.tool.title, page_data.tool.label, PARAS.tools.html, PARAS.mcp_servers.html, page_data.plan_mode.title, page_data.plan_mode.label, page_data.model.title, page_data.model.label);
    result += &PARAS.api.pulldown_model;
    result += r###"            </select>
        </div>

        <!-- select recent log -->
"###;
    let tmp_option = page_data.message.option.as_ref().unwrap();
    let tmp_option_cot = page_data.cot.option.as_ref().unwrap();
    result += &format!("
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-log-num' class='left_para for_focus' name='num'>
                <option disabled>--{}--</option>
                <option value='unlimit'>{}</option>
                <option value='1qa'>{}</option>
                <option value='2qa'>{}</option>
                <option value='3qa'>{}</option>
                <option value='4qa'>{}</option>
                <option value='5qa'>{}</option>
                <option value='p1qa' selected>{}</option>
                <option value='p2qa'>{}</option>
                <option value='p3qa'>{}</option>
                <option value='p4qa'>{}</option>
                <option value='p5qa'>{}</option>
                <option value='1'>{}</option>
                <option value='2'>{}</option>
                <option value='3'>{}</option>
                <option value='4'>{}</option>
                <option value='5'>{}</option>
                <option value='p1'>{}</option>
                <option value='p2'>{}</option>
                <option value='p3'>{}</option>
                <option value='p4'>{}</option>
                <option value='p5'>{}</option>
            </select>
        </div>

        <div class='top_add_space switch-toggle' title='{}'>
            <label>{}</label>
            <input id='select-web' class='left_para for_focus' type='checkbox' name='web'>
            <label for='select-web'></label>
        </div>

        <!-- show prompt -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='show-prompt' class='left_para'>
        </div>

        <!-- show uuid -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='show-uuid' class='left_para'>
        </div>

        <!-- show input token -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='show-in-token' class='left_para'>
        </div>

        <!-- show output token -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='show-out-token' class='left_para'>
        </div>

        <!-- show context token -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='show-context-token' class='left_para' placeholder='0'>
        </div>
    </div>

    <div id='left-part-other' class='side-nav'>
        <!-- select chain of thought effort -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-effort' class='left_para for_focus' name='effort'>
                <option disabled>--{}--</option>
                <optgroup label='Low'>
                    <option value='1' selected title='{}'>{}</option>
                    <option value='2' title='{}'>{}</option>
                </optgroup>
                <optgroup label='Medium'>
                    <option value='3' title='{}'>{}</option>
                    <option value='4' title='{}'>{}</option>
                </optgroup>
                <optgroup label='High'>
                    <option value='5' title='{}'>{}</option>
                    <option value='6' title='{}'>{}</option>
                </optgroup>
            </select>
        </div>

        <!-- uuid -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='input-uuid' class='left_para' type='text' name='uuid' placeholder='{}'>
        </div>

        <!-- select related uuid -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-related-uuid' class='left_para for_focus' name='related-uuid'>
                <option value='-1' disabled selected>--{}--</option>\n", page_data.message.title, page_data.message.label, page_data.message.disabled.as_ref().unwrap(), tmp_option[0].0, tmp_option[1].0, tmp_option[2].0, tmp_option[3].0, tmp_option[4].0, tmp_option[5].0, tmp_option[6].0, tmp_option[7].0, tmp_option[8].0, tmp_option[9].0, tmp_option[10].0, tmp_option[11].0, tmp_option[12].0, tmp_option[13].0, tmp_option[14].0, tmp_option[15].0, tmp_option[16].0, tmp_option[17].0, tmp_option[18].0, tmp_option[19].0, tmp_option[20].0, page_data.web.title, page_data.web.label, page_data.prompt_name.title, page_data.prompt_name.label, page_data.uuid_current.title, page_data.uuid_current.label, page_data.input.title, page_data.input.label, page_data.output.title, page_data.output.label, page_data.context_len.title, page_data.context_len.label, page_data.cot.title, page_data.cot.label, page_data.cot.disabled.as_ref().unwrap(), tmp_option_cot[0].1.as_ref().unwrap(), tmp_option_cot[0].0, tmp_option_cot[1].1.as_ref().unwrap(), tmp_option_cot[1].0, tmp_option_cot[2].1.as_ref().unwrap(), tmp_option_cot[2].0, tmp_option_cot[3].1.as_ref().unwrap(), tmp_option_cot[3].0, tmp_option_cot[4].1.as_ref().unwrap(), tmp_option_cot[4].0, tmp_option_cot[5].1.as_ref().unwrap(), tmp_option_cot[5].0, page_data.uuid_input.title, page_data.uuid_input.label, page_data.uuid_input.placeholder.as_ref().unwrap(), page_data.uuid_drop.title, page_data.uuid_drop.label, page_data.uuid_drop.disabled.as_ref().unwrap());
    for i in related_uuid_prompt {
        result += &format!("                <option value='{}'>{} ({})</option>\n", i.0, i.0, i.1);
    }
    let tmp_option = page_data.voice.option.as_ref().unwrap();
    result += &format!("            </select>
        </div>

        <!-- temperature -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='input-temperature' class='left_para' type='number' min='0' max='2' name='temperature' placeholder='{}'>
        </div>

        <!-- top-p -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <input id='input-top-p' class='left_para' type='number' min='0' max='1' name='top-p' placeholder='{}'>
        </div>

        <!-- select stream -->
        <!--<div class='top_add_space' title='流式输出边生成边显示，否则得到完整答案后一次性显示全部'>
            <label>流式输出</label>
            <select id='select-stm' class='left_para for_focus' name='stream'>
                <option disabled>--是否流式输出--</option>
                <option value='yes' selected>Yes</option>
                <option value='no'>No</option>
            </select>
        </div>-->

        <div class='top_add_space switch-toggle' title='{}'>
            <label>{}</label>
            <input id='select-stm' class='left_para for_focus' type='checkbox' checked name='stream'>
            <label for='select-stm'></label>
        </div>

        <!-- select voice -->
        <div class='top_add_space' title='{}'>
            <label>{}</label>
            <select id='select-voice' class='left_para for_focus' name='voice'>
                <option disabled>--{}--</option>
                <option value='1' selected>{}</option>
                <option value='2'>{}</option>
                <option value='3'>{}</option>
                <option value='4'>{}</option>
                <option value='5'>{}</option>
                <option value='6'>{}</option>
            </select>
        </div>
    </div>

    <!-- chat part -->
    <div id='right-part' class='content'>
        <!-- chat content region -->
        <div id='scrolldown' class='chat-content-area'>", page_data.temp.title, page_data.temp.label, page_data.temp.placeholder.as_ref().unwrap(), page_data.top_p.title, page_data.top_p.label, page_data.top_p.placeholder.as_ref().unwrap(), page_data.stream.title, page_data.stream.label, page_data.voice.title, page_data.voice.label, page_data.voice.disabled.as_ref().unwrap(), tmp_option[0].0, tmp_option[1].0, tmp_option[2].0, tmp_option[3].0, tmp_option[4].0, tmp_option[5].0);

    let (next_msg_id, m_num, qa_num, logs) = get_log_for_display(uuid, true); // cookie对应的chat记录
    for log in logs.iter() {
        if log.is_query { // 用户输入的问题
            let tmp_title = if log.token > 0 {
                format!("{}{}{}{}{}{}{}", 
                    page_data.m_qa_token[0],
                    log.idx_m,
                    page_data.m_qa_token[1],
                    log.idx_qa,
                    page_data.m_qa_token[2],
                    log.token,
                    page_data.m_qa_token[3],
                )
            } else {
                format!("{}{}{}{}{}", 
                    page_data.m_qa_token[0],
                    log.idx_m,
                    page_data.m_qa_token[1],
                    log.idx_qa,
                    if page_data.m_qa_token[2].ends_with("，") {
                        page_data.m_qa_token[2].strip_suffix("，").unwrap()
                    } else {
                        ""
                    },
                )
            };
            result += &format!("\n            <!-- user -->
            <div class='right-time'>
                <span id='d{}' class='for_focus_button del_btn' title='{}'>
                    {}
                </span>
                {}{}
            </div>
            <div class='user-chat-box'>
                <div class='q_icon_query'>
                    <div class='chat-txt right' id='m{}' title='{}'></div>
                    <div class='chat-icon'>\n", log.id, page_data.delete[0], ICON_DELETE, if log.is_web {"🌐 "} else {""}, log.time, log.id, tmp_title);
            if log.is_img || log.is_voice {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_USER);
            } else {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' onclick=\"copy('m{}');\" title='{}' src='{}' />", log.id, page_data.copy, ICON_USER);
            }
            result += r###"
                    </div>
                </div>
            </div>
"###;
        } else { // 答案
            result += &format!("            <!-- robot -->
            <div class='left-time'>
                {}
                <span id='d{}' class='for_focus_button del_btn' title='{}'>
                    {}
                </span>
            </div>
            <div class='gpt-chat-box'>
                <div class='chat-icon'>\n", log.time, log.id, page_data.delete[1], ICON_DELETE);
            if log.is_img || log.is_voice {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_CHATGPT);
            } else {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' onclick=\"copy('m{}');\" title='{}' src='{}' />", log.id, page_data.copy, ICON_CHATGPT);
            }
            result += &format!("
                </div>
                <div class='chat-txt left' id='m{}' title='{}{}{}{}{}{}{}'></div>
            </div>\n", log.id, page_data.m_qa_token[0], log.idx_m, page_data.m_qa_token[1], log.idx_qa, page_data.m_qa_token[2], log.token, page_data.m_qa_token[3]);
        }
    }
    result += r###"        </div>

        <!-- user input region -->
        <div class="chat-inputs-container">
            <div class="chat-inputs-inner">
"###;
    result += &format!("                <label for='upload-file' title='{}'>
                    <input id='upload-file' type='file' name='file' multiple>
                    <img id='upload-file-icon' src='{}' />
                </label>
                <textarea autofocus name='Input your query' id='input_query' placeholder='{}'></textarea>
                <span id='submit_span' class='for_focus_button'>
                    <img src='{}' class='search_btn' aria-hidden='true' />", page_data.upload, ICON_UPLOAD, page_data.textarea, ICON_SEND);
    result += r###"
                </span>
            </div>
        </div>

    </div>

    <!-- 弹窗结构 -->
    <div class="modal-overlay" id="permissionModal">
        <div id="modal-box">
            <h3 class="modal-title">Approval</h3>
            <div class="modal-message" id="modalMessage"></div>
            <div class="modal-actions">
                <button class="btn btn-agree" onclick="handleUserChoice(true)">Agree</button>
                <button class="btn btn-disagree" onclick="handleUserChoice(false)">Disagree</button>
            </div>
        </div>
    </div>

    <!-- footer -->
    <footer>
"###;
    result += &format!("        <button onclick='toggle()' id='left-toggle' class='left-bottom' title='{}'>
            <img src='{}' aria-hidden='true' />
        </button>
        <a href='http://{}:{}{}/save-log' id='left-save' class='left-bottom' title='{}'>
            <img src='{}' aria-hidden='true' />
        </a>
        <a href='http://{}:{}{}/usage' id='left-usage' class='left-bottom' title='{}'>
            <img src='{}' aria-hidden='true' />
        </a>
        <div id='left-compress' class='left-bottom' title='{}'>
            <img src='{}' id='compress' aria-hidden='true' />
        </div>
        <div id='left-incognito' class='left-bottom' title='{}'>
            <img src='{}' id='incognito' aria-hidden='true' />
        </div>", page_data.button[0], ICON_SETTING, PARAS.addr_str, PARAS.port, v, page_data.button[1], ICON_DOWNLOAD, PARAS.addr_str, PARAS.port, v, page_data.button[2], ICON_HELP, page_data.button[3], ICON_COMPRESS, if is_incognito { &page_data.incognito[1] } else { &page_data.incognito[0] }, if is_incognito { ICON_INCOGNITO2 } else { ICON_INCOGNITO1 });
    result += r###"
        <!-- <div>&copy; 2025 Copyright srx</div> -->
        <a href='https://github.com/jingangdidi'>https://github.com/jingangdidi</a>
    </footer>

    <script>
"###;
    result += &format!("{}\n", PRISM_MIN_JS);
    result += &format!("{}\n", MARKED_MIN_JS);
    result += &format!("{}\n", DIFF2HTML_JS);
    result += r###"    </script>
    <script>
        // markdown转html
        function markhigh() {
"###;
    for log in logs.iter() {
        result += &format!("            var msg = document.getElementById('m{}');
            var tmp = `{}`; // 这里将模板中的chat内容（已将“`”做了转译，“script”结束标签去掉了“<”）存入变量中
            if (tmp.startsWith('data:image/svg+xml;base64,')) {{ // 插入图片
                let tmp_img = document.createElement('img');
                tmp_img.src = tmp;
                msg.appendChild(tmp_img);\n", log.id, log.content);
        if log.is_voice {
            result += "                tmp_img.setAttribute('class', 'voice-size');\n"; // 设置voice图标大小
        }
        if !log.is_query { // 回答生成的图片或音频文件，添加hover下载按钮
            result += &format!("                let tmp_div = document.createElement('div');
                tmp_div.setAttribute('class', 'details');
                let tmp_a = document.createElement('a');
                tmp_a.setAttribute('class', 'title');
                tmp_a.setAttribute('href', 'http://{}:{}{}/save/{}');
                tmp_a.textContent = 'Download';
                tmp_div.appendChild(tmp_a);
                msg.setAttribute('class', 'chat-txt left tile'); // 加上tile
                msg.appendChild(tmp_div);\n", PARAS.addr_str, PARAS.port, v, log.id);
        }
        result += r###"
            } else { // 文本问题或答案
                tmp = tmp.replaceAll('\\`', '`').replaceAll('/scrip', '</scrip'); // 恢复转译的“`”和“script”结束标签
"###;
        if log.is_query { // 用户输入的问题
            result += "                msg.textContent = tmp.replaceAll('\\\\n', '\\n');\n            }\n            // 问题不需要markdown解析\n";
        } else { // 答案
            result += &format!("                if (tmp.includes('edit_file') && tmp.includes(' result\\n```\\n--- ')) {{
                    var text_diff = tmp.split(' result\\n```');
                    msg.innerHTML = marked.parse(text_diff[0]+' result').replaceAll('<p>', '').replaceAll('</p>', '');
                    let diff_code = document.createElement('div');
                    diff_code.setAttribute('id', 'm{}diff');
                    const diffCode = Diff2Html.html('```'+text_diff[1], {{
                        drawFileList: false,
                        matching: 'lines',
                        //colorScheme: 'dark',
                        outputFormat: 'side-by-side'
                    }});
                    diff_code.innerHTML = diffCode;
                    msg.appendChild(diff_code);
                }} else {{
                    msg.innerHTML = marked.parse(tmp).replaceAll('<p>', '').replaceAll('</p>', '');
                }}
            }}", log.id);
        }
    }
    result += &format!("        }}
        window.onload = markhigh();
        document.getElementById('show-prompt').value = '{}';
        document.getElementById('show-uuid').value = '{}';
        document.getElementById('show-in-token').value = '{}';
        document.getElementById('show-out-token').value = '{}';
", prompt_name, uuid, token[0], token[1]);
    result += "    </script>
</body>

<!-- js -->
<script type='module'>
";
    result += &format!("    var address = 'http://{}:{}{}/chat?q='; // http://127.0.0.1:8080\n    var current_id = {}; // 当前最新message的id，之后插入新问题或答案的id会基于该值继续增加\n    var qa_num = {}; // 问答对数量\n    var m_num = {}; // 信息数\n    var last_is_answer = true; // 最后一条信息是否是回答\n", PARAS.addr_str, PARAS.port, v, next_msg_id, qa_num, m_num);
    result += r###"    var emptyInput = true; // 全局变量，存储输入问题是否为空
    var no_message = true; // 是否没有获取到效回复，没有获取到，则将添加的msg_res删掉
    var already_clear_log = false; // 是否已清除了当前的记录
    var for_markdown = ''; // 累加原始信息，用于markdown显示
    var del_id = ''; // 要删除的信息的id
    var compress = 'false'; // summary/compress current chat history
    var submit_send_stop;
    // 左侧下拉菜单选取完成后，自动focus到问题输入框
    document.querySelectorAll('.for_focus').forEach(select => {
        select.addEventListener('change', function() {
            document.getElementById('input_query').focus();
        });
    });
    // 切换无痕模式，参数only_update为null表示进行toggle，为true表示更新为开启无痕模式，false表示更新为关闭无痕模式
    function incognito_toggle(toggle) {
        const incognitoDiv = document.getElementById('left-incognito');
        const incognitoImg = document.getElementById('incognito');
        let open_incognito;
        let send_update = false;
        if (toggle === null) {
            //console.log(1, toggle);
"###;
    result += &format!("            open_incognito = incognitoDiv.title.substring(0, 2) === '{}';", page_data.incognito[2]);
    result += r###"
            send_update = true;
        } else if (toggle) {
            //console.log(2, toggle);
            open_incognito = true;
        } else {
            //console.log(3, toggle);
            open_incognito = false;
        }
"###;
    result += &format!("        if (open_incognito) {{ // 此时关闭状态，更新后变为开启
            incognitoImg.src = '{}';
            incognitoDiv.title = '{}';
        }} else {{ // 此时开启状态，更新后变为关闭
            incognitoImg.src = '{}';
            incognitoDiv.title = '{}';
        }}
        if (send_update) {{
            fetch('http://{}:{}{}/incognito').catch(error => {{
                console.error('Failed set incognito:', error);
            }});
        }}
    }}", ICON_INCOGNITO2, page_data.incognito[1], ICON_INCOGNITO1, page_data.incognito[0], PARAS.addr_str, PARAS.port, v);
    result += r###"
    // 监听点击无痕模式按钮
    document.getElementById('left-incognito').addEventListener('click', function(event) {
        incognito_toggle(null);
    })
    // 使用事件委托监听点击事件
    document.addEventListener('click', async function(event) {
        if (event.target.classList.contains('for_focus_button')) { // 点击提交按钮和头像后，自动focus到问题输入框。由于头像消息是动态增加的，因此不能像上面那样，而应该使用事件委托
            document.getElementById('input_query').focus();
        } else { // 删除消息按钮
            const delBtn = event.target.closest('.del_btn'); // 这里要获取最近的del_btn，否则点击删除图标可能无效
            if (delBtn && isStopped) {
                const idx_num = Number(delBtn.id.substring(1));
                if (idx_num < 18446744073709551612) { // rust usize最后4个数是示例信息的id，没记录在服务端，不需要删除
                    // 向服务端发送删除信息的请求
"###;
    result += &format!("                    const response = await fetch('http://{}:{}{}/delmsg/'+delBtn.id);\n", PARAS.addr_str, PARAS.port, v);
    result += r###"                    if (response.ok) {
                        // 前端删除
                        const parentDiv = delBtn.parentNode; // 获取按钮的父div
                        const nextDiv = parentDiv.nextElementSibling; // 获取下一个相邻的div
                        // 删除父div和下一个div（如果存在）
                        if (parentDiv && parentDiv.tagName === 'DIV') parentDiv.remove();
                        if (nextDiv && nextDiv.tagName === 'DIV') nextDiv.remove();
                        // 总信息数减1
                        m_num -= 1;
                        // 更新删除后所有信息的第几条信息、第几对QA
                        // 1. 获取所有class="chat-txt"的div元素
                        const allDivMsg = document.querySelectorAll('div.chat-txt');
                        // 2. 遍历每个元素
                        let m_num_new = 0;
                        let qa_num_new = 0;
                        let last_is_answer_new = true;
                        allDivMsg.forEach(div => {
                            // 获取当前信息的id序号
                            const id_num = Number(div.id.substring(1)); // 'm0' -> 0
                            if (id_num < 18446744073709551612) { // rust usize最后4个数是示例信息的id，没记录在服务端，忽略
                                const msg_l = div.classList.contains('left');
                                const msg_r = div.classList.contains('right');
                                m_num_new += 1;
                                if (msg_l) {
                                    last_is_answer_new = true;
                                } else if (msg_r) {
                                    if (last_is_answer_new) {
                                        qa_num_new += 1;
                                        last_is_answer_new = false;
                                    }
                                }
                                // 如果当前信息id序号比删除的信息id序号大，则需要更新
                                if (id_num >= idx_num) {
"###;
    result += &format!("                                    const newTitle = div.getAttribute('title').replace(/{}(\\d+){}(\\d+){}/, `{}${{m_num_new}}{}${{qa_num_new}}{}`);", page_data.m_qa_token[0], page_data.m_qa_token[1], page_data.m_qa_token[2], page_data.m_qa_token[0], page_data.m_qa_token[1], page_data.m_qa_token[2]);
    result += r###"
                                    div.setAttribute("title", newTitle);
                                }
                            }
                        });
                        // 最后更新全局m_num第几条信息、qa_num第几对QA、last_is_answer
                        m_num = m_num_new;
                        qa_num = qa_num_new;
                        last_is_answer = last_is_answer_new;
                    } else {
                        console.error('delete message error');
                    }
                }
            }
        }
    });
    // 停止接收回答
    let reader; // 接收答案
    let isStopped = true; // 是否停止接收答案
    // 左下按钮，切换左侧参数栏
    let toggleMain = true; // true显示主参数，false显示其余参数
    function toggle() {
        toggleMain = !toggleMain;
        const left_main = document.getElementById('left-part');
        const left_other = document.getElementById('left-part-other');
        if (toggleMain) {
            left_other.classList.add('animate');
            left_main.style.display = 'block';
            left_other.style.display = 'none';
            sleep(300).then(() => { // 这里300ms要与css中animate的时间相同
                left_main.classList.remove('animate');
            });
        } else {
            left_main.classList.add('animate');
            left_other.style.display = 'block';
            left_main.style.display = 'none';
            sleep(300).then(() => { // 这里300ms要与css中animate的时间相同
                left_other.classList.remove('animate');
            });
        }
    }
    window.toggle = toggle;
    function sleep(time) {
        return new Promise((resolve) => setTimeout(resolve, time));
    }

    // 将pdf每页转为图片
    //import { getDocument, GlobalWorkerOptions } from 'https://cdnjs.cloudflare.com/ajax/libs/pdf.js/5.4.149/pdf.min.mjs';
    //GlobalWorkerOptions.workerSrc = 'https://cdnjs.cloudflare.com/ajax/libs/pdf.js/5.4.149/pdf.worker.mjs';
    // 渲染单页PDF为Blob对象
    // pdfDocument: pdf.js加载的文档对象
    // pageNo: 页码 (从1开始)
    // conversion_config: - 转换配置，如scale
    // 返回一个包含图片数据的Blob对象的Promise
    function renderPage(pdfDocument, pageNo, conversion_config) {
        // 返回一个Promise，因为页面渲染和toBlob都是异步的
        return new Promise((resolve, reject) => {
            // 获取指定页码的页面对象
            pdfDocument.getPage(pageNo).then(page => {
                const scale = conversion_config.scale || 1.5;
                const viewport = page.getViewport({ scale: scale });

                // 创建一个离屏canvas元素
                const canvas = document.createElement('canvas');
                const context = canvas.getContext('2d');
                canvas.height = viewport.height;
                canvas.width = viewport.width;

                // 渲染配置
                const renderContext = {
                    canvasContext: context,
                    viewport: viewport,
                };

                // 开始渲染页面到canvas上
                page.render(renderContext).promise.then(() => {
                    // 渲染完成后，将canvas内容转换为Blob对象
                    // toBlob是异步的，它接受一个回调函数
                    canvas.toBlob(blob => {
                        if (blob) {
                            resolve(blob); // 成功，将blob传递出去
                        } else {
                            reject(new Error(`Failed to create blob for page ${pageNo}`));
                        }
                    }, 'image/png'); // 输出为PNG格式
                }).catch(reject); // 捕获渲染错误
            }).catch(reject); // 捕获获取页面错误
        });
    }

    // 将PDF文件转换为图片Blob数组
    // pdfInput: 上传的File对象或一个ArrayBuffer
    // getDocument: 从PDF.js动态导入的getDocument函数
    // conversion_config: 配置对象，比如 { scale: 1.5 }
    // 返回一个包含所有页面图片Blob的数组的Promise
    function convertPdfToImages(pdfInput, getDocument, conversion_config = {}) {
        return new Promise((resolve, reject) => {
            const reader = new FileReader();

            // 当FileReader读取完成时
            reader.onload = function(event) {
                const pdfData = event.target.result; // 这是ArrayBuffer

                // 使用pdf.js加载PDF数据
                const loadingTask = getDocument(pdfData);
                loadingTask.promise.then(pdfDocument => {
                    const numPages = pdfDocument.numPages;
                    const pagePromises = [];

                    // 为每一页创建一个渲染任务（Promise）
                    for (let i = 1; i <= numPages; i++) {
                        pagePromises.push(renderPage(pdfDocument, i, conversion_config));
                    }

                    // 等待所有页面的渲染任务完成
                    Promise.all(pagePromises)
                        .then(imageBlobs => {
                            resolve(imageBlobs); // 所有页面转换成功，返回Blob数组
                        })
                        .catch(reject); // 捕获Promise.all中的任何一个错误
                }).catch(reject); // 捕获加载PDF文档的错误
            };

            // 当FileReader读取失败时
            reader.onerror = function() {
                reject(new Error("Failed to read the file."));
            };

            // 如果输入是File对象，开始读取它
            if (pdfInput instanceof File) {
                reader.readAsArrayBuffer(pdfInput);
            } 
            // 如果已经是ArrayBuffer，直接处理
            else if (pdfInput instanceof ArrayBuffer) {
                // 模拟onload事件，以便重用逻辑
                reader.onload({ target: { result: pdfInput } });
            } else {
                reject(new Error("Invalid input type. Expected File or ArrayBuffer."));
            }
        });
    }

    // 存储上传的文件
    let uploadedFiles = {};
    document.getElementById("upload-file").onchange = async function(event) {
        uploadedFiles = {};
        const formData = new FormData();
        for (let i = 0; i < event.target.files.length; i++) {
            const file = event.target.files[i];
            if (file) {
                //if (file.name.toLowerCase().endsWith('.pdf')) {
                if (file.name.endsWith('.pdf')) { // lowercase means convert to image, otherwise upload pdf file, extract text from pdf file
                    // convert pdf to images
                    try {
                        // 1. 动态导入PDF.js库。这个操作返回一个Promise
                        // 只有当用户上传PDF时，才会执行这里的网络请求
                        const pdfjsLib = await import('https://cdnjs.cloudflare.com/ajax/libs/pdf.js/5.4.149/pdf.min.mjs');
                        // 2. 从导入的模块中解构出所需的函数和对象
                        const { getDocument, GlobalWorkerOptions } = pdfjsLib;
                        // 3. 设置PDF.js的worker路径
                        GlobalWorkerOptions.workerSrc = 'https://cdnjs.cloudflare.com/ajax/libs/pdf.js/5.4.149/pdf.worker.mjs';
                        // 4. 调用转换函数，并将getDocument作为参数传入
                        const imageBlobs = await convertPdfToImages(file, getDocument, { scale: 1.5 }); // 1.0有点模糊，2.0浪费token
                        // 5. 转换完成后，再将 Blob 添加到 formData
                        imageBlobs.forEach((blob, i) => {
                            // 页面右侧插入图片
                            uploadedFiles[`${i + 1}.png`] = 'm'+current_id;
                            insert_right_image(); // 先插入右侧的空内容，后面写入图片或上传文件的文件名
                            let new_id = 'm'+(current_id-1);
                            const msg_req_right = document.getElementById(new_id);
                            // 生成临时URL并设置为图片的src
                            const objectURL = URL.createObjectURL(blob);
                            let right_img = document.createElement("img");
                            right_img.src = objectURL;
                            msg_req_right.appendChild(right_img);
                            // 记录要上传的图片
                            const fileName = `${i + 1}.png`;
                            formData.append('files', blob, fileName);
                            //console.log(`Appended ${fileName} to FormData`);
                        });
                    } catch (error) {
                        // 这个 catch 块会捕获两种错误：
                        // a) 动态 import() 失败（因为无网络）
                        // b) convertPdfToImages() 内部处理PDF时出错（文件损坏等）
                        console.error("PDF conversion failed for", file.name, error);
                        alert(`Unable to process the PDF file "${file.name}". Please ensure you have an internet connection to use this feature, or verify the file's validity`);
                        continue; // 跳过这个文件，继续处理下一个
                    }
                } else {
                    uploadedFiles[file.name] = 'm'+current_id;
                    insert_right_image(); // 先插入右侧的空内容，后面写入图片或上传文件的文件名
                    let new_id = 'm'+(current_id-1);
                    const msg_req_right = document.getElementById(new_id);
                    formData.append('files', file);
                    if (file.type.startsWith('image/')) { // 插入显示上传的图片或文件名
                        // 生成临时URL并设置为图片的src
                        const objectURL = URL.createObjectURL(file);
                        let right_img = document.createElement("img");
                        right_img.src = objectURL;
                        msg_req_right.appendChild(right_img);
                    } else { // 如果不是图片，显示上传文件的名称
                        msg_req_right.textContent = file.name;
                    }
                }
                sleep(100).then(() => { // 这里要等一小会儿，否则滚动到底之后图片才加载完，看上去未滚动到底
                    scroll();
                });
            }
        }

        /*
        console.log(formData);
        for (let [key, value] of formData.entries()) {
            console.log(key, value);
        }
        */

        // 上传
"###;
    result += &format!("        fetch('http://{}:{}{}/upload', {{", PARAS.addr_str, PARAS.port, v);
    result += r###"
            method: 'POST',
            body: formData
        })
        .then(response => response.json())
        .then(data => {
            //console.log('服务器返回的上传文件token数:', data);
            Object.entries(uploadedFiles).forEach(([key, value]) => {
                if (data[key] > 0) {
                    let msg_lr = document.getElementById(value);
                    const currentTitle = msg_lr.getAttribute("title");
"###;
    result += &format!("                    msg_lr.setAttribute('title', currentTitle+data[key]+'{}');", page_data.m_qa_token[3]);
    result += r###"
                    // 更新页面左侧总输入token
                    let tmp = document.getElementById("show-in-token");
                    tmp.value = parseInt(tmp.value) + data[key];
                }
                //console.log(`Key: ${key}, Value: ${value}`);
            });
        })
        .catch(error => {
            console.error('上传文件失败:', error);
        });
        document.getElementById('input_query').focus();
    };
    // 清空指定元素的所有子元素，https://stackoverflow.com/questions/3955229/remove-all-child-elements-of-a-dom-node-in-javascript
    function clear_all_child(id_name) {
        const parent = document.getElementById(id_name)
        while (parent.firstChild) {
            parent.firstChild.remove();
        }
    }
    // 更新相关uuid的下拉项
    function related_uuid(uuids) {
        clear_all_child('select-related-uuid');
        let options = document.getElementById("select-related-uuid");
        let disabled_option = document.createElement("option");
        disabled_option.setAttribute("value", "-1");
        disabled_option.setAttribute("disabled", "");
        disabled_option.setAttribute("selected", "");
        disabled_option.text = "--select uuid--";
        options.appendChild(disabled_option);
        for (let i of uuids) {
            let uuid_option = document.createElement("option");
            uuid_option.setAttribute("value", i[0]);
            uuid_option.text = i[0]+' ('+i[1]+')';
            options.appendChild(uuid_option);
        }
    }
    // 插入右侧图片
    function insert_right_image() {
        /* 输入内容 */
        let msg_req_right = document.createElement("div");
        msg_req_right.setAttribute("class", "chat-txt right");
        let new_id = 'm'+current_id;
        current_id += 1; // id序号加1
        if (last_is_answer) {
            qa_num += 1;
            last_is_answer = false;
        }
        m_num += 1; // 更新总信息数
"###;
    result += &format!("        msg_req_right.setAttribute('title', '{}'+m_num+'{}'+qa_num+'{}');", page_data.m_qa_token[0], page_data.m_qa_token[1], page_data.m_qa_token[2]);
    result += r###"
        msg_req_right.setAttribute("id", new_id);
        /* 头像 */
        let icon_div = document.createElement("div");
        icon_div.setAttribute("class", "chat-icon");
        let icon_right = document.createElement("img");
"###;
    result += &format!("        icon_right.setAttribute('src', '{}');\n", ICON_USER);
    result += r###"        icon_right.setAttribute("class", "chatgpt-icon for_focus_button");
        //icon_right.setAttribute("onclick", "copy('"+new_id+"');");
        //icon_right.setAttribute("title", "点击复制");
        icon_div.appendChild(icon_right);
        /* 提问的头像和内容放到一个div右侧对齐 */
        let q_icon_query_div = document.createElement("div");
        q_icon_query_div.setAttribute("class", "q_icon_query");
        q_icon_query_div.appendChild(msg_req_right);
        q_icon_query_div.appendChild(icon_div);
        /* 用户输入内容最外的div */
        let Con1 = document.createElement("div");
        Con1.setAttribute("class", "user-chat-box");
        /* chat区域插入输入内容和头像 */
        let message = document.getElementById("scrolldown");
        /* 提问的当前时间 */
        let timeInfo = document.createElement("div");
        timeInfo.setAttribute("class", "right-time");

        let delicon = document.createElement("span");
        delicon.setAttribute("id", "d"+(current_id-1));
        delicon.setAttribute("class", "for_focus_button del_btn");
"###;
    result += &format!("        delicon.setAttribute('title', '{}');", page_data.delete[0]);
    result += r###"
        const parser = new DOMParser(); // 1. 创建一个解析器
"###;
    result += &format!("        const svgDoc = parser.parseFromString(`{}`, 'image/svg+xml'); // 2. 将 SVG 字符串解析为 XML 文档，注意类型是 'image/svg+xml'\n", ICON_DELETE);
    result += r###"        const svgElement = svgDoc.documentElement; // 3. 从解析后的文档中获取根元素，即 <svg> 元素
        delicon.appendChild(svgElement);
        timeInfo.appendChild(delicon);
        let time_text = document.createTextNode(formatDate(true));
        timeInfo.appendChild(time_text);
        message.appendChild(timeInfo);
        Con1.appendChild(q_icon_query_div);
        message.appendChild(Con1);
    }
    // 插入左侧答案和右侧问题
    function insert_left_right(message_content, message_time, id, is_left, is_img, is_voice, is_web, current_token, is_diff) {
        if (id === current_id) { // 当前消息还没插入
            let new_id = 'm'+current_id; // 当前要插入消息的id
            current_id += 1; // id序号加1
            let msg_lr = document.createElement("div");
            msg_lr.setAttribute("id", new_id);
            if (is_img) { // 插入图片
                let lr_img = document.createElement("img");
                lr_img.src = message_content;
                msg_lr.appendChild(lr_img);
                if (is_left) { // 左侧图片hover时下载按钮
                    msg_lr.setAttribute("class", "chat-txt left tile"); // tile用于hover时下载图片或语音
                    let tmp_div = document.createElement('div');
                    tmp_div.setAttribute('class', 'details');
                    let tmp_a = document.createElement('a');
                    tmp_a.setAttribute('class', 'title');
                    if (is_voice) {
                        lr_img.setAttribute('class', 'voice-size'); // 设置voice图标大小
                    }
"###;
    result += &format!("                    tmp_a.setAttribute('href', 'http://{}:{}{}/save/'+(current_id-1));\n", PARAS.addr_str, PARAS.port, v);
    result += r###"                    tmp_a.textContent = 'Download';
                    tmp_div.appendChild(tmp_a);
                    msg_lr.appendChild(tmp_div);
                } else {
                    msg_lr.setAttribute("class", "chat-txt right");
                }
            } else {
                if (is_left) { // 文本答案
                    for_markdown = message_content.replaceAll('srxtzn', '\n');
                    msg_lr.setAttribute("class", "chat-txt left");
                    if (is_diff) {
                        var text_diff = for_markdown.split(' result\n\`\`\`');
                        // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
                        msg_lr.innerHTML = marked.parse(text_diff[0]+' result').replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
                        let diff_code = document.createElement("div");
                        diff_code.setAttribute("id", new_id+'diff');
                        const diffCode = Diff2Html.html('\`\`\`'+text_diff[1], {
                            drawFileList: false,
                            matching: 'lines',
                            //colorScheme: 'dark',
                            outputFormat: 'side-by-side'
                        });
                        diff_code.innerHTML = diffCode;
                        msg_lr.appendChild(diff_code);
                    } else {
                        // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
                        msg_lr.innerHTML = marked.parse(for_markdown).replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
                        // 对每个代码块进行高亮
                        msg_lr.querySelectorAll('pre code').forEach((block) => {
                            Prism.highlightElement(block);
                        });
                    }
                } else { // 文本问题
                    msg_lr.setAttribute("class", "chat-txt right");
                    msg_lr.textContent = message_content.replaceAll('srxtzn', '\n').replaceAll('\\n', '\n'); // 不要使用innerHTML，innerHTML会识别标签将内容解析为html，textContent只是文本，innerText会受到css影响，https://stackoverflow.com/questions/31002593/type-new-line-character-in-element-textcontent
                }
            }
            /* 头像 */
            let icon_div = document.createElement("div");
            icon_div.setAttribute("class", "chat-icon");
            let icon_lr = document.createElement("img");
"###;
    result += &format!("            if (is_left) {{ // 答案
                icon_lr.setAttribute('src', '{}');
            }} else {{ // 问题
                icon_lr.setAttribute('src', '{}');
            }}
", ICON_CHATGPT, ICON_USER);
    result += r###"            icon_lr.setAttribute("class", "chatgpt-icon for_focus_button");
            if (!is_img) {
                icon_lr.setAttribute("onclick", "copy('"+new_id+"');");
"###;
    result += &format!("                icon_lr.setAttribute('title', '{}');", page_data.copy);
    result += r###"
            }
            icon_div.appendChild(icon_lr);

            /* 最外层提问/回答的当前时间 */
            let timeInfo = document.createElement("div");
            let delicon = document.createElement("span");
            delicon.setAttribute("id", "d"+(current_id-1));
            delicon.setAttribute("class", "for_focus_button del_btn");
            const parser = new DOMParser(); // 1. 创建一个解析器
"###;
    result += &format!("            const svgDoc = parser.parseFromString(`{}`, 'image/svg+xml'); // 2. 将 SVG 字符串解析为 XML 文档，注意类型是 'image/svg+xml'\n", ICON_DELETE);
    result += r###"            const svgElement = svgDoc.documentElement; // 3. 从解析后的文档中获取根元素，即 <svg> 元素
            delicon.appendChild(svgElement);

            let time_text = document.createTextNode(message_time);
            if (is_web) {
                time_text = document.createTextNode("🌐 "+message_time);
            }
"###;
    result += &format!("            if (is_left) {{
                timeInfo.setAttribute('class', 'left-time');
                delicon.setAttribute('title', '{}');
                timeInfo.appendChild(time_text);
                timeInfo.appendChild(delicon);
            }} else {{
                timeInfo.setAttribute('class', 'right-time');
                delicon.setAttribute('title', '{}');
                timeInfo.appendChild(delicon);
                timeInfo.appendChild(time_text);
            }}

            /* chat区域插入问题/答案的时间 */
            let message = document.getElementById('scrolldown');
            message.appendChild(timeInfo);

            // 更新总信息数
            m_num += 1;

            if (is_left) {{
                last_is_answer = true;
                if (current_token > 0) {{
                    msg_lr.setAttribute('title', '{}'+m_num+'{}'+qa_num+'{}'+current_token+'{}');
                }} else {{
                    msg_lr.setAttribute('title', '{}'+m_num+'{}'+qa_num+'{}'); // 这里先不显示token数，等回答完成后再加上
                }}
                /* 答案外的div */
                let Con2 = document.createElement('div');
                Con2.setAttribute('class', 'gpt-chat-box');
                /* chat区域插入答案和头像 */
                Con2.appendChild(icon_div);
                Con2.appendChild(msg_lr);
                /* 提问的当前时间 */
                message.appendChild(Con2);
            }} else {{
                if (last_is_answer) {{
                    qa_num += 1;
                    last_is_answer = false;
                }}
                if (current_token > 0) {{
                    msg_lr.setAttribute('title', '{}'+m_num+'{}'+qa_num+'{}'+current_token+'{}');
                }} else {{
                    msg_lr.setAttribute('title', '{}'+m_num+'{}'+qa_num+'{}'); // 这里先不显示token数，等回答完成后再加上
                }}", page_data.delete[1], page_data.delete[0], page_data.m_qa_token[0], page_data.m_qa_token[1], page_data.m_qa_token[2], page_data.m_qa_token[3], page_data.m_qa_token[0], page_data.m_qa_token[1], page_data.m_qa_token[2], page_data.m_qa_token[0], page_data.m_qa_token[1], page_data.m_qa_token[2], page_data.m_qa_token[3], page_data.m_qa_token[0], page_data.m_qa_token[1], if page_data.m_qa_token[2].ends_with("，") { page_data.m_qa_token[2].strip_suffix("，").unwrap() } else { "" });
    result += r###"
                /* 提问的头像和内容放到一个div右侧对齐 */
                let q_icon_query_div = document.createElement("div");
                q_icon_query_div.setAttribute("class", "q_icon_query");
                q_icon_query_div.appendChild(msg_lr);
                q_icon_query_div.appendChild(icon_div);
                /* 用户输入内容最外的div */
                let Con1 = document.createElement("div");
                Con1.setAttribute("class", "user-chat-box");
                Con1.appendChild(q_icon_query_div);
                message.appendChild(Con1);
            }
        } else if (id === current_id - 1) { // 当前消息已经插入，继续追加内容。由于图片base64在一个stream中，因此这里只能是stream传输的文本答案
            let new_id = 'm'+id; // 当前要插入消息的id
            let msg_lr = document.getElementById(new_id);
            for_markdown += message_content.replaceAll('srxtzn', '\n');
            if (is_diff) {
                var text_diff = for_markdown.split(' result\n\`\`\`');
                // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
                msg_lr.innerHTML = marked.parse(text_diff[0]+' result').replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
                let diff_code = document.getElementById(new_id+'diff');
                const diffCode = Diff2Html.html('\`\`\`'+text_diff[1], {
                    drawFileList: false,
                    matching: 'lines',
                    //colorScheme: 'dark',
                    outputFormat: 'side-by-side'
                });
                diff_code.innerHTML = diffCode;
            } else {
                // 注意这里去除转换后的`<p>`和`</p>`，因为p标签会让回复内容上下有更多的空间，与右侧提问不一致
                msg_lr.innerHTML = marked.parse(for_markdown).replaceAll('<p>', '').replaceAll('</p>', ''); // 转为markdown显示，https://github.com/markedjs/marked，head标签中加上：<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"><\/script>
                // 对每个代码块进行高亮
                msg_lr.querySelectorAll('pre code').forEach((block) => {
                    Prism.highlightElement(block);
                });
            }
        } else { // 不应该出现
            console.error(`message id not match: current_id='${current_id}', received_id='${id}'`);
        }
    }
    // 个位数左侧加0补为2位数，https://www.toptal.com/software/definitive-guide-to-datetime-manipulation
    function pad(n) {
        return n<10 ? '0'+n : n;
    }
    // 获取当前时间，并格式化为：2024-10-20 17:37:47，https://stackoverflow.com/questions/14638018/current-time-formatting-with-javascript
    function formatDate(is_user) {
        var d = new Date();
        var year = d.getFullYear();
        var month = pad(d.getMonth()+1); // 0-11
        var day = pad(d.getDate()); // 1-31
        var hr = pad(d.getHours()); // 0-23
        var min = pad(d.getMinutes()); // 0-59
        var sec = pad(d.getSeconds()); // 0-59
        if (is_user) {
            return year+"-"+month+"-"+day+" "+hr+":"+min+":"+sec;
        } else {
            // https://stackoverflow.com/questions/14976495/get-selected-option-text-with-javascript
            var sel = document.getElementById("select-model");
            var text= sel.options[sel.selectedIndex].text.split(" (")[0];
            return year+"-"+month+"-"+day+" "+hr+":"+min+":"+sec+" "+text;
        }
    }
    // 复制指定头像id对应的内容
    function copy(id) {
        // https://code-boxx.com/strip-remove-html-tags-javascript/
        var textToCopy = document.getElementById(id).textContent;
        //console.log(textToCopy);
        navigator.clipboard.writeText(textToCopy);
    }
    window.copy = copy;
    /* chat region scroll bottom */
    function scroll() {
        var scrollMsg = document.getElementById("scrolldown");
        scrollMsg.scrollTop = scrollMsg.scrollHeight;
    }
"###;
    result += &format!("    // ask approval
    let wait_approval = false;
    const modal = document.getElementById('permissionModal');
    const messageEl = document.getElementById('modalMessage');
    const modalBox = document.getElementById('modal-box');
    function showApprovalWindow(msg, useDiff) {{
        if (useDiff) {{
            /*const diffStr = `
--- /data/srx/cloud/test.py
+++ /data/srx/cloud/test.py
@@ -1,6 +1,6 @@
a = 1
b = 2
c = 3
-println!('{{}}', a);
+print(a)
print(b)
-print(c/0)
+print(c)
`;*/
            const diffCode = Diff2Html.html(msg, {{
                drawFileList: false,
                matching: 'lines',
                outputFormat: 'side-by-side'
            }});
            modalBox.style.maxWidth = '800px';
            messageEl.innerHTML = diffCode;
        }} else {{
            modalBox.style.maxWidth = '400px';
            messageEl.textContent = msg;
        }}
        modal.classList.add('active');
    }}
    function handleUserChoice(isAgreed) {{
        modal.classList.remove('active');
        sendApprovalToBackend(isAgreed);
        wait_approval = false;
    }}
    window.handleUserChoice = handleUserChoice; // 暴露给全局
    function sendApprovalToBackend(agreed) {{
        fetch('http://{}:{}{}/approval?approval='+agreed).catch(error => {{
            console.error('Failed send approval to server:', error);
        }});
    }}
    //showApprovalWindow('是否允许运行该工具？', true);", PARAS.addr_str, PARAS.port, v);
    result += r###"
    // 获取用户发起提问时提交的信息
    function get_url() {
        var req = document.getElementById("input_query").value;
        if (req !== '') { // 输入不为空才不在界面显示输入内容
            emptyInput = false;
            // 插入用户输入内容
            //insert_left_right(req, formatDate(true), current_id, false, false, false); // 不在这里插入问题，后面问题会作为MainData插入，附带token数等信息
        } else {
            emptyInput = true;
        }
        // 清空输入框，滚动到最下面，等待答案
        document.getElementById("input_query").value = "";
        scroll();
        // https://stackoverflow.com/questions/1085801/get-selected-value-in-dropdown-list-using-javascript
        // 获取选择的模型
        var para_model = document.getElementById("select-model").value;
        // get selected tools
        var para_tool = document.getElementById("select-tool").value;
        // plan mode
        var para_plan = document.getElementById("select-plan").checked;
        // 获取选择的思考深度
        var para_effort = document.getElementById("select-effort").value;
        // 获取输入的对话名称
        var para_chat_name = document.getElementById("input-chat-name").value;
        // 获取输入的uuid
        var para_uuid = document.getElementById("input-uuid").value;
        if (para_uuid === '') { // 输入的uuid优先级要高于下拉选择的uuid
            para_uuid = document.getElementById("select-related-uuid").value;
            if (para_uuid === '-1') {
                para_uuid = '';
            }
        }
        // 获取输入的temperature
        var para_temperature = document.getElementById("input-temperature").value;
        // 获取输入的top-p
        var para_top_p = document.getElementById("input-top-p").value;
        // 获取选择的stream
        var para_stm = document.getElementById("select-stm").checked;
        // 获取是否网络搜索
        var para_web = document.getElementById("select-web").checked;
        // 获取选择的要保留的最近的最多问答记录数
        var para_num = document.getElementById("select-log-num").value;
        // 使用选择的prompt开启新对话
        var para_prompt = document.getElementById("select-prompt").value;
        // 使用选择生成音频的声音
        var para_voice = document.getElementById("select-voice").value;
        // 输入框无效，并显示信息
"###;
    result += &format!("        if (emptyInput) {{ // 输入为空表示提问
            var q = 0;
            document.getElementsByName('Input your query')[0].placeholder = '{} ...';
        }} else if (para_web) {{ // 使用网络搜索需要等待搜索结束
            var q = 1;
            document.getElementsByName('Input your query')[0].placeholder = '{} ...';
        }} else {{ // 输入不为空表示用户继续提问
            var q = 1;
            document.getElementsByName('Input your query')[0].placeholder = '{} ...';
        }}", page_data.wait[0], page_data.wait[1], page_data.wait[2]);
    result += r###"
        document.getElementById('input_query').disabled = true; // 完成回复之前禁止继续提问
        // 将参数加到问题后面
        let req2 = q+"&model="+para_model+"&chatname="+para_chat_name+"&uuid="+para_uuid+"&stream="+para_stm+"&web="+para_web+"&num="+para_num+"&prompt="+para_prompt+"&voice="+para_voice+"&effort="+para_effort+"&temp="+para_temperature+"&topp="+para_top_p+"&tools="+para_tool+"&compress="+compress+"&plan="+para_plan;
        compress = 'false';
        return [req, req2];
    }
    // 回答完成后恢复提问输入框
    function restore_input() {
"###;
    result += &format!("        submit_send_stop.innerHTML = \"<img src='{}' class='search_btn' aria-hidden='true' />\";\n", ICON_SEND);
    result += r###"        isStopped = true;
        document.getElementById("select-prompt").value = '-1'; // prompt恢复为不开启新会话
        //document.getElementById("input-chat-name").value = ''; // 清空填写的对话名称
        document.getElementById("input-uuid").value = ''; // 清空填写的uuid，此时左下“current uuid”中显示的即是填写的uuid
        document.getElementById("input_query").value = "";
        document.getElementById('input_query').disabled = false; // 已完成回复，可以继续提问
"###;
    result += &format!("        document.getElementsByName('Input your query')[0].placeholder = '{}';", page_data.textarea);
    result += r###"
        document.getElementById("input_query").focus();
    }
    // 提交问题并获取答案
    let controller = null;
    async function send_query_receive_answer() {
        // 从服务器获取stream内容
        no_message = true;
        already_clear_log = false;
        var autoScroll = true; // 默认随着流式输出自动滚动，如果用户进行了手动滚动，则停止自动滚动，这样就保持页面停留在用户想看的那个位置
        let tmpmsg = ""; // 累加存储流式输出的结果，转为markdown
        submit_send_stop = document.getElementById("submit_span");
"###;
    result += &format!("        submit_send_stop.innerHTML = \"<img src='{}' class='search_btn' style='width: 50px;' aria-hidden='true' />\";\n", ICON_STOP);
    result += r###"        isStopped = false;
        // 由于EventSource不支持post，因此无法将问题通过body传递，只能放到url中通过url参数传递，但url有长度限制（好像大部分浏览器是2k），因此输入内容长度不能太长
        // 这里用fetch发送post，将问题字符串通过body传递，其他简单参数通过url传递
        let [req, req2] = get_url();
        controller = new AbortController();
        const response = await fetch(address+req2, {
            method: 'POST',
            signal: controller.signal,
            headers: {
                'Content-Type': 'text/plain;charset=UTF-8',
                'Accept': 'text/event-stream'
            },
            body: req,
        });
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = ''; // Buffer to accumulate partial messages
        // 解析数据
        while (!isStopped) {
            try {
                const { done, value } = await reader.read();
                if (done) {
                    // Process any remaining data in buffer if it forms a complete message
                    if (buffer.trim()) processSseBuffer(); 
                    break;
                }
                buffer += decoder.decode(value, { stream: true }); // stream: true is important
                processSseBuffer();
            } catch (e) {
                if (e.name === 'AbortError') {
                    console.log('The request has been terminated');
                } else {
                    //console.error(`Failed to parse JSON for event '${currentEvent}':`, e, 'Raw data:', eventData);
                    console.log('Failed to parse JSON:', e);
                }
            }
        }
        restore_input();
        // 解析完整数据
        function processSseBuffer() {
            let eolIndex;
            // SSE messages are separated by double newlines "\n\n"
            while ((eolIndex = buffer.indexOf('\n\n')) >= 0) {
                // 从buffer中获取“\n\n”之前的内容
                const messageStr = buffer.substring(0, eolIndex);
                // 从buffer中去除“\n\n”以及之前的内容，buffer此时剩下“\n\n”之后的内容
                buffer = buffer.substring(eolIndex + 2);
                // Skip empty messages
                if (messageStr.trim() === '') continue;
                // Parse the individual SSE message
                let currentEvent = 'maindata'; // Default event type
                let currentData = [];
                // 根据\n拆分解析每行，注意一个data内不要有\n，多行可以写到多个data中
                messageStr.split('\n').forEach(line => {
                    if (line.startsWith('event: ')) {
                        currentEvent = line.substring('event: '.length).trim();
                    } else if (line.startsWith('data: ')) {
                        currentData.push(line.substring('data: '.length));
                    } else {
                        console.warn("line not starts with event and data:", line);
                    }
                });
                // 用\n将data数据合并为一个字符串
                const eventData = currentData.join('\n');
                // 基于event类型解析数据
                const jsonData = JSON.parse(eventData);
                switch (currentEvent) {
                    case 'metadata':
                        incognito_toggle(jsonData.is_incognito);
                        let answer_id = 'm'+(current_id - 1); // 当前回答的id
                        let msg_lr = document.getElementById(answer_id);
                        const currentTitle = msg_lr.getAttribute("title");
                        if (jsonData.current_token > 0) { // 回答结束，更新token数
"###;
    result += &format!("                            msg_lr.setAttribute('title', currentTitle+jsonData.current_token+'{}');", page_data.m_qa_token[3]);
    result += r###"
                        }
                        //console.log('Received metadata:', jsonData);
                        // 更新页面左测当前uuid、问题token、答案token、prompt名称、相关uuid
                        document.getElementById("input-chat-name").value = jsonData.chat_name;
                        document.getElementById("show-prompt").value = jsonData.prompt;
                        document.getElementById("show-uuid").value = jsonData.current_uuid;
                        document.getElementById("show-in-token").value = jsonData.in_token;
                        document.getElementById("show-out-token").value = jsonData.out_token;
                        document.getElementById("show-context-token").value = jsonData.context_token;
                        related_uuid(jsonData.related_uuid);
                        if (autoScroll) {
                            scroll();
                        }
                        break; // 否则会继续执行下面的case
                    case 'maindata':
                        // ask approval
                        if (jsonData.approval) {
                            wait_approval = true;
                            showApprovalWindow(jsonData.approval.replaceAll('srxtzn', '\n'), jsonData.diff);
                        } else if (!wait_approval || jsonData.content !== '') {
                            //console.log('Received maindata:', jsonData);
                            // 如果信息是之前的问答记录，先清空当前所有信息
                            if (!already_clear_log && jsonData.is_history) {
                                clear_all_child('scrolldown');
                                already_clear_log = true;
                                current_id = 0;
                                qa_num = 0;
                                m_num = 0;
                                last_is_answer = true;
                            }
                            // https://stackoverflow.com/questions/15275969/javascript-scroll-handler-not-firing
                            // https://www.answeroverflow.com/m/1302587682957824081
                            window.addEventListener('wheel', function(event) { // “scroll”无效
                                // event.deltaY 的值：负值表示向上滚动，正值表示向下滚动
                                if (autoScroll) {
                                    //console.log('Scrolling via mouse');
                                    if (event.deltaY < 0) { // 向上滚动
                                        autoScroll = false; // 用户手动向上滚动，停止自动向下滚动
                                    }
                                } else {
                                    if (event.deltaY > 0) { // 向下滚动
                                        autoScroll = true; // 用户手动向下滚动，恢复自动向下滚动
                                    }
                                }
                            });
                            window.addEventListener('touchmove', function() { // 触屏这个有效，没有deltaY，先不考虑触屏滚动方向
                                if (autoScroll) {
                                    //console.log('Scrolling via touch');
                                    autoScroll = false; // 用户手动进行滚动，后面将不再自动滚动
                                }
                            });
                            no_message = false;
                            // 如果是之前的记录，则用传递的id更新当前id，因为传递的id可能不连续（有部分被用户点击删除）
                            if (jsonData.is_history && jsonData.id !== current_id && jsonData.id !== current_id - 1) {
                                current_id = jsonData.id
                            }
                            // 插入信息
                            if (jsonData.time_model) {
                                insert_left_right(jsonData.content, jsonData.time_model, jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token, jsonData.diff);
                            } else { // 没有传递时间则使用当前时间
                                if (jsonData.is_left) {
                                    insert_left_right(jsonData.content, formatDate(false), jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token, jsonData.diff);
                                } else {
                                    insert_left_right(jsonData.content, formatDate(true), jsonData.id, jsonData.is_left, jsonData.is_img, jsonData.is_voice, jsonData.is_web, jsonData.current_token, jsonData.diff);
                                }
                            }
                            //Prism.highlightAll();
                            if (autoScroll) {
                                if (jsonData.is_img) {
                                    sleep(100).then(() => { // 这里要等一小会儿，否则滚动到底之后图片才加载完，看上去未滚动到底
                                        scroll();
                                    });
                                } else {
                                    scroll();
                                }
                            }
                        }
                        break; // 否则会继续执行下面的case
                    case 'close':
                        //console.log('Received close:', jsonData);
                        break; // 否则会继续执行下面的case
                    default:
                        console.log(`Received unhandled event '${currentEvent}':`, jsonData);
                }
            }
        }
    }
    scroll();
    // 按下回车键发送
    document.getElementById("input_query").addEventListener("keydown", async(e) => {
        if (e.key === 'Enter') {
            if (e.shiftKey) { // 换行
                return;
            } else { // 提交问题
                e.preventDefault(); // 阻止默认的换行行为
                if (isStopped) { // 发送问题
                    del_id = '';
                    await send_query_receive_answer();
                } else { // 停止接收回答
                    //if (reader) reader.cancel();
                    controller.abort();
                    restore_input();
                    isStopped = true;
                    controller = null;
                }
            }
        }
    });
    // 鼠标点击按钮发送
    document.getElementById("submit_span").addEventListener("click", async(e) => {
        if (isStopped) { // 发送问题
            del_id = '';
            await send_query_receive_answer();
        } else { // 停止接收回答
            //if (reader) reader.cancel();
            controller.abort();
            restore_input();
            isStopped = true;
            controller = null;
        }
    });
    // click left bottom summary/compress button
    document.getElementById("left-compress").addEventListener("click", async(e) => {
        if (isStopped) { // 发送问题
            del_id = '';
            compress = 'true';
            await send_query_receive_answer();
        } else { // 停止接收回答
            //if (reader) reader.cancel();
            controller.abort();
            restore_input();
            isStopped = true;
            controller = null;
        }
    });
</script>

</html>
"###;
    result
}

/// 生成指定uuid对话记录的html字符串，css和js都写在html中，供下载使用
/// err_str不是None表示无法获取chat记录，记录的是错误信息
pub fn create_download_page(uuid: &str, err_str: Option<String>) -> String {
    // 页面信息
    let page_data_locked = PAGE.read().unwrap();
    let page_data = page_data_locked.get(&PARAS.english).unwrap();

    // 创建包含css和js，并插入chat记录的html页面
    let mut result = r###"<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="author" content="srx">
    <title>chat log</title>
"###.to_string();
    //result += &format!("    <link rel='shortcut icon' href='{}/templates/images/robot-7.svg' type='image/x-icon'>\n", v);
    result += &format!("    <link rel='shortcut icon' href='{}' type='image/x-icon'>\n", ICON_SHORTCUT);
    result += "</head>\n";

    result += "<style type='text/css'>\n";
    result += CSS_CODE_DOWNLOAD;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += PRISM_MIN_CSS;
    result += "</style>\n";

    result += "<style type='text/css'>\n";
    result += DIFF2HTML_CSS;
    result += "</style>\n";

    result += r###"<body>
    <div id="right-part" class="content">
        <!-- chat content region -->
        <div id="scrolldown" class="chat-content-area">
"###;
    // 获取该uuid的chat记录，如果传递的err_str不是None，则表示无法获取chat记录
    let logs = match err_str {
        Some(e) => vec![DisplayInfo{is_query: false, content:  e, id: 0, time: "".to_string(), is_img: false, is_voice: false, is_web: false, idx_qa: 1, idx_m: 1, token: 0}],
        None => {
            // 在保存当前chat记录之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
            // pop_message_before_end(uuid); // 这里不要执行这一函数，只在关闭服务时执行，这里执行完，如果再继续输入问题，会因为id与服务端不对应而报错
            get_log_for_display(uuid, true).3 // cookie对应的chat记录
        },
    };
    for log in logs.iter() {
        if log.is_query { // 用户输入的问题
            let tmp_title = if log.token > 0 {
                format!("{}{}{}{}{}{}{}", 
                    page_data.m_qa_token[0],
                    log.idx_m,
                    page_data.m_qa_token[1],
                    log.idx_qa,
                    page_data.m_qa_token[2],
                    log.token,
                    page_data.m_qa_token[3],
                )
            } else {
                format!("{}{}{}{}{}", 
                    page_data.m_qa_token[0],
                    log.idx_m,
                    page_data.m_qa_token[1],
                    log.idx_qa,
                    if page_data.m_qa_token[2].ends_with("，") {
                        page_data.m_qa_token[2].strip_suffix("，").unwrap()
                    } else {
                        ""
                    },
                )
            };
            result += &format!("            <!-- user -->
            <div class='right-time'>{}{}</div>
            <div class='user-chat-box'>
                <div class='q_icon_query'>
                    <div class='chat-txt right' id='m{}' title='{}'></div>
                    <div class='chat-icon'>\n", if log.is_web {"🌐 "} else {""}, log.time, log.id, tmp_title);
            if log.is_img || log.is_voice {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_USER);
            } else {
                result += &format!("                        <img class='chatgpt-icon for_focus_button' onclick=\"copy('m{}');\" title='{}' src='{}' />", log.id, page_data.copy, ICON_USER);
            }
            result += r###"
                    </div>
                </div>
            </div>
"###;
        } else { // 答案
            result += &format!("            <!-- robot -->
            <div class='left-time'>{}</div>
            <div class='gpt-chat-box'>
                <div class='chat-icon'>\n", log.time);
            if log.is_img || log.is_voice {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' src='{}' />", ICON_CHATGPT);
            } else {
                result += &format!("                    <img class='chatgpt-icon for_focus_button' onclick=\"copy('m{}');\" title='{}' src='{}' />", log.id, page_data.copy, ICON_CHATGPT);
            }
            result += &format!("
                </div>
                <div class='chat-txt left' id='m{}' title='{}{}{}{}{}{}{}'></div>
            </div>\n", log.id, page_data.m_qa_token[0], log.idx_m, page_data.m_qa_token[1], log.idx_qa, page_data.m_qa_token[2], log.token, page_data.m_qa_token[3]);
        }
    }
    result += r###"        </div>
    </div>

    <!-- footer -->
    <footer>
        <!--<div>https://github.com/jingangdidi</div>-->
        <a href='https://github.com/jingangdidi'>https://github.com/jingangdidi</a>
    </footer>

    <script>
"###;
    result += &format!("{}\n", PRISM_MIN_JS);
    result += &format!("{}\n", MARKED_MIN_JS);
    result += &format!("{}\n", DIFF2HTML_JS);
    result += r###"    </script>
    <script>
        // markdown转html
        function markhigh() {
"###;
    for log in logs.iter() {
        result += &format!("            var msg = document.getElementById('m{}');
            var tmp = `{}`; // 这里将模板中的chat内容（已将“`”做了转译，“script”结束标签去掉了“<”）存入变量中
            if (tmp.startsWith('data:image/svg+xml;base64,')) {{ // 插入图片
                let tmp_img = document.createElement('img');
                tmp_img.src = tmp;
                msg.appendChild(tmp_img);
            }} else {{ // 文本问题或答案
                tmp = tmp.replaceAll('\\`', '`').replaceAll('/scrip', '</scrip'); // 恢复转译的“`”和“script”结束标签\n", log.id, log.content);
        if log.is_query { // 用户输入的问题
            result += "                msg.textContent = tmp.replaceAll('\\\\n', '\\n');\n            }\n // 问题不需要markdown解析\n";
        } else { // 答案
            result += &format!("                if (tmp.includes('edit_file') && tmp.includes(' result\\n```\\n--- ')) {{
                    var text_diff = tmp.split(' result\\n```');
                    msg.innerHTML = marked.parse(text_diff[0]+' result').replaceAll('<p>', '').replaceAll('</p>', '');
                    let diff_code = document.createElement('div');
                    diff_code.setAttribute('id', 'm{}diff');
                    const diffCode = Diff2Html.html('```'+text_diff[1], {{
                        drawFileList: false,
                        matching: 'lines',
                        //colorScheme: 'dark',
                        outputFormat: 'side-by-side'
                    }});
                    diff_code.innerHTML = diffCode;
                    msg.appendChild(diff_code);
                }} else {{
                    msg.innerHTML = marked.parse(tmp).replaceAll('<p>', '').replaceAll('</p>', '');
                }}
            }}", log.id);
        }
    }
    result += r###"        }
        window.onload = markhigh();
        function copy(id) {{
            // https://code-boxx.com/strip-remove-html-tags-javascript/
            var textToCopy = document.getElementById(id).textContent;
            navigator.clipboard.writeText(textToCopy);
        }}
    </script>
</body>
</html>
"###;
    result
}
