use std::collections::HashMap;
use std::fs::{read, write, create_dir_all, read_to_string};
use std::path::Path;
use std::sync::Mutex;

use axum_extra::extract::cookie::{Cookie, SameSite, CookieJar};
use chrono::{Local, NaiveDateTime};
use once_cell::sync::Lazy;
use openai_dive::v1::resources::chat::{
    ChatMessage,
    ChatMessageContent,
    ChatMessageContentPart,
};
use serde::{Serialize, Deserialize};
use tracing::{event, Level};

/// parse_paras: 解析命令行参数
/// error: 定义的错误类型，用于错误传递
use crate::{
    parse_paras::PARAS,
    html_page::create_download_page, // 生成chat记录页面html字符串
    error::MyError,
};

/// 生成音频时传输给用户端的图标base64
/// https://base64.run/
pub const VOICE: &str = include_str!("../../assets/image/voice-one-svgrepo-com.txt");

/// 信息类型
#[derive(Serialize, Deserialize, PartialEq)]
pub enum DataType {
    Raw(String),   // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
    Image(String), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
    Voice,         // 音频文件
    Normal,        // 常规问题
}

/// 问答记录
#[derive(Serialize, Deserialize)]
pub struct ChatData {
    message: ChatMessage, // 问答记录，如果舍弃之前记录，则初始化时不读取之前的记录，否则先读取之前的记录
    time:    String,      // 问答记录的时间，记录messages中每条信息的时间，如果时回答则在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
    data:    DataType,    // 该问答记录的数据类型，比如网络搜索的内容、zip压缩包提取的代码、图片base64
}

impl ChatData {
    fn new(message: ChatMessage, time: String, data: DataType) -> Self {
        ChatData{message, time, data}
    }
}

/// 记录用户信息
#[derive(Serialize, Deserialize)]
pub struct Info {
    pub uuid:       String,               // 每个用户一个uuid，如果指定了之前的uuid，则不重新生成，实现对话隔离，https://github.com/uuid-rs/uuid
    pub chat_name:  String,               // 创建对话时，可以输入该对话的名称，方便在相关uuid下拉选项中选择，并作为保存的chat记录文件名
    pub messages:   Vec<ChatData>,        // 问答记录
    //pub messages:   Vec<ChatMessage>,     // 问答记录，如果舍弃之前记录，则初始化时不读取之前的记录，否则先读取之前的记录
    //pub time:       Vec<String>,          // 问答记录的时间，记录messages中每条信息的时间，如果时回答则在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
    //pub query:      Vec<String>,          // 问答记录的原始问题，使用`web `进行网络搜索或解析url、html，或zip压缩包代码时，记录原始输入的内容，而不是最终解析的内容，不使用`web `或`code `则为空字符串，这样在页面加载之前chat记录时，只显示用户提问的内容，不显示中间搜索解析的内容
    pub file:       String,               // 存储chat记录的文件，格式：`指定输出路径/uuid/时间戳.log`，这里的时间戳是本次访问的时间
    pub token:      [usize;2],            // 提问和答案的token数，注意提问的token数不是计算messages中每个提问的token数，因为提问时可能会带上之前的message，因此要比messages中所有提问的token数多
    pub prompt:     Option<ChatMessage>,  // 该uuid所用的prompt
    pub prompt_str: Option<[String; 2]>,  // 该uuid所用的prompt的名称(用于显示在页面左侧)和内容(用于显示在页面右侧)
    pub num_q:      usize,                // 记录当前uuid客户端提交的问题数量，用于服务端命令行显示
    pub qa_msg_p:   (usize, usize, bool), // 第1项表示限制问答对的数量，第2项表示限制消息的数量，第3项表示每次提问是否包含prompt。注意前2项只有一个生效，0表示不使用
    pub save:       bool,                 // 是否需要保存该uuid的chat记录，如果只是提问，没有实际调用OpenAI的api进行回答，则最后退出程序时不需要保存该uuid的chat记录，只有本次开启服务后该uuid实际调用OpenAI的api得到回答这里才设为true
    pub pop:        usize,                // 如果只是提问而没有实际调用OpenAI api获取答案，则舍弃最后的连续的提问，这里记录要从messages最后移除的message数量，最后是答案则该值重置为0，否则累加连续的问题数
}

/// 实现Info的方法
impl Info {
    /// 根据指定uuid创建Info对象
    fn new(uuid: &str, chat_name: Option<String>) -> Self {
        // 路径`指定输出路径/uuid`不存在则创建
        if let Err(e) = create_uuid_dir(uuid) {
            event!(Level::ERROR, "{}", e);
        }
        // 对话名称
        let tmp_chat_name = match chat_name {
            Some(c) => c,
            None => "".to_string(),
        };
        // 创建chat记录输出文件，每次开启服务，uuid都会生成新的时间戳作为chat记录输出文件名，因此同一uuid路径下可能会有多个不同时间戳的chat记录文件
        let tmp_chat_file = format!("{}/{}/{}.log", PARAS.outpath, uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()); // 存储chat记录的文件，格式：指定输出路径/uuid/时间戳.log，例如：`2024-04-04_12-49-50.log`
        // 初始化Info对象
        Info {
            uuid:       uuid.to_string(),               // 每个用户一个uuid，如果指定了之前的uuid，则不重新生成，实现对话隔离，https://github.com/uuid-rs/uuid
            chat_name:  tmp_chat_name,                  // 创建对话时，可以输入该对话的名称，方便在相关uuid下拉选项中选择，并作为保存的chat记录文件名
            messages:   vec![],                         // 问答记录
            //messages:   vec![],                         // 问答记录，如果舍弃之前记录，则初始化时不读取之前的记录，否则先读取之前的记录
            //time:       vec![],                         // 问答记录的时间，记录messages中每条信息的时间，如果时回答则在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
            //query:      vec![],                         // 问答记录的原始问题，使用`web `进行网络搜索或解析url、html，或zip压缩包代码时，记录原始输入的内容，而不是最终解析的内容，不使用`web `或`code `则为空字符串，这样在页面加载之前chat记录时，只显示用户提问的内容，不显示中间搜索解析的内容
            file:       tmp_chat_file,                  // 存储chat记录的文件，格式：`指定输出路径/uuid/时间戳.log`，这里的时间戳是本次访问的时间
            token:      [0, 0],                         // 提问和答案的token数，注意提问的token数不是计算messages中每个提问的token数，因为提问时可能会带上之前的message，因此要比messages中所有提问的token数多
            prompt:     None,                           // 该uuid所用的prompt
            prompt_str: None,                           // 该uuid所用的prompt的名称(用于显示在页面左侧)和内容(用于显示在页面右侧)
            num_q:      0,                              // 记录当前uuid客户端提交的问题数量，用于服务端命令行显示
            qa_msg_p:   (usize::MAX, usize::MAX, true), // 第1项表示限制问答对的数量，第2项表示限制消息的数量，第3项表示每次提问是否包含prompt。注意前2项只有一个生效，0表示不使用
            save:       false,                          // 是否需要保存该uuid的chat记录，如果只是提问，没有实际调用OpenAI的api进行回答，则最后退出程序时不需要保存该uuid的chat记录，只有本次开启服务后该uuid实际调用OpenAI的api得到回答这里才设为true
            pop:        0,                              // 如果只是提问而没有实际调用OpenAI api获取答案，则舍弃最后的连续的提问，这里记录要从messages最后移除的message数量，最后是答案则该值重置为0，否则累加连续的问题数
        }
    }

