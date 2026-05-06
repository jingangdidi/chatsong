use std::{thread, time};

use arboard::Clipboard;
use openai_dive::v1::{
    api::Client,
    resources::{
        chat::{
            ChatMessage,
            ChatMessageContent,
            ChatCompletionParametersBuilder,
            ChatCompletionResponseFormat,
        },
        shared::ReasoningEffort,
    },
};
use rdev::{
    listen,
    Event,
    EventType,
    Key,
    simulate,
    SimulateError,
};

use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{event, Level};

use crate::{
    openai::for_chat::not_use_stream,
    parse_paras::PARAS,
};

/// 触发的按键信号
#[derive(PartialEq)]
pub enum KeySignal {
    Completion, // 代码补全
    Debug,      // 修复代码
    Shell,      // 补全或编写shell命令
}

/// 监听指定按键
/// 监听连续按下3次`ctrl`（windows或linux）或`command`（macOS），触发代码补全
/// 监听连续按下4次键盘左侧`shift`，触发debug
pub fn listen_hotkey(sender: UnboundedSender<KeySignal>) {
    // 监听连续按下3次`ctrl`（windows或linux）或`command`（macOS）
    let mut nctrl_release: u8 = 0; // 记录连续松开Ctrl键的次数，连续3次则触发提问
    let mut previous_ctrl_press = false; // 上一个键是否按下Ctrl键
    let mut previous_ctrl_release = false; // 上一个键是否松开Ctrl键
    // 监听连续按下4次键盘左侧`shift`
    let mut nshift_l_release: u8 = 0; // 记录连续松开左侧shift键的次数，连续3次则触发提问
    let mut previous_shift_l_press = false; // 上一个键是否按下左侧shift键
    let mut previous_shift_l_release = false; // 上一个键是否松开左侧shift键
    // 监听连续按下4次键盘右侧`shift`
    let mut nshift_r_release: u8 = 0; // 记录连续松开左侧shift键的次数，连续3次则触发提问
    let mut previous_shift_r_press = false; // 上一个键是否按下左侧shift键
    let mut previous_shift_r_release = false; // 上一个键是否松开左侧shift键

    if let Err(error) = listen(move |event: Event| {
        // 1(按下左侧`ctrl`或`command`), 2(释放左侧`ctrl`或`command`), 3(按下左侧`shift`), 4(释放左侧`shift`), 5(按下右侧`shift`), 6(释放右侧`shift`), 7(按下其他键), 8(释放其他键), 9(其他事件)
        let press_release = match event.event_type {
            EventType::KeyPress(key) => {
                match key {
                    Key::ControlLeft => if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
                        1
                    } else if cfg!(target_os = "macos") {
                        7
                    } else {
                        7
                    },
                    Key::MetaLeft => if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
                        7
                    } else if cfg!(target_os = "macos") {
                        1
                    } else {
                        7
                    },
                    Key::ShiftLeft => 3,
                    Key::ShiftRight => 5,
                    _ => 7,
                }
            },
            EventType::KeyRelease(key) => {
                match key {
                    Key::ControlLeft => if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
                        2
                    } else if cfg!(target_os = "macos") {
                        8
                    } else {
                        8
                    },
                    Key::MetaLeft => if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
                        8
                    } else if cfg!(target_os = "macos") {
                        2
                    } else {
                        8
                    },
                    Key::ShiftLeft => 4,
                    Key::ShiftRight => 6,
                    _ => 8,
                }
            },
            //EventType::ButtonPress(Button), // Linux下有效，Windows下无效
            //EventType::ButtonRelease(Button),
            //EventType::MouseMove{x: f64, y: f64},
            //EventType::Wheel{delta_x: i64, delta_y: i64},
            _ => 9,
        };

        if press_release == 1 || press_release == 2 {
            trigger_code_completion(&mut previous_ctrl_press, &mut previous_ctrl_release, &mut nctrl_release, press_release == 1);
            //println!("press ctrl: {}, release ctrl: {}, {}", previous_ctrl_press, previous_ctrl_release, nctrl_release);
            if nctrl_release >= 3 {
                // 这里是同步的，通过管道将监听信号发送给异步函数执行
                if let Err(e) = sender.send(KeySignal::Completion) {
                    event!(Level::ERROR, "listen keyboard error: {:?}", e);
                }
                // 恢复状态
                previous_ctrl_release = false;
                nctrl_release = 0;
            }
            // 更新其他键
            if press_release == 1 && previous_shift_l_press {
                previous_shift_l_press = false;
                previous_shift_r_press = false;
            } else if press_release == 2 && previous_shift_l_release {
                previous_shift_l_release = false;
                previous_shift_r_release = false;
            }
        } else if press_release == 3 || press_release == 4 {
            trigger_code_completion(&mut previous_shift_l_press, &mut previous_shift_l_release, &mut nshift_l_release, press_release == 3);
            //println!("press left shift: {}, release left shift: {}, {}", previous_shift_l_press, previous_shift_l_release, nshift_l_release);
            if nshift_l_release >= 4 {
                // 这里是同步的，通过管道将监听信号发送给异步函数执行
                if let Err(e) = sender.send(KeySignal::Debug) {
                    event!(Level::ERROR, "listen keyboard error: {:?}", e);
                }
                // 恢复状态
                previous_shift_l_release = false;
                nshift_l_release = 0;
            }
            // 更新其他键
            if press_release == 3 && previous_ctrl_press {
                previous_ctrl_press = false;
                previous_shift_r_press = false;
            } else if press_release == 4 && previous_ctrl_release {
                previous_ctrl_release = false;
                previous_shift_r_release = false;
            }
        } else if press_release == 5 || press_release == 6 {
            trigger_code_completion(&mut previous_shift_r_press, &mut previous_shift_r_release, &mut nshift_r_release, press_release == 5);
            //println!("press right shift: {}, release right shift: {}, {}", previous_shift_r_press, previous_shift_r_release, nshift_r_release);
            if nshift_r_release >= 4 {
                // 这里是同步的，通过管道将监听信号发送给异步函数执行
                if let Err(e) = sender.send(KeySignal::Shell) {
                    event!(Level::ERROR, "listen keyboard error: {:?}", e);
                }
                // 恢复状态
                previous_shift_r_release = false;
                nshift_r_release = 0;
            }
            // 更新其他键
            if press_release == 3 && previous_ctrl_press {
                previous_ctrl_press = false;
                previous_shift_l_press = false;
            } else if press_release == 4 && previous_ctrl_release {
                previous_shift_l_release = false;
                previous_ctrl_release = false;
            }
        } else if press_release != 9 { // 更新监听按键的状态
            if press_release == 7 {
                previous_ctrl_press = false;
                previous_shift_l_press = false;
                previous_shift_r_press = false;
            } else if press_release == 8 {
                previous_ctrl_release = false;
                previous_shift_l_release = false;
                previous_shift_r_release = false;
            }
        }
    }) {
        event!(Level::ERROR, "listen keyboard error: {:?}", error);
    }
}