    /// 读取指定uuid的log文件，不存在或报错则初始化空Info
    fn load_or_init(uuid: &str, chat_name: Option<String>) -> Self {
        let tmp_log_file = get_latest_file(format!("{}/{}/", PARAS.outpath, uuid), ".log");
        if tmp_log_file.is_empty() {
            Info::new(uuid, chat_name)
        } else {
            match read_to_string(&tmp_log_file) {
                Ok(s) => {
                    match serde_json::from_str(&s) {
                        Ok(s) => s,
                        Err(e) => {
                            event!(Level::ERROR, "{} warning: load previous chat log string to json error: {:?}", uuid, e);
                            Info::new(uuid, chat_name)
                        },
                    }
                },
                Err(e) => {
                    event!(Level::ERROR, "{} warning: read log file {} to string error: {:?}", uuid, tmp_log_file, e);
                    Info::new(uuid, chat_name)
                },
            }
        }
    }

    // 从messages最后移除pop数量个message，跳转uuid或保存chat记录前调用该方法
    fn pop_message(&mut self) {
        if self.pop > 0 {
            if self.prompt.is_some() && self.pop == self.messages.len() { // 有prompt，且要去除的数量与总message数相同，则保留第一个message，即prompt
                for _ in 0..self.pop-1 {
                    self.messages.pop();
                }
            } else {
                for _ in 0..self.pop {
                    self.messages.pop();
                }
            }
            self.pop = 0; // pop完成后需要将pop归零
        }
    }

    /// 将当前Info对象保存至本地json文件中
    fn save(&mut self) -> Result<(), MyError> {
        // 从messages最后移除pop数量个message
        self.pop_message();
        // 是否保存重置为false，下次加载时就是false
        self.save = false;
        // Info对象转json字符串
        let chat_log_json_str = serde_json::to_string(&self).map_err(|e| MyError::ToJsonStirngError{uuid: self.uuid.clone(), error: e})?;
        // 保存chat记录
        write(&self.file, chat_log_json_str).map_err(|e| MyError::WriteFileError{file: self.file.clone(), error: e})
    }

    /// 从messages中提取所有的message，返回Vec<ChatMessage>
    fn get_inner_messages(&self, skip_pre: usize, skip_suf: usize) -> Vec<ChatMessage> {
        if skip_pre == 0 && skip_suf == 0 {
            self.messages.iter().map(|m| m.message.clone()).collect()
        } else {
            //self.messages.iter().skip(skip_pre).map(|m| m.message.clone()).collect()
            self.messages[skip_pre..(self.messages.len()-skip_suf)].iter().map(|m| m.message.clone()).collect()
        }
    }

    /// 根据限制的问答对数量，获取(要忽略前几个消息数, 要保留的消息数, 最后要忽略的连续回答数)
    /// 一对问答对可以有连续多个问题，以及连续多条答案，例如下面的示例，question1和answer4之间的多个消息都属于一对问答：
    /// +----------------------+
    /// |            question1 | 可能一条信息没有把问题描述完
    /// |            question2 | 又接着发了一条补充说明
    /// |            question3 | 又接着发了一条补充说明
    /// | answer1              | 获取的答案不满意
    /// | answer2              | 换个模型又回答一次
    /// | answer3              | 再换个模型回答一次
    /// | answer4              | 再换个模型回答一次
    /// +----------------------+
    /// 有2点需要注意：
    /// 1. 最后一个信息不是问题而是回答：说明上次回答之后，用户没有输入新问题，而是直接又发起请求，此时将忽略最后1个回答或连续的多个回答，用最后一个问题继续提问。比如用户对答案不满意，更换了模型基于同一问题再问一次，这样就省去再输入一次问题
    /// 2. 如果连续的答案不在最后，而是在整个对话的中间：此时会把一个问题对应的连续多个回答视为一对问答
    /// 比如下面示例：
    /// 如果正常情况0要获取2对问答信息，则会获取到4条信息作为上下文：question2 + question3 + answer2 + question4
    /// 如果特殊情况1要获取2对问答信息，则会获取到4条信息作为上下文：question1 + answer1 + question2 + question3
    /// 如果特殊情况2要获取3对问答信息，则会获取到8条信息作为上下文：question1 + answer1 + question2 + question3 + answer2 + answer3 + answer4 + question4
    /// +------------------------------------------------------------------------+
    /// |             0                      1                      2            |
    /// | +----------------------+----------------------+----------------------+ |
    /// | |            question1 |            question1 |            question1 | |
    /// | | answer1              | answer1              | answer1              | |
    /// | |            question2 |            question2 |            question2 | |
    /// | |            question3 |            question3 |            question3 | |
    /// | | answer2              | answer2              | answer2              | |
    /// | |            question4 | answer3              | answer3              | |
    /// | +----------------------| answer4              | answer4              | |
    /// |                        +----------------------|            question4 | |
    /// |                                               +----------------------+ |
    /// +------------------------------------------------------------------------+
    /// 正常情况0有3对问答对话：第1对（question1 + answer1）、第2对（question2 + answer2）、第3对（question3）
    /// 特殊情况1有2对问答对话：第1对（question1 + answer1）、第2对（question2 + answer2 + answer3 + answer4）
    /// 特殊情况2有3对问答对话：第1对（question1 + answer1）、第2对（question2 + answer2 + answer3 + answer4）、第3对（question3）
    fn context_msg_num_by_qa(&self) -> (usize, usize, usize) {
        if self.qa_msg_p.0 == 0 || self.qa_msg_p.0 == usize::MAX { // 不通过问答对限制，或不限制问答对
            (0, self.messages.len(), 0)
        } else {
            let mut keep_qa_num = 0; // 要保留的问答对数量
            let mut keep_msg_num = 0; // 要保留的消息数量
            let mut skip_last_answer_num = 0; // 要忽略的最后连续一个或多个的回答数量
            let mut is_answer = false; // 是否是回答
            for m in self.messages.iter().rev() {
                if let &ChatMessage::Assistant{..} = &m.message {
                    if keep_qa_num == 0 { // 该回答是最后一对问答的回答
                        if keep_msg_num == 0 { // 最后一个信息是回答（或连续多个都是回答），用户在最后一个回答之后没有输入新问题，此时用户可能对最后一个问题的答案（一个或连续多个）不满意，要对最后一个问题再回答一次
                            skip_last_answer_num += 1;
                            continue
                        } else { // 最后一个信息不是回答，用户在最后一个回答之后输入了新问题；或者用户在最后一个回答之后没有输入新问题，想要对最后一个问题再回答一次
                            keep_qa_num = 2; // 此时还没有获取新答案，但也要算一对Q&A，因此这里设为2。比如`self.qa_msg_p.0`是1，则最终keep_msg_num就是最后一个回答之后的所有问题
                        }
                    } else {
                        if !is_answer { // 这里is_answer是true说明上一条信息也是回答，连续回答视为一对问答，因此只统计最后一个，即最后一个回答和问题，以及中间其他回答算作一对问答
                            keep_qa_num += 1; // 一对完整问答只统计最后一个回答，中间其他回答不统计
                        }
                    }
                    if keep_qa_num > self.qa_msg_p.0 {
                        break
                    }
                    if !is_answer {
                        is_answer = true;
                    }
                } else if is_answer {
                    is_answer = false;
                }
                keep_msg_num += 1;
            }
            (self.messages.len() - keep_msg_num - skip_last_answer_num, keep_msg_num, skip_last_answer_num)
        }
    }

    /// 根据限制的消息数量，获取(要忽略前几个消息数, 要保留的消息数, 最后要忽略的连续回答数)
    /// 直接按照消息数统计，就没有按照问答对那么麻烦了，有1点需要注意：
    /// 最后一个信息不是问题而是回答：说明上次回答之后，用户没有输入新问题，而是直接又发起请求，此时将忽略最后1个回答或连续的多个回答，用最后一个问题继续提问。比如用户对答案不满意，更换了模型基于同一问题再问一次，这样就省去再输入一次问题
    /// 比如下面示例：
    /// 如果正常情况0要获取3条信息，则会获取到：question3 + answer2 + question4
    /// 如果特殊情况1要获取3条信息，则会获取到：answer1 + question2 + question3
    /// +-------------------------------------------------+
    /// |             0                      1            |
    /// | +----------------------+----------------------+ |
    /// | |            question1 |            question1 | |
    /// | | answer1              | answer1              | |
    /// | |            question2 |            question2 | |
    /// | |            question3 |            question3 | |
    /// | | answer2              | answer2              | |
    /// | |            question4 | answer3              | |
    /// | +----------------------| answer4              | |
    /// |                        +----------------------+ |
    /// +-------------------------------------------------+
    fn context_msg_num(&self) -> (usize, usize, usize) {
        if self.qa_msg_p.1 == 0 || self.qa_msg_p.1 == usize::MAX { // 不通过消息数限制，或不限制消息数
            (0, self.messages.len(), 0)
        } else {
            let mut keep_msg_num = 0; // 要保留的消息数量
            let mut skip_last_answer_num = 0; // 要忽略的最后连续一个或多个的回答数量
            for m in self.messages.iter().rev() {
                if let &ChatMessage::Assistant{..} = &m.message {
                    if keep_msg_num == 0 { // 最后一个信息是回答，用户在最后一个回答之后没有输入新问题，此时用户可能对最后一个问题的答案（一个或连续多个）不满意，要对最后一个问题再回答一次
                        skip_last_answer_num += 1;
                        continue
                    }
                }
                keep_msg_num += 1;
                if keep_msg_num >= self.qa_msg_p.1 {
                    break
                }
            }
            (self.messages.len() - keep_msg_num - skip_last_answer_num, keep_msg_num, skip_last_answer_num)
        }
    }
}

/// 全局变量，可以修改，存储每个用户uuid的对话记录
pub static DATA: Lazy<Mutex<HashMap<String, Info>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 向DATA中指定uuid中插入新ChatMessage，uuid不存在则创建
pub fn insert_message(uuid: &str, message: ChatMessage, time: String, query: DataType, qa_msg_p: Option<(usize, usize, bool)>, model: &str, chat_name: Option<String>) {
    let mut data = DATA.lock().unwrap();
    // 如果指定uuid不在服务端，则从本地log文件加载或创建新Info对象
    if !data.contains_key(uuid) {
        // 从本地log文件加载或创建新Info对象
        data.insert(uuid.to_string(), Info::load_or_init(uuid, chat_name));
        // 更新刚插入的uuid的prompt，以及名称和内容
        if let Some(prompt_name_str) = get_prompt_from_file(uuid) {
            let info = data.get_mut(uuid).unwrap();
            info.prompt = Some(ChatMessage::User{
                content: ChatMessageContent::Text(prompt_name_str[1].clone()),
                name: None,
            });
            info.prompt_str = Some(prompt_name_str);
        }
    }
    let info = data.get_mut(uuid).unwrap();
    // 在插入新message之前先更新限制的问答对数量、限制的消息数量、提问是否包含prompt
    if let Some((qa, msg, with_prompt)) = qa_msg_p {
        if qa != info.qa_msg_p.0 || msg != info.qa_msg_p.1 || with_prompt != info.qa_msg_p.2 { // 客户端下拉选项`上下文消息数`改变时才更新限制的问答对数量、限制的消息数量、提问是否包含prompt
            info.qa_msg_p.0 = qa;
            info.qa_msg_p.1 = msg;
            info.qa_msg_p.2 = with_prompt;
        }
    }
    // 更新问题数和最后是否保存该uuid的chat记录
    match message {
        ChatMessage::User{..} => {
            info.num_q += 1;
            info.pop += 1; // 累加最后的连续问题数
        },
        _ => {
            info.save = true; // 不是用户输入的问题，则最后停止程序前需要保存该uuid的chat记录
            info.pop = 0; // 新插入的是答案，pop重置为0
        },
    }
    // 插入本次的message、时间、原始问题
    if qa_msg_p.is_some() { // 目前用户提出的问题都是Some，不需要加模型名称
        info.messages.push(ChatData::new(message, time, query));
    } else { // 目前模型回答的内容都是None
        info.messages.push(ChatData::new(message, format!("{} {}", time, model), query)); // 在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
    }
}