/// 根据`EventType`判断是否触发指定功能
///fn trigger_code_completion(event: &EventType, previous_press: &mut bool, previous_release: &mut bool, n_release: &mut u8, key_type: Key) {
fn trigger_code_completion(previous_press: &mut bool, previous_release: &mut bool, n_release: &mut u8, is_press: bool) {
    if is_press { // press
        if !*previous_press {
            *previous_press = true;
        }
    } else { // release
        match (*previous_press, *previous_release) {
            (true, true) => *n_release += 1,
            (true, false) => {
                *previous_release = true;
                *n_release = 1;
            },
            (false, true) => *n_release = 0,
            (false, false) => {
                *previous_release = true;
                *n_release = 0;
            },
        }
    }
}

/// 调用键盘
fn press_release_key(event_type: &EventType) -> Result<(), SimulateError> {
    simulate(event_type)?;
    // Let ths OS catchup (at least MacOS)
    thread::sleep(time::Duration::from_millis(50));
    Ok(())
}

/// 代码补全prompt
/// -----------------------------------------------------
/// 你是一名精通多种编程语言的资深开发工程师，擅长理解上下文意图。
///
/// 任务目标：
///     根据下面给出的代码前缀，预测并生成后续最符合逻辑的代码片段。
/// 
/// 生成规范：
///     1. 一致性：保持与现有代码相同的缩进、命名风格及注释习惯，避免在不必要的地方开启新行，不要输出解释、对话、markdown代码围栏。
///     2. 准确性：确保语法正确，逻辑连贯，无幻觉引用。
///     3. 简洁性：仅输出需要补全的代码部分，不包含用户输入内容的后缀，用户会自己将输入内容与你回复的内容合并为完整代码。
///     4. 安全性：避免生成潜在的漏洞或不安全的代码实践。
///
/// 下方为待补全的代码，请继续编写。
/// -----------------------------------------------------
/// https://github.com/x1xhlol/system-prompts-and-models-of-ai-tools/blob/main/VSCode%20Agent/nes-tab-completion.txt
const COMPLETION_PROMPT: &str = r###"You are a senior development engineer proficient in multiple programming languages, skilled at understanding contextual intent.

Task Objective:
    Based on the code prefix provided below, predict and generate the most logically consistent subsequent code fragment.

Generation Guidelines:
    1. Consistency: Maintain the same indentation, naming conventions, and commenting style as the existing code. Avoid inserting unnecessary new lines. Do not output explanations, dialogues, or markdown code fences.
    2. Accuracy: Ensure syntactic correctness and logical coherence, avoid hallucinated references.
    3. Conciseness: Output only the necessary code to complete the fragment. Do not include any suffixes of the user's input. The user will manually merge your response with their input to form the complete code.
    4. Security: Avoid generating potentially vulnerable or unsafe code practices.

Below is the code context requiring completion. Please continue writing.
"###;

/// 修复代码prompt
/// -----------------------------------------------------
/// 你是一个代码审查与修复专家，分析以下代码中的所有缺陷（语法错误、逻辑错误、运行时异常、安全隐患等），进行精确修复，并输出完整的修正后代码。
///
/// 修复规则：
///     1. 不得改变代码原本的正确功能与意图。
///     2. 在每处修改的相邻位置，使用注释标记原因，格式为：`# BUGFIX: <简要说明>`（自动适配代码所用的语言，如 Python 使用 `#`，Rust 使用 `//`）。
///     3. 输出必须是完整、可直接运行的代码，原样保留未修改的部分（包括原有注释）。
///     4. 禁止输出任何解释、对话、markdown代码围栏或其他额外内容。
///     5. 保持与原代码完全一致的缩进、命名风格、代码风格，仅修改必要的缺陷处。
/// -----------------------------------------------------
const DEBUG_PROMPT: &str = r###"You are a code review and repair expert. Analyze all defects in the following code (syntax errors, logical errors, runtime exceptions, security vulnerabilities, etc.), make precise fixes, and output the complete corrected code.

Fixing rules:
    1. Do not alter the original correct functionality or intent of the code.
    2. At each modified location, add a comment explaining the reason, using the format: `# BUGFIX: <brief description>` (automatically adapted to the language used, e.g., `#` for Python, `//` for Rust).
    3. The output must be a complete, directly runnable code, retain unmodified parts exactly as they were (including original comments).
    4. Do not output any explanations, dialogue, markdown code fences, or additional content.
    5. Preserve the original indentation, naming conventions, and coding style exactly, modify only the necessary defective parts.
"###;

/// https://github.com/Myzel394/zsh-copilot/blob/main/zsh-copilot.plugin.zsh
#[allow(dead_code)]
const SHELL_PROMPT: &str = r###"You will be given the raw input of a shell command.
Your task is to either complete the command or provide a new command that you think the user is trying to type.

Your response MAY NOT contain any newlines!
Do NOT add any additional text, comments, or explanations to your response.

Do NOT ask for more information, you won't receive it.
DO NOT INTERACT WITH THE USER IN NATURAL LANGUAGE! If you do, you will be banned from the system.