/// 在跳转到其他uuid或下载该chat记录之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
pub fn pop_message_before_end(uuid: &str) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        info.pop_message();
    }
}

/// 获取指定uuid客户端提交的问题数量，用于服务端命令行显示
pub fn get_query_num(uuid: &str) -> usize {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => { // uuid已存在
            info.num_q
        },
        None => { // uuid不存在
            0
        },
    }
}

/// 判断指定uuid是否已存在于DATA中
pub fn contain_uuid(uuid: &str) -> bool {
    let mut data = DATA.lock().unwrap();
    if data.contains_key(uuid) {
        true
    } else { // 不存在则尝试从服务端加载
        if get_latest_log_file(uuid).is_empty() { // 该uuid在服务端没有chat记录
            false
        } else { // 该uuid在服务端有chat记录，则加载
            data.insert(uuid.to_string(), Info::load_or_init(uuid, None));
            true
        }
    }
}

/// 从DATA中删除指定uuid
pub fn remove_uuid(uuid: &str) {
    let mut data = DATA.lock().unwrap();
    if data.contains_key(uuid) {
        data.remove(uuid);
    }
}

/// 从DATA中获取指定uuid的ChatMessage
/// info.qa_msg_p.2表示是否将prompt作为第一个message，不计算在问答对或消息数量内，即最终返回`1个prompt + num个问答对`或`1个prompt + num个message`
/// update_token: 是否将计算获取到的messages的token，并更新到该uuid的输入总token中
pub fn get_messages(uuid: &str, update_token: bool) -> Vec<ChatMessage> {
    let mut data = DATA.lock().unwrap();
    match data.get_mut(uuid) {
        Some(info) => {
            let final_messages = if info.qa_msg_p.0 == usize::MAX && info.qa_msg_p.1 == usize::MAX { // 没有对问答对或消息数进行限制
                info.get_inner_messages(0, 0)
            } else { // 通过问答对或消息数进行了限制，需要跳过前指定数量个消息
                // 总消息数
                let total_num = info.messages.len();
                // 获取(要忽略前几个消息数, 要保留的消息数, 最后要忽略的连续回答数)
                // 理论上`skip_msg_num`可能为0，但不可能等于总消息数，`keep_msg_num`肯定大于0，最大就是总消息数
                let (skip_msg_num, keep_msg_num, skip_last_answer_num) = if info.qa_msg_p.0 > 0 && info.qa_msg_p.0 < usize::MAX { // 对问答对数量进行限制
                    info.context_msg_num_by_qa()
                } else if info.qa_msg_p.1 > 0 && info.qa_msg_p.1 < usize::MAX { // 对消息数量进行限制
                    info.context_msg_num()
                } else {
                    unreachable!()
                };
                // 获取要保留的消息
                let mut messages: Vec<ChatMessage> = info.get_inner_messages(skip_msg_num, skip_last_answer_num);
                // 把prompt插入到第一位
                if info.qa_msg_p.2 {
                    if let Some(p) = &info.prompt {
                        if total_num != keep_msg_num { // 把prompt插入到第一位，如果相等则已经包含了prompt则不必再插入
                            messages.insert(0, p.clone());
                        }
                    }
                }
                messages
            };
            if update_token { // 计算获取到的上下文所有问题和答案的总token，加到输入总token上，因为这些上下文都要发给api
                let tokens = token_count_messages(&final_messages);
                info.token[0] += tokens[0]+tokens[1];
            }
            final_messages
        },
        None => vec![],
    }
}

/// 获取指定uuid的messages总数
pub fn get_messages_num(uuid: &str) -> usize {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => info.messages.len(),
        None => 0,
    }
}

/// 将DATA中指定uuid的chat记录保存至本地json文件中
/// 文件名为：`时间戳.log`
pub fn save_log(uuid: &str) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        // 保存chat记录
        if let Err(e) = info.save() {
            event!(Level::ERROR, "{} save chat log error: {}", uuid, e);
        }
    }
}

/// 遍历所有uuid，保存相应chat记录
pub fn save_all_chat() {
    let mut uuid_vec: Vec<(String, String)> = vec![]; // 存储保存了log的uuid及文件名，用于保存这些uuid的html文件，因为生成html文件需要lock，不能和log一起保存
    // 保存log文件
    let mut data = DATA.lock().unwrap();
    for (k, v) in data.iter_mut() {
        if v.save { // 如果只是提问，没有实际调用OpenAI的api进行回答，则最后退出程序时不需要保存该uuid的chat记录，只有本次开启服务后该uuid实际调用OpenAI的api得到回答这里才是true
            if let Err(e) = v.save() {
                event!(Level::ERROR, "{} save chat log error: {}", k, e);
            }
            uuid_vec.push((k.to_string(), v.file.clone()));
        }
    }
    drop(data); // 下面获取html字符串的`create_download_page`函数内部需要进行lock，这里需要手动释放之前的lock
    // 保存html文件
    for (uuid, log_file) in uuid_vec {
        let html_str = create_download_page(&uuid, None);
        if let Err(e) = write(log_file.replace(".log", ".html"), html_str) {
            event!(Level::ERROR, "{} save chat log to html error: {}", uuid, e);
        }
    }
    event!(Level::INFO, "save all chat log done");
}

/// 创建cookie，默认1天后过期，过期后客户端再次发送请求，则cookie将被更新
/// 默认uuid即为cookie值
pub fn create_cookie<'a>(v: String) -> Cookie<'a> {
    Cookie::build(("srx-tzn", v))
        //.secure(true)
        .same_site(SameSite::None) // Strict, Lax, None, 不设置则客户端浏览器会警告：由于 Cookie “srx-tzn”缺少正确的“sameSite”属性值，缺少“SameSite”或含有无效值的 Cookie 即将被视作指定为“Lax”，该 Cookie 将无法发送至第三方上下文中。若您的应用程序依赖这组 Cookie 以在不同上下文中工作，请添加“SameSite=None”属性。若要了解“SameSite”属性的更多信息，请参阅：https://developer.mozilla.org/docs/Web/HTTP/Headers/Set-Cookie/SameSite
        .http_only(true) // 设置为true会导致客户端浏览器无法通过js获取到cookie值，无法在页面显示，但是Chrome可以获取
        .path("/")
        .max_age(PARAS.maxage) // 默认cookie在客户端保留1天，1天之后需要指定uuid访问才能继续之前的chat记录，SECOND, MINUTE, HOUR, DAY, WEEK
        .build()
}

/// 更新cookie的max-age，用于在每次访问时都将max-age以当前时间为起始更新max-age
pub fn update_cookie_max_age(cjar: CookieJar) -> CookieJar {
    if let Some(mut cookie) = cjar.get("srx-tzn").cloned() {
        cookie.set_max_age(PARAS.maxage);
        cjar.add(cookie)
    } else {
        cjar        
    }
}

/// 获取当前uuid的问题和答案的总token数
pub fn get_token(uuid: &str) -> [usize; 2] {
    let mut data = DATA.lock().unwrap();
    match data.get_mut(uuid) {
        Some(info) => info.token,
        None => [0, 0],
    }
}

/// 更新当前uuid的token数
pub fn update_token_num(uuid: &str, n: usize, is_user: bool) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        if is_user {
            info.token[0] += n;
        } else {
            info.token[1] += n;
        }
    }
}

/// 计算指定字符串的token数，更新当前uuid的token数
pub fn update_token(uuid: &str, s: &str, is_user: bool) {
    update_token_num(uuid, token_count_str(s), is_user);
}

/// 计算指定字符串的token数
fn token_count_str(s: &str) -> usize {
    PARAS.bpe.encode_with_special_tokens(s).len()
}

/// 计算指定message的token数，以及是否是user
fn token_count_message(message: &ChatMessage) -> (usize, bool) {
    match message {
        ChatMessage::System{content, ..} => match content {
            ChatMessageContent::Text(t) => (token_count_str(&t), false),
            ChatMessageContent::ContentPart(res_vec) => {
                let mut tokens = 0;
                for res in res_vec {
                    match res {
                        ChatMessageContentPart::Text(t) => tokens += token_count_str(&t.text),
                        ChatMessageContentPart::Image(i) => tokens += token_count_str(&i.image_url.url),
                        ChatMessageContentPart::Audio(a) => tokens += token_count_str(&a.input_audio.data),
                    }
                }
                (tokens, false)
            },
            ChatMessageContent::None => (0, false),
        },
        ChatMessage::User{content, ..} => match content {
            ChatMessageContent::Text(t) => (token_count_str(&t), true),
            ChatMessageContent::ContentPart(res_vec) => {
                let mut tokens = 0;
                for res in res_vec {
                    match res {
                        ChatMessageContentPart::Text(t) => tokens += token_count_str(&t.text),
                        ChatMessageContentPart::Image(i) => tokens += token_count_str(&i.image_url.url),
                        ChatMessageContentPart::Audio(a) => tokens += token_count_str(&a.input_audio.data),
                    }
                }
                (tokens, true)
            },
            ChatMessageContent::None => (0, true),
        },
        ChatMessage::Assistant{content, ..} => match content {
            Some(c) => match c {
                ChatMessageContent::Text(t) => (token_count_str(&t), false),
                ChatMessageContent::ContentPart(res_vec) => {
                    let mut tokens = 0;
                    for res in res_vec {
                        match res {
                            ChatMessageContentPart::Text(t) => tokens += token_count_str(&t.text),
                            ChatMessageContentPart::Image(i) => tokens += token_count_str(&i.image_url.url),
                            ChatMessageContentPart::Audio(a) => tokens += token_count_str(&a.input_audio.data),
                        }
                    }
                    (tokens, false)
                },
                ChatMessageContent::None => (0, false),
            },
            None => (0, false),
        },
        ChatMessage::Developer{content, ..} => match content {
            ChatMessageContent::Text(t) => (token_count_str(&t), false),
            ChatMessageContent::ContentPart(res_vec) => {
                let mut tokens = 0;
                for res in res_vec {
                    match res {
                        ChatMessageContentPart::Text(t) => tokens += token_count_str(&t.text),
                        ChatMessageContentPart::Image(i) => tokens += token_count_str(&i.image_url.url),
                        ChatMessageContentPart::Audio(a) => tokens += token_count_str(&a.input_audio.data),
                    }
                }
                (tokens, false)
            },
            ChatMessageContent::None => (0, false),
        },
        ChatMessage::Tool{content, ..} => (token_count_str(&content), false),
    }
}

/// 计算指定Vec<ChatMessage>中问题和答案的token数
fn token_count_messages(messages: &Vec<ChatMessage>) -> [usize; 2] {
    let mut token_in_out: [usize; 2] = [0, 0];
    for message in messages {
         match token_count_message(message) {
            (n, true)  => token_in_out[0] += n,
            (n, false) => token_in_out[1] += n,
         }
    }
    token_in_out
}