Your response will be run in the user's shell.
Make sure input is escaped correctly if needed so.
Note that the double quote sign is escaped. Keep this in mind when you create quotes.
Your input should be able to run without any modifications to it.

Here are two examples: 
    * User input: 'list files in current directory'; Your response: 'ls' (ls is the builtin command for listing files)
    * User input: 'cd /tm'; Your response: 'cd /tmp' (/tmp is the standard temp folder on linux and mac)
"###;

/// https://github.com/violettoolssite/cnmsb/blob/main/src/ai/completer.rs
/// -----------------------------------------------------
/// 你是一个 Linux/Unix shell 命令生成助手。根据用户输入，提供对应的 shell 命令建议。
///
/// 用户输入可能是：
///     1. 部分命令（如 "git com"）- 提供命令补全
///     2. 自然语言描述（如 "从文件 a.txt 中获取第一列内容"）- 生成对应的 shell 命令
///     3. 中英文混合 - 理解意图并生成命令
///
/// 规则：
///     1. 必须输出为一行完整的 shell 命令，如果需要多个命令，必须写为一行，不要有换行符
///     2. 格式：命令 # 简短描述
///     3. 不要输出解释性文字，只输出命令
///     4. 如果用户用中文描述意图，生成对应的英文 shell 命令
///
/// 示例1 - 命令补全：
///     用户输入: git com
///     输出: git commit -m "" # 提交更改
///
/// 示例2 - 自然语言转命令：
///     用户输入: 提交代码到仓库
///     输出: git add . && git commit -m "update" && git push # 添加、提交并推送
///
/// 示例3 - 自然语言：
///     用户输入: 查看磁盘使用情况
///     输出: df -h # 显示磁盘使用情况
/// -----------------------------------------------------
const SHELL_ASSIST_PROMPT: &str = r###"You are a Linux/Unix shell command generator assistant. Based on the user input, provide corresponding shell command suggestions.

User input may be:
    1. Partial command (e.g., "git com") — offer command completion
    2. Natural language description (e.g., "get the first column from file a.txt") — generate the corresponding shell command
    3. Mixed Chinese and English — understand the intent and generate the command

Rules:
    1. Output must be a single line of complete shell command, if multiple commands are needed, combine them on one line without line breaks
    2. Do not output explanatory text, only output the command
    3. If the user describes the intent in Chinese, generate the corresponding English shell command

Example 1 - Command completion:
    User input: git com
    Output: git commit -m "xxx"
Example 2 - Natural language to command:
    User input: Commit code to repository
    Output: git add . && git commit -m "update" && git push
Example 3 - Natural language:
    User input: Check disk usage
    Output: df -h
"###;

/// 问题和模型
#[derive(Clone)]
pub struct ModelForCompletion {
    pub model:           String,
    pub lowercase_model: String,
    pub api_key:         String,
    pub endpoint:        String,
    pub thinking:        bool,
}

impl ModelForCompletion {
    /// 默认模型
    pub fn new() -> Self {
        let (api_key, endpoint, model, thinking) = PARAS.api.get_default_model().unwrap_or(("".to_string(), "".to_string(), "".to_string(), false));
        let lowercase_model = model.to_lowercase();
        Self {
            model,
            lowercase_model,
            api_key,
            endpoint,
            thinking,
        }
    }

    /// 根据指定模型序号（1-based）获取模型
    fn from_n(n: usize) -> Option<Self> {
        if let Ok((api_key, endpoint, model, thinking)) = PARAS.api.get_model_by_usize(n) {
            let lowercase_model = model.to_lowercase();
            Some(Self {
                model,
                lowercase_model,
                api_key,
                endpoint,
                thinking,
            })
        } else {
            None
        }
    }

    /// 调用LLM
    pub async fn code_completion_llm(&mut self, clipboard: &mut Clipboard, key_signal: KeySignal) {
        let mut run_next = true; // 是否正常没报错，继续余下流程
        let mut question: Option<String> = None; // 从剪切板获取的问题
        let mut answer: Option<String> = None; // 答案或错误
        let copy_paste_key = if cfg!(target_os = "macos") {
            Key::MetaLeft
        } else {
            Key::ControlLeft
        };
        if KeySignal::Shell == key_signal {
            // 1. 将当前命令行的命令复制到剪切板
            // `ctrl+a`光标移到起始
            // 调用键盘输入`echo "`
            // `ctrl+e`光标移到末尾
            // 调用键盘输入`" ｜ 系统剪切板命令`，`clip`（windows），`xclip -selection clipboard`（linux），`pbcopy`（macOS）
            // 调用键盘按下`enter`键执行上面命令，此时原始命令已复制到剪切板
            if let Some(e) = press_multi_keys(vec![Key::ControlLeft, Key::KeyA]) {
                answer = Some(format!("{}", e));
                run_next = false;
                event!(Level::ERROR, "1. code_completion_llm: {}", e);
            } else if let Some(e) = press_string_key("echo \"", clipboard) {
                answer = Some(format!("{}", e));
                run_next = false;
                event!(Level::ERROR, "1. code_completion_llm: {}", e);
            } else if let Some(e) = press_multi_keys(vec![Key::ControlLeft, Key::KeyE]) {
                answer = Some(format!("{}", e));
                run_next = false;
                event!(Level::ERROR, "1. code_completion_llm: {}", e);
            } else if cfg!(target_os = "windows") {
                // ` | clip`
                if let Some(e) = press_string_key("\" | clip", clipboard) {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "1. code_completion_llm: {}", e);
                }
            } else if cfg!(target_os = "linux") {
                // ` | xclip -selection clipboard`
                // apt install xclip
                if let Some(e) = press_string_key("\" | xclip -selection clipboard", clipboard) {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "1. code_completion_llm: {}", e);
                }
            } else if cfg!(target_os = "macos") {
                // ` | pbcopy`
                if let Some(e) = press_string_key("\" | pbcopy", clipboard) {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "1. code_completion_llm: {}", e);
                }
            }
            if answer.is_some() {
                // `ctrl+u`清空当前命令
                if let Some(e) = press_multi_keys(vec![Key::ControlLeft, Key::KeyU]) {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "1. code_completion_llm: {}", e);
                }
            } else {
                // `enter`
                if let Some(e) = press_release_single_key(Key::Return) {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "1. code_completion_llm: {}", e);
                } else {
                    event!(Level::INFO, "1. code_completion_llm: copy command line text to clipboard");
                }
            }
        } else {
            // 1. 调用`ctrl+c`（windows或linux）或`command+c`（macOS）将选中内容复制到剪切板
            if let Some(e) = press_multi_keys(vec![copy_paste_key, Key::KeyC]) {
                answer = Some(format!("{}", e));
                run_next = false;
                event!(Level::ERROR, "1. code_completion_llm: {}", e);
            } else {
                event!(Level::INFO, "1. listen_hotkey_run_llm: copy text to clipboard");
            }
        }
        // 2. 从剪切板获取问题
        if run_next {
            match clipboard.get_text() {
                Ok(q) => {
                    let trim_q = q.trim_matches('"').to_string();
                    if trim_q.is_empty() {
                        event!(Level::INFO, "2. listen_hotkey_run_llm: empty content from clipboard");
                        run_next = false;
                    } else if let Ok(n) = trim_q.parse::<usize>() { // 修改模型
                        if let Some(c) = ModelForCompletion::from_n(n) {
                            event!(Level::INFO, "2. listen_hotkey_run_llm: change model from {} to {}", self.model, c.model);
                            *self = c;
                        } else {
                            event!(Level::INFO, "2. listen_hotkey_run_llm: model unchanged {}", self.model);
                        }
                        run_next = false;
                        if let Some(e) = press_release_single_key(Key::Backspace) {
                            event!(Level::ERROR, "2. listen_hotkey_run_llm: {}", e);
                        }
                    } else if let Some(suffix) = trim_q.strip_prefix("thinking=") {
                        if suffix == "true" {
                            event!(Level::INFO, "2. listen_hotkey_run_llm: enable {} thinking", self.model);
                            self.thinking = true;
                        } else if suffix == "false" {
                            event!(Level::INFO, "2. listen_hotkey_run_llm: disable {} thinking", self.model);
                            self.thinking = false;
                        }
                        run_next = false;
                        if let Some(e) = press_release_single_key(Key::Backspace) {
                            event!(Level::ERROR, "2. listen_hotkey_run_llm: {}", e);
                        }
                    } else {
                        event!(Level::INFO, "2. listen_hotkey_run_llm: get text from clipboard: `{}`", trim_q);
                        question = Some(trim_q);
                    }
                },
                Err(e) => {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "2. listen_hotkey_run_llm clipboard error: {:?}", e);
                },
            }
        }
        // 3. 开始提问
        if let Some(q) = question {
            event!(Level::INFO, "3. listen_hotkey_run_llm: run {}", self.model);
            let messages = vec![
                ChatMessage::User{
                    content: ChatMessageContent::Text(match key_signal {
                        KeySignal::Completion => COMPLETION_PROMPT.to_string(),
                        KeySignal::Debug => DEBUG_PROMPT.to_string(),
                        KeySignal::Shell => SHELL_ASSIST_PROMPT.to_string(),
                    }),
                    name: None,
                },
                ChatMessage::User{
                    content: ChatMessageContent::Text(q),
                    name: None,
                },
            ];
            // 使用api key初始化
            let mut client = Client::new(self.api_key.clone());
            client.set_base_url(&self.endpoint);
            let mut para_builder = ChatCompletionParametersBuilder::default();
            para_builder.model(self.model.clone()); // 指定模型
            para_builder.response_format(ChatCompletionResponseFormat::Text);
            // 对思维链模型设置effort
            if self.thinking {
                para_builder.reasoning_effort(ReasoningEffort::Low); // 设置使用思维链，Low（思考的少，简单问答）, Medium（思考适中，多步骤推理）, High（思考更多，复杂逻辑推导）
                // 开启思考，不同模型思考的设置不同
                if self.lowercase_model.starts_with("deepseek") {
                    // deepseek: https://api-docs.deepseek.com/
                    para_builder.extra_body(json!({"thinking": {"type": "enabled"}}));
                } else if self.lowercase_model.starts_with("qwen") {
                    // Qwen: https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions#05cfceb898csa
                    para_builder.extra_body(json!({"enable_thinking": true}));
                } else if self.lowercase_model.starts_with("kimi") {
                    // kimi: https://platform.kimi.com/docs/api/models-overview
                    para_builder.extra_body(json!({"thinking": {"type": "enabled"}}));
                } else if self.lowercase_model.starts_with("glm") {
                    // glm: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction
                    para_builder.extra_body(json!({"thinking": {"type": "enabled"}}));
                }
            } else {
                // 关闭思考，不同模型思考的设置不同
                if self.lowercase_model.starts_with("deepseek") {
                    // deepseek: https://api-docs.deepseek.com/
                    para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
                } else if self.lowercase_model.starts_with("qwen") {
                    // Qwen: https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions#05cfceb898csa
                    para_builder.extra_body(json!({"enable_thinking": false}));
                } else if self.lowercase_model.starts_with("kimi") {
                    // kimi: https://platform.kimi.com/docs/api/models-overview
                    para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
                } else if self.lowercase_model.starts_with("glm") {
                    // glm: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction
                    para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
                }
            }
            para_builder.messages(messages);
            match para_builder.build() {
                Ok(parameters) => {
                    match not_use_stream("listen_hotkey_run_llm".to_string(), client, parameters, &self.model, false).await {
                        Ok((result, _resoning)) => {
                            answer = if result.is_empty() {
                                Some("no response".to_string())
                            } else {
                                if KeySignal::Shell == key_signal {
                                    Some(result.replace("\r\n", "\n").replace('\n', " && "))
                                } else {
                                    Some(result)
                                }
                            };
                        },
                        Err(e) => {
                            answer = Some(format!("{}", e));
                            event!(Level::ERROR, "3. listen_hotkey_run_llm: {}", e);
                        },
                    }
                },
                Err(e) => {
                    answer = Some(format!("{}", e));
                    run_next = false;
                    event!(Level::ERROR, "3. listen_hotkey_run_llm: {:?}", e);
                },
            }
        }
        // 4. 答案写入剪切板
        if let Some(a) = &answer {
            if let Err(e) = clipboard.set_text(a) {
                run_next = false;
                event!(Level::ERROR, "4. listen_hotkey_run_llm clipboard error: {:?}", e);
            } else {
                run_next = true;
                thread::sleep(time::Duration::from_millis(20));
                event!(Level::INFO, "4. listen_hotkey_run_llm: set answer to clipboard");
            }
        }
        // 5. 调用`ctrl+c`（windows或linux）或`command+c`（macOS）将答案内容贴到编辑器
        if run_next {
            if KeySignal::Shell == key_signal { // shell命令不粘贴，调用键盘打印到终端
                if let Some(a) = &answer {
                    if let Some(e) = press_string_key(a, clipboard) {
                        event!(Level::ERROR, "5. listen_hotkey_run_llm: {}", e);
                    } else {
                        event!(Level::INFO, "5. listen_hotkey_run_llm: write answer to command line");
                    }
                }
            } else {
                if KeySignal::Completion == key_signal { // 代码补全需要取消选中，并将光标放在选中内容的最后
                    if let Some(e) = press_release_single_key(Key::RightArrow) {
                        event!(Level::ERROR, "5. listen_hotkey_run_llm: {}", e);
                    }
                }
                if let Some(e) = press_multi_keys(vec![copy_paste_key, Key::KeyV]) {
                    event!(Level::ERROR, "5. listen_hotkey_run_llm: {}", e);
                } else {
                    event!(Level::INFO, "5. listen_hotkey_run_llm: paste answer");
                }
            }
        }
    }
}