/// 获取指定输出路径下最近的指定格式后缀的文件路径，文件名为时间戳，例如：`2024-04-04_12-49-50.指定格式后缀`
pub fn get_latest_file(p: String, suffix: &str) -> String {
    let tmp_outpath = Path::new(&p);
    if tmp_outpath.exists() && tmp_outpath.is_dir() {
        match tmp_outpath.read_dir() {
            Ok(entrys) => {
                let mut tmp_file = "".to_string(); // 获取时间戳最新的文件
                for entry in entrys {
                    if let Ok(file) = entry {
                        if file.path().is_file() {
                            if let Some(f) = file.path().file_name() {
                                if let Some(s) = f.to_str() {
                                    if s.ends_with(suffix) {
                                        // 检查字符串是否是时间戳，时间戳格式为`2024-04-04_12-49-50.指定格式后缀`
                                        // 这里使用`use chrono::NaiveDateTime;`的`parse_from_str`直接从字符串中解析时间，如果失败则表示不含有日期
                                        if let Ok(_) = NaiveDateTime::parse_from_str(s.strip_suffix(suffix).unwrap(), "%Y-%m-%d_%H-%M-%S") {
                                            tmp_file = format!("{}/{}", p, s);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                tmp_file
            },
            Err(e) => {
                event!(Level::INFO, "warning: read_dir {} error: {:?}", p, e);
                "".to_string()
            },
        }
    } else {
        "".to_string()
    }
}

/// 获取指定输出路径下最近的chat记录文件路径，例如：`2024-04-04_12-49-50.log`
fn get_latest_log_file(uuid: &str) -> String {
    get_latest_file(format!("{}/{}", PARAS.outpath, uuid), ".log")
}

/// 返回指定uuid路径下`speech.mp3`的路径，如果不存在则返回空字符串
pub fn get_speech_file(uuid: &str) -> String {
    let tmp_speech = format!("{}/{}/speech.mp3", PARAS.outpath, uuid);
    let tmp_path = Path::new(&tmp_speech);
    if tmp_path.exists() && tmp_path.is_file() {
        tmp_speech
    } else {
        "".to_string()
    }
}

/// 获取prompt名称和内容，不存在则返回空字符串
pub fn get_prompt(p: usize) -> [String; 2] {
    match PARAS.prompt.get(&p) {
        Some(prompt) => prompt.clone(),
        None => ["".to_string(), "".to_string()],
    }
}

/// 读取服务端`指定输出路径/uuid/prompt.txt`（其中写着该uuid使用的prompt的序号），获取该uuid的prompt的序号，然后用该序号去获取prompt的名称和内容
/// 序号从0开始，0表示无prompt
fn get_prompt_from_file(uuid: &str) -> Option<[String; 2]> {
    let tmp = format!("{}/{}/prompt.txt", PARAS.outpath, uuid);
    let tmp_path = Path::new(&tmp);
    if tmp_path.exists() && tmp_path.is_file() {
        match read_to_string(&tmp_path) {
            Ok(p) => {
                match p.parse::<usize>() {
                    Ok(n) => {
                        let prompt_name_str = get_prompt(n);
                        if prompt_name_str[0].is_empty() {
                            None
                        } else {
                            Some(prompt_name_str)
                        }
                    },
                    Err(e) => {
                        event!(Level::INFO, "{} warning: parse {} -> usize error: {:?}", uuid, p, e);
                        None
                    },
                }
            },
            Err(e) => {
                event!(Level::INFO, "{} warning: read_to_string {} error: {:?}", uuid, tmp, e);
                None
            },
        }
    } else {
        None
    }
}

/// 获取当前uuid的prompt名称
pub fn get_prompt_name(uuid: &str) -> String {
    let mut data = DATA.lock().unwrap();
    if !data.contains_key(uuid) { // 该uuid不在服务端，则尝试从服务端指定路径加载
        if get_latest_log_file(uuid).is_empty() { // 该uuid在服务端也没有chat记录文件
            return "no prompt".to_string()
        } else { // 该uuid在服务端有chat记录，则加载
            // 从本地log文件加载该uuid的Info对象
            data.insert(uuid.to_string(), Info::load_or_init(uuid, None));
        }
    }
    match &data.get(uuid).unwrap().prompt_str { // 此时该uuid一定在服务端data中，这里直接unwrap
        Some(name_str) => {
            if name_str[0].is_empty() {
                "no prompt".to_string()
            } else {
                name_str[0].clone()
            }
        },
        None => "no prompt".to_string(),
    }
}

/// 将之前问答记录显示到页面
pub struct DisplayInfo {
    pub is_query: bool,   // 是否是提问
    pub content:  String, // 问题或答案字符串
    pub id:       String, // 作为html中tag的id的序号
    pub time:     String, // 时间
    pub is_img:   bool,   // 是否是图片base64
    pub is_voice: bool,   // 是否是语音base64
}

/// 读取指定uuid最新问答记录，提取字符串，用于在chat页面显示
/// 注意如果是网络搜索的问题或zip压缩包代码，则不使用message中的内容，而是用记录的原始提问内容
/// 如果该uuid是新创建的，指定了prompt，则显示prompt，没指定prompt，则显示示例问答
/// for_template: 是否是给模板使用，即访问chat页面使用于模板渲染
/// 如果是true则需要将“`”替换为“\\”，“</scrip”替换为“/scrip”
/// 如果是false则需要将“\n”替换为“srxtzn”
pub fn get_log_for_display(uuid: &str, for_template: bool) -> (usize, Vec<DisplayInfo>) {
    //let mut logs: Vec<(bool, String, String, String)> = vec![]; // (是否是提问, 问题或答案字符串, 作为html中tag的id的序号, 时间)
    let mut logs: Vec<DisplayInfo> = vec![]; // 是否是提问、问题或答案字符串、作为html中tag的id的序号、时间、是否是图片base64、是否是语音base64
    // 获取指定uuid的chat记录
    let mut data = DATA.lock().unwrap();
    if !data.contains_key(uuid) { // 该uuid不在服务端，则尝试从服务端指定路径加载
        data.insert(uuid.to_string(), Info::load_or_init(uuid, None));
    }
    let info = data.get_mut(uuid).unwrap(); // 此时该uuid一定在服务端data中，这里直接unwrap
    for (i, m) in info.messages.iter().enumerate() {
        let tmp_time = m.time.clone();
        let tmp_id = format!("m{}", i);
        match &m.message {
            ChatMessage::System{content, ..} => match content {
                ChatMessageContent::Text(t) => {
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((false, t.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, t.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\n", "srxtzn"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    }
                },
                ChatMessageContent::ContentPart(res_vec) => {
                    let mut all_res = "".to_string();
                    for res in res_vec {
                        match res {
                            ChatMessageContentPart::Text(text) => all_res += &text.text,
                            ChatMessageContentPart::Image(image) => {
                                all_res += &image.image_url.url;
                                all_res += "\n";
                            },
                            ChatMessageContentPart::Audio(audio) => all_res += &audio.input_audio.data,
                        }
                    }
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((false, all_res.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  all_res.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, all_res.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  all_res.replace("\n", "srxtzn"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    }
                },
                ChatMessageContent::None => logs.push(DisplayInfo{is_query: false, content: "".to_string(), id: tmp_id, time: tmp_time, is_img: false, is_voice: false}),
            },
            ChatMessage::User{content, ..} => match content {
                ChatMessageContent::Text(t) => {
                    let (tmp, is_img) = match &m.data {
                        DataType::Raw(s) => (s.clone(), false), // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
                        DataType::Image(s) => (s.clone(), true), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
                        DataType::Normal | DataType::Voice => (t.clone(), false), // 常规问题
                    };
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((true, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((true, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\n", "srxtzn"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                        });
                    }
                },
                ChatMessageContent::ContentPart(res_vec) => {
                    let (tmp, is_img) = match &m.data {
                        DataType::Raw(s) => (s.clone(), false), // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
                        DataType::Image(s) => (s.clone(), true), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
                        DataType::Normal | DataType::Voice => { // 常规问题
                            let mut all_res = "".to_string();
                            for res in res_vec {
                                match res {
                                    ChatMessageContentPart::Text(text) => all_res += &text.text,
                                    ChatMessageContentPart::Image(image) => {
                                        all_res += &image.image_url.url;
                                        all_res += "\n";
                                    },
                                    ChatMessageContentPart::Audio(audio) => all_res += &audio.input_audio.data,
                                }
                            }
                            (all_res, false)
                        },
                    };
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((true, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((true, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\n", "srxtzn"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                        });
                    }
                },
                ChatMessageContent::None => logs.push(DisplayInfo{is_query: true, content: "".to_string(), id: tmp_id, time: tmp_time, is_img: false, is_voice: false}),
            },
            ChatMessage::Assistant{content, ..} => match content {
                Some(c) => match c {
                    ChatMessageContent::Text(t) => {
                        let (tmp, is_img, is_voice) = match &m.data {
                            DataType::Raw(s) => (s.clone(), false, false), // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
                            DataType::Image(s) => (s.clone(), true, false), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
                            DataType::Voice => (VOICE.to_string(), false, true), // 传输音频图标base64
                            DataType::Normal => (t.clone(), false, false), // 常规问题
                        };
                        if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                            //logs.push((false, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                                id:       tmp_id,
                                time:     tmp_time,
                                is_img,
                                is_voice,
                            });
                        } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                            //logs.push((false, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\n", "srxtzn"),
                                id:       tmp_id,
                                time:     tmp_time,
                                is_img,
                                is_voice,
                            });
                        }
                    },
                    ChatMessageContent::ContentPart(res_vec) => {
                        let (tmp, is_img) = match &m.data {
                            DataType::Raw(s) => (s.clone(), false), // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
                            DataType::Image(s) => (s.clone(), true), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
                            DataType::Normal | DataType::Voice => { // 常规问题
                                let mut all_res = "".to_string();
                                for res in res_vec {
                                    match res {
                                        ChatMessageContentPart::Text(text) => all_res += &text.text,
                                        ChatMessageContentPart::Image(image) => {
                                            all_res += &image.image_url.url;
                                            all_res += "\n";
                                        },
                                        ChatMessageContentPart::Audio(audio) => all_res += &audio.input_audio.data,
                                    }
                                }
                                (all_res, false)
                            },
                        };
                        if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                            //logs.push((false, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                                id:       tmp_id,
                                time:     tmp_time,
                                is_img,
                                is_voice: false,
                            });
                        } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                            //logs.push((false, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\n", "srxtzn"),
                                id:       tmp_id,
                                time:     tmp_time,
                                is_img,
                                is_voice: false,
                            });
                        }
                    },
                    ChatMessageContent::None => logs.push(DisplayInfo{is_query: false, content: "".to_string(), id: tmp_id, time: tmp_time, is_img: false, is_voice: false}),
                },
                None => (),
            },
            ChatMessage::Developer{content, ..} => match content {
                ChatMessageContent::Text(t) => {
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((false, t.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, t.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\n", "srxtzn"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    }
                },
                ChatMessageContent::ContentPart(res_vec) => {
                    let mut all_res = "".to_string();
                    for res in res_vec {
                        match res {
                            ChatMessageContentPart::Text(text) => all_res += &text.text,
                            ChatMessageContentPart::Image(image) => {
                                all_res += &image.image_url.url;
                                all_res += "\n";
                            },
                            ChatMessageContentPart::Audio(audio) => all_res += &audio.input_audio.data,
                        }
                    }
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((false, all_res.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  all_res.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, all_res.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  all_res.replace("\n", "srxtzn"),
                            id:       tmp_id,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                        });
                    }
                },
                ChatMessageContent::None => logs.push(DisplayInfo{is_query: false, content: "".to_string(), id: tmp_id, time: tmp_time, is_img: false, is_voice: false}),
            },
            ChatMessage::Tool{content, ..} => logs.push(DisplayInfo{is_query: false, content: content.clone(), id: tmp_id, time: tmp_time, is_img: false, is_voice: false}),
        }
    }
    // 如果该uuid是新建的，且指定了prompt，只是还没有保存对话，则写入prompt
    if logs.len() == 0 {
        if let Some(p) = &info.prompt_str { // 该uuid有prompt，则展示prompt
            if !p[1].is_empty() {
                if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                    //logs.push((true, p[1].replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), "m0".to_string(), info.messages[0].time.clone()));
                    logs.push(DisplayInfo{
                        is_query: true,
                        content:  p[1].replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                        id:       "m0".to_string(),
                        time:     info.messages[0].time.clone(),
                        is_img:   false,
                        is_voice: false,
                    });
                } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                    //logs.push((true, p[1].replace("\n", "srxtzn"), "m0".to_string(), info.messages[0].time.clone()));
                    logs.push(DisplayInfo{
                        is_query: true,
                        content:  p[1].replace("\n", "srxtzn"),
                        id:       "m0".to_string(),
                        time:     info.messages[0].time.clone(),
                        is_img:   false,
                        is_voice: false,
                    });
                }
            }
        }
    }
    let logs_num = logs.len(); // 总message数，不把下面示例消息计算在内，因此下面示例消息的id设为“m-序号”，而真实message的id设为“m序号”
    // 如果该uuid没有之前的chat记录，也不是新建的有prompt的uuid，则写入默认对话
    if logs.len() == 0 {
        // 问题1
        logs.push(DisplayInfo{
            is_query: true,
            content:  "Hello".to_string(),
            id:       "m-0".to_string(),
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
        });
        // 回答1
        logs.push(DisplayInfo{
            is_query: false,
            content:  "Hello! How are you doing today? If there's anything you'd like to discuss or ask, feel free to let me know.".to_string(),
            id:       "m-1".to_string(),
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
        });
        // 问题2
        logs.push(DisplayInfo{
            is_query: true,
            content:  "what is chatgpt?".to_string(),
            id:       "m-2".to_string(),
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
        });
        // 回答2
        logs.push(DisplayInfo{
            is_query: false,
            content:  "ChatGPT is a conversational AI model developed by OpenAI. It's based on the GPT (Generative Pre-trained Transformer) architecture, specifically designed to understand and generate natural language text. ChatGPT can engage in conversations, answer questions, provide explanations, and assist with a wide range of inquiries. It's trained on diverse datasets from the internet, allowing it to generate human-like responses on various topics. However, it doesn't have real-time access to current events or the ability to browse the web, and its knowledge is based on information available up to its last training cut-off.".to_string(),
            id:       "m-3".to_string(),
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
        });
    }
    (logs_num, logs)
}

/// 计算指定字符串中含有的非英文字符的比例，不考虑数字和ASCII内的特殊字符（-=？&*等）
pub fn chinese_ratio(s: &str) -> f64 {
    let mut total: f64 = 0.0;
    let mut chinese: f64 = 0.0;
    for c in s.chars() {
        if c.is_ascii() {
            if c.is_ascii_alphabetic() { // a-z和A-Z，不包括特殊字符和数字
                total += 1.0;
            }
        } else { // 视为中文
            total += 1.0;
            chinese += 1.0;
        }
    }
    chinese/total
}

/// 判断指定字符串是否是指定uuid中的文件，如果是则读取内容
pub fn try_read_file(uuid: &str, s: &str) -> String {
    if s.is_empty() {
        "".to_string()
    } else {
        let tmp_file = format!("{}/{}/{}", PARAS.outpath, uuid, s);
        let tmp_path = Path::new(&tmp_file);
        if tmp_path.exists() && tmp_path.is_file() { // 检查是否存在于服务端
            /*
            match read_to_string(&tmp_file) {
                Ok(q) => q,
                Err(e) => {
                    event!(Level::INFO, "{} warning: read_to_string {} error: {:?}", uuid, tmp_file, e);
                    "".to_string()
                },
            }
            */
            // 上面方法遇到无效UTF-8字符会报错，这里将无效UTF-8字符替换为“�”
            // https://stackoverflow.com/questions/61221763/how-can-i-get-the-content-of-a-file-if-it-isnt-contain-a-valid-utf-8
            // https://doc.rust-lang.org/beta/std/fs/fn.read.html
            match read(&tmp_file) { // 相当于`File::open`和`read_to_end`
                Ok(s) => String::from_utf8_lossy(&s).to_string(), // 文件中含有的无效UTF-8字符会被替换为“�”，即`REPLACEMENT_CHARACTER`，表示无效字符
                Err(e) => {
                    event!(Level::INFO, "{} warning: fs::read {} error: {:?}", uuid, tmp_file, e);
                    "".to_string()
                },
            }
        } else {
            "".to_string()
        }
    }
}

/// uuid文件夹不存在则创建
pub fn create_uuid_dir(uuid: &str) -> Result<(), MyError> {
    let tmp = format!("{}/{}", PARAS.outpath, uuid);
    let tmp_path = Path::new(&tmp);
    if !(tmp_path.exists() && tmp_path.is_dir()) {
        create_dir_all(&tmp).map_err(|e| MyError::CreateDirAllError{dir_name: tmp, error: e})?;
    }
    Ok(())
}

/// 获取指定uuid对话的名称
pub fn get_chat_name(uuid: &str) -> String {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => info.chat_name.clone(),
        None => "".to_string(),
    }
}

/// 获取保存chat记录时的文件名
/// 如果该对话创建时指定了对话名称，且对话名称可以作为文件名，则“chat_log_对话名称_uuid.html”，否则“chat_log_uuid.html”
/// Windows不能含有：['<', '>', ':', '"', '/', '\\', '|', '?', '*']
/// Linux不能含有：['/']
pub fn valid_filename(uuid: &str) -> String {
    let data = DATA.lock().unwrap();
    let info = data.get(uuid).unwrap(); // 调用该函数则该uuid一定在服务端data中，这里直接unwrap
    if info.chat_name.is_empty() {
        format!("chat_log_{}.html", uuid)
    } else {
        // 无效字符
        let invalid_chars = if cfg!(windows) {
            // Windows 不允许的字符
            vec!['<', '>', ':', '"', '/', '\\', '|', '?', '*']
        } else {
            // Unix-like 系统不允许的字符
            vec!['/', '\0']
        };
        // 检查指定的对话名称是否含有无效字符
        if info.chat_name.chars().any(|c| invalid_chars.contains(&c)) {
            format!("chat_log_{}.html", uuid)
        } else {
            let tmp_name = format!("chat_log_{}_{}.html", info.chat_name, uuid);
            // 检查文件名长度是否超过系统限制
            if tmp_name.len() > 255 {
                format!("chat_log_{}.html", uuid)
            } else {
                tmp_name
            }
        }
    }
}

/// 获取最后一个message，且必须是用户发送的query字符串
pub fn get_latest_query(uuid: &str) -> Option<String> {
    let data = DATA.lock().unwrap();
    let info = data.get(uuid).unwrap(); // 调用该函数则该uuid一定在服务端data中，这里直接unwrap
    if let Some(m) = info.messages.last() { // 最后一个message
        if let ChatMessage::User{content, ..} = &m.message { // 必须是User
            match &m.data {
                DataType::Raw(s) => Some(s.clone()),
                DataType::Normal => {
                    if let ChatMessageContent::Text(c) = content {
                        Some(c.clone())
                    } else {
                        None
                    }
                },
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// 获取最后一个上传的图片
pub fn get_latest_image(uuid: &str) -> Option<String> {
    let data = DATA.lock().unwrap();
    let info = data.get(uuid).unwrap(); // 调用该函数则该uuid一定在服务端data中，这里直接unwrap
    for m in info.messages.iter().rev() { // 反序遍历
        if let ChatMessage::User{content, ..} = &m.message {
            if let (DataType::Image(_), ChatMessageContent::Text(c)) = (&m.data, content) {
                return Some(c.clone()) // 返回图片名称，该图片上传存储在服务端当前uuid路径下
            }
        }
    }
    None
}

/// 获取最后一个上传的音频文件
pub fn get_latest_voice(uuid: &str) -> Option<String> {
    let data = DATA.lock().unwrap();
    let info = data.get(uuid).unwrap(); // 调用该函数则该uuid一定在服务端data中，这里直接unwrap
    for m in info.messages.iter().rev() { // 反序遍历
        if let ChatMessage::User{content, ..} = &m.message {
            if let (DataType::Voice, ChatMessageContent::Text(c)) = (&m.data, content) {
                return Some(c.clone()) // 返回音频文件名称，该音频文件上传存储在服务端当前uuid路径下
            }
        }
    }
    None
}

/// 获取指定uuid对话中，指定索引对应message的图片或音频文件名（包含路径），以及是否是音频，提供给用户下载
pub fn get_file_for_download(uuid: &str, idx: usize) -> Option<(String, bool)> {
    let data = DATA.lock().unwrap();
    let info = data.get(uuid).unwrap(); // 调用该函数则该uuid一定在服务端data中，这里直接unwrap
    if info.messages.len() > idx {
        match info.messages[idx].data {
            DataType::Image(_) | DataType::Voice => { // 图片或音频文件
                if let ChatMessage::Assistant{content, ..} = &info.messages[idx].message { // 这里是Assistant，即只提供下载Assistant生成的图片，用户上传的图片不需要下载
                    if let Some(ChatMessageContent::Text(c)) = content {
                        Some((format!("{}/{}/{}", PARAS.outpath, uuid, c), info.messages[idx].data == DataType::Voice)) // 返回：(图片或音频文件名称, 是否是音频)，该文件存储在服务端当前uuid路径下
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
            _ => None, // 其他类型不需要下载
        }
    } else { // 索引出界
        None
    }
}