/// press & release single key
fn press_release_single_key(key: Key) -> Option<String> {
    // 按下
    if let Err(e) = press_release_key(&EventType::KeyPress(key)) {
        return Some(format!("press {:?}: {}", key, e))
    }
    // 松开
    if let Err(e) = press_release_key(&EventType::KeyRelease(key)) {
        return Some(format!("release {:?}: {}", key, e))
    }
    // 以上没有报错返回None
    None
}

/// press multiple keys
fn press_multi_keys(keys: Vec<Key>) -> Option<String> {
    // 先依次都按下
    for key in keys.clone() {
        if let Err(e) = press_release_key(&EventType::KeyPress(key)) {
            return Some(format!("press {:?}: {}", key, e))
        }
    }
    // 再依次都松开
    for key in keys.into_iter().rev() {
        if let Err(e) = press_release_key(&EventType::KeyRelease(key)) {
            return Some(format!("release {:?}: {}", key, e))
        }
    }
    // 以上没有报错返回None
    None
}

/// press string keyboard
fn press_string_key(command: &str, clipboard: &mut Clipboard) -> Option<String> {
    for c in command.chars() {
        let (k, need_shift) = match c {
            '`' => (Key::BackQuote, false),
            '~' => (Key::BackQuote, true),
            '1' => (Key::Num1, false),
            '!' => (Key::Num1, true),
            '2' => (Key::Num2, false),
            '@' => (Key::Num2, true),
            '3' => (Key::Num3, false),
            '#' => (Key::Num3, true),
            '4' => (Key::Num4, false),
            '$' => (Key::Num4, true),
            '5' => (Key::Num5, false),
            '%' => (Key::Num5, true),
            '6' => (Key::Num6, false),
            '^' => (Key::Num6, true),
            '7' => (Key::Num7, false),
            '&' => (Key::Num7, true),
            '8' => (Key::Num8, false),
            '*' => (Key::Num8, true),
            '9' => (Key::Num9, false),
            '(' => (Key::Num9, true),
            '0' => (Key::Num0, false),
            ')' => (Key::Num0, true),
            '-' => (Key::Minus, false),
            '_' => (Key::Minus, true),
            '=' => (Key::Equal, false),
            '+' => (Key::Equal, true),
            'q' => (Key::KeyQ, false),
            'Q' => (Key::KeyQ, true),
            'w' => (Key::KeyW, false),
            'W' => (Key::KeyW, true),
            'e' => (Key::KeyE, false),
            'E' => (Key::KeyE, true),
            'r' => (Key::KeyR, false),
            'R' => (Key::KeyR, true),
            't' => (Key::KeyT, false),
            'T' => (Key::KeyT, true),
            'y' => (Key::KeyY, false),
            'Y' => (Key::KeyY, true),
            'u' => (Key::KeyU, false),
            'U' => (Key::KeyU, true),
            'i' => (Key::KeyI, false),
            'I' => (Key::KeyI, true),
            'o' => (Key::KeyO, false),
            'O' => (Key::KeyO, true),
            'p' => (Key::KeyP, false),
            'P' => (Key::KeyP, true),
            'a' => (Key::KeyA, false),
            'A' => (Key::KeyA, true),
            's' => (Key::KeyS, false),
            'S' => (Key::KeyS, true),
            'd' => (Key::KeyD, false),
            'D' => (Key::KeyD, true),
            'f' => (Key::KeyF, false),
            'F' => (Key::KeyF, true),
            'g' => (Key::KeyG, false),
            'G' => (Key::KeyG, true),
            'h' => (Key::KeyH, false),
            'H' => (Key::KeyH, true),
            'j' => (Key::KeyJ, false),
            'J' => (Key::KeyJ, true),
            'k' => (Key::KeyK, false),
            'K' => (Key::KeyK, true),
            'l' => (Key::KeyL, false),
            'L' => (Key::KeyL, true),
            'z' => (Key::KeyZ, false),
            'Z' => (Key::KeyZ, true),
            'x' => (Key::KeyX, false),
            'X' => (Key::KeyX, true),
            'c' => (Key::KeyC, false),
            'C' => (Key::KeyC, true),
            'v' => (Key::KeyV, false),
            'V' => (Key::KeyV, true),
            'b' => (Key::KeyB, false),
            'B' => (Key::KeyB, true),
            'n' => (Key::KeyN, false),
            'N' => (Key::KeyN, true),
            'm' => (Key::KeyM, false),
            'M' => (Key::KeyM, true),
            ';' => (Key::SemiColon, false),
            ':' => (Key::SemiColon, true),
            '\'' => (Key::Quote, false),
            '"' => (Key::Quote, true),
            '[' => (Key::LeftBracket, false),
            '{' => (Key::LeftBracket, true),
            ']' => (Key::RightBracket, false),
            '}' => (Key::RightBracket, true),
            '\\' => (Key::BackSlash, false),
            '|' => (Key::BackSlash, true),
            ',' => (Key::Comma, false),
            '<' => (Key::Comma, true),
            '.' => (Key::Dot, false),
            '>' => (Key::Dot, true),
            '/' => (Key::Slash, false),
            '?' => (Key::Slash, true),
            ' ' => (Key::Space, false),
            //_ => return Some(format!("unsupported character: {}", c)),
            _ => {
                // 特殊字符不能通过键盘输入，这里通过剪切板输入，先写入剪切板，然后输入`ctrl+v`
                if let Err(e) = clipboard.set_text(c.to_string()) {
                    return Some(format!("set {} to clipboard error: {}", c, e))
                } else {
                    thread::sleep(time::Duration::from_millis(20));
                }
                if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
                    if let Some(e) = press_multi_keys(vec![Key::ControlLeft, Key::ShiftLeft, Key::KeyV]) {
                        return Some(e)
                    }
                } else if cfg!(target_os = "macos") {
                    if let Some(e) = press_multi_keys(vec![Key::MetaLeft, Key::KeyV]) {
                        return Some(e)
                    }
                } else {
                    return Some(format!("unsupported character: {}", c))
                }
                continue
            },
        };
        if need_shift {
            if let Some(e) = press_multi_keys(vec![Key::ShiftLeft, k]) {
                return Some(e)
            }
        } else {
            if let Some(e) = press_release_single_key(k) {
                return Some(e)
            }
        }
    }
    return None
}
