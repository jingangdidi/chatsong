use std::collections::HashMap;
use std::fs::{read, write, create_dir_all, read_to_string};
use std::path::Path;
use std::sync::Mutex;

use axum_extra::extract::cookie::{Cookie, SameSite, CookieJar};
use chrono::{Local, NaiveDateTime};
use image::ImageReader;
use once_cell::sync::Lazy;
use openai_dive::v1::resources::chat::{
    ChatMessage,
    ChatMessageContent,
    ChatMessageContentPart,
    ChatMessageImageContentPart,
    ImageUrlType,
};
use serde::{Serialize, Deserialize};
use tracing::{event, Level};

/// parse_paras: 解析命令行参数
/// error: 定义的错误类型，用于错误传递
use crate::{
    parse_paras::PARAS,
    graph::graph_remove_uuid,
    html_page::create_download_page, // 生成chat记录页面html字符串
    error::MyError,
};

/// 生成音频时传输给用户端的图标base64
/// https://base64.run/
pub const VOICE: &str = include_str!("../../assets/image/voice-one-svgrepo-com.txt");

/// 信息类型
#[derive(Serialize, Deserialize, PartialEq)]
pub enum DataType {
    Raw(String),                   // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
    Image(String),                 // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
    Voice,                         // 音频文件
    Normal,                        // 常规问题
    Hide((usize, Option<String>)), // 隐藏该信息，(隐藏前DataType的索引, 隐藏前存储的字符串)，该信息被用户删除了，显示chat记录、获取上下文时忽略该信息
}

impl DataType {
    // 该数据类型是否是Hide
    fn is_hide(&self) -> bool {
        if let DataType::Hide(_) = self {
            true
        } else {
            false
        }
    }

    // 将当前DataType转为DataType::Hide，并返回是否做了转换
    fn to_hide(&mut self) -> bool {
        match self {
            DataType::Raw(r)   => {
                *self = DataType::Hide((0, Some(r.to_owned())));
                true
            },
            DataType::Image(i) => {
                *self = DataType::Hide((1, Some(i.to_owned())));
                true
            },
            DataType::Voice    => {
                *self = DataType::Hide((2, None));
                true
            },
            DataType::Normal   => {
                *self = DataType::Hide((3, None));
                true
            },
            DataType::Hide(_)  => false, // 已经隐藏过了
        }
    }

    // 将当前DataType::Hide还原回原始DataType
    // 这个以后可能用到
    /*
    fn restore_hide(&mut self) {
        match self {
            DataType::Hide((0, Some(r))) => *self = DataType::Raw(r.to_owned()),
            DataType::Hide((1, Some(i))) => *self = DataType::Image(i.to_owned()),
            DataType::Hide((2, None))    => *self = DataType::Voice,
            DataType::Hide((3, None))    => *self = DataType::Normal,
            DataType::Hide((4, _))       => (), // 原始就是隐藏信息，不应该出现这种情况
            _                            => (), // 不应该出现这种情况
        }
    }
    */
}

/// 问答记录
#[derive(Serialize, Deserialize)]
pub struct ChatData {
    id:      usize,       // 该信息的id，这个id是包含隐藏信息的序号，为了避免遍历获取到的不含隐藏信息的多个信息时，直接使用索引序号出现id不对应问题
    message: ChatMessage, // 问答记录，如果舍弃之前记录，则初始化时不读取之前的记录，否则先读取之前的记录
    time:    String,      // 问答记录的时间，记录messages中每条信息的时间，如果时回答则在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
    data:    DataType,    // 该问答记录的数据类型，比如网络搜索的内容、zip压缩包提取的代码、图片base64
    is_web:  bool,        // 是否网络搜索
    idx_qa:  usize,       // 该message属于第几个Q&A对
    idx_m:   usize,       // 该message属于第几条信息
    token:   u32,         // 该message的token数
}

impl ChatData {
    fn new(uuid: &str, id: usize, message: ChatMessage, time: String, data: DataType, is_web: bool, idx_qa: usize, idx_m: usize) -> Self {
        // 计算token数
        let token = if let (DataType::Image(_), Some(name)) = (&data, get_image_name(&message)) {
            // Qwen3-vl图片token: `width / 32 * height / 32`
            // Qwen2.5-vl图片token: `width / 28 * height / 28`
            // https://github.com/QwenLM/Qwen3-VL/issues/1238
            // https://github.com/QwenLM/Qwen3-VL
            let image_file = format!("{}/{}/{}", PARAS.outpath, uuid, name);
            let image_file_path = Path::new(&image_file);
            if image_file_path.exists() && image_file_path.is_file() {
                match ImageReader::open(&image_file) {
                    Ok(img) => match img.decode() {
                        Ok(dec) => dec.width() * dec.height() / 1024, // Qwen3-vl tokens
                        Err(e) => {
                            event!(Level::ERROR, "{} decode image error: {:?}", uuid, e);
                            0
                        },
                    },
                    Err(e) => {
                        event!(Level::ERROR, "{} read image error: {:?}", uuid, e);
                        0
                    },
                }
            } else {
                0
            }
        } else {
            0
        };
        //ChatData{message, time: if is_web {format!("🌐 {time}")} else {time}, data, idx_qa, token} // 不管用，页面不显示emoji
        ChatData{id, message, time, data, is_web, idx_qa, idx_m, token}
    }

    /// convert uploaded image to User
    fn get_real_message(&self) -> ChatMessage {
        if let DataType::Image(b64) = &self.data {
            ChatMessage::User {
                content: ChatMessageContent::ContentPart(vec![ChatMessageContentPart::Image(
                    ChatMessageImageContentPart {
                        r#type: "image_url".to_string(),
                        image_url: ImageUrlType {
                            url: b64.clone(), // Either a URL of the image or the base64 encoded image data
                            detail: None,
                        },
                    },
                )]),
                name: None,
            }
        } else {
            self.message.clone()
        }
    }
}

/// 记录用户信息
#[derive(Serialize, Deserialize)]
pub struct Info {
    pub uuid:          String,               // 每个用户一个uuid，如果指定了之前的uuid，则不重新生成，实现对话隔离，https://github.com/uuid-rs/uuid
    pub chat_name:     String,               // 创建对话时，可以输入该对话的名称，方便在相关uuid下拉选项中选择，并作为保存的chat记录文件名
    pub messages:      Vec<ChatData>,        // 问答记录
    pub msg_len:       usize,                // 当前messages的总数，排除了DataType是Hide的message，因此不要使用`messages.len()`获取总信息数
    //pub messages:      Vec<ChatMessage>,     // 问答记录，如果舍弃之前记录，则初始化时不读取之前的记录，否则先读取之前的记录
    //pub time:          Vec<String>,          // 问答记录的时间，记录messages中每条信息的时间，如果时回答则在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
    //pub query:         Vec<String>,          // 问答记录的原始问题，使用`web `进行网络搜索或解析url、html，或zip压缩包代码时，记录原始输入的内容，而不是最终解析的内容，不使用`web `或`code `则为空字符串，这样在页面加载之前chat记录时，只显示用户提问的内容，不显示中间搜索解析的内容
    pub file:          String,               // 存储chat记录的文件，格式：`uuid/时间戳.log`，这里的时间戳是本次访问的时间
    pub token:         [u32;2],              // 提问和答案的token数，注意提问的token数不是计算messages中每个提问的token数，因为提问时可能会带上之前的message，因此要比messages中所有提问的token数多
    pub context_token: u32,                  // context token
    pub prompt:        Option<ChatMessage>,  // 该uuid所用的prompt
    pub prompt_str:    Option<[String; 2]>,  // 该uuid所用的prompt的名称(用于显示在页面左侧)和内容(用于显示在页面右侧)
    pub num_q:         (usize, usize),       // 记录当前uuid用户发送的是第几个message（不是总消息数）以及属于第几对Q&A
    pub qa_msg_p:      (usize, usize, bool), // 第1项表示限制问答对的数量，第2项表示限制消息的数量，第3项表示每次提问是否包含prompt。注意前2项只有一个生效，0表示不使用
    pub save:          bool,                 // 是否需要保存该uuid的chat记录，如果只是提问，没有实际调用OpenAI的api进行回答，则最后退出程序时不需要保存该uuid的chat记录，只有本次开启服务后该uuid实际调用OpenAI的api得到回答这里才设为true
    pub pop:           usize,                // 如果只是提问而没有实际调用OpenAI api获取答案，则舍弃最后的连续的提问，这里记录要从messages最后移除的message数量，最后是答案则该值重置为0，否则累加连续的问题数
    pub is_incognito:  bool,                 // 是否无痕模式，true则关闭服务时不保存该对话，直接舍弃，如果是基于之前保存的对话继续提问，则本次新的问答不会保存；false则像常规对话那样，关闭服务时保存至本地
    pub approved:      Option<bool>,         // call tool approval
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
        let tmp_chat_file = format!("{}/{}.log", uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()); // 存储chat记录的文件，格式：uuid/时间戳.log，例如：`2024-04-04_12-49-50.log`
        // 初始化Info对象
        Info {
            uuid:          uuid.to_string(),               // 每个用户一个uuid，如果指定了之前的uuid，则不重新生成，实现对话隔离，https://github.com/uuid-rs/uuid
            chat_name:     tmp_chat_name,                  // 创建对话时，可以输入该对话的名称，方便在相关uuid下拉选项中选择，并作为保存的chat记录文件名
            messages:      vec![],                         // 问答记录
            msg_len:       0,                              // 当前messages的总数，排除了DataType是Hide的message，因此不要使用`messages.len()`获取总信息数
            //messages:      vec![],                         // 问答记录，如果舍弃之前记录，则初始化时不读取之前的记录，否则先读取之前的记录
            //time:          vec![],                         // 问答记录的时间，记录messages中每条信息的时间，如果时回答则在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
            //query:         vec![],                         // 问答记录的原始问题，使用`web `进行网络搜索或解析url、html，或zip压缩包代码时，记录原始输入的内容，而不是最终解析的内容，不使用`web `或`code `则为空字符串，这样在页面加载之前chat记录时，只显示用户提问的内容，不显示中间搜索解析的内容
            file:          tmp_chat_file,                  // 存储chat记录的文件，格式：`uuid/时间戳.log`，这里的时间戳是本次访问的时间
            token:         [0, 0],                         // 提问和答案的token数，注意提问的token数不是计算messages中每个提问的token数，因为提问时可能会带上之前的message，因此要比messages中所有提问的token数多
            context_token: 0,                              // context token
            prompt:        None,                           // 该uuid所用的prompt
            prompt_str:    None,                           // 该uuid所用的prompt的名称(用于显示在页面左侧)和内容(用于显示在页面右侧)
            num_q:         (0, 0),                         // 记录当前uuid用户发送的是第几个message（不是总消息数）以及属于第几对Q&A
            qa_msg_p:      (usize::MAX, usize::MAX, true), // 第1项表示限制问答对的数量，第2项表示限制消息的数量，第3项表示每次提问是否包含prompt。注意前2项只有一个生效，0表示不使用
            save:          false,                          // 是否需要保存该uuid的chat记录，如果只是提问，没有实际调用OpenAI的api进行回答，则最后退出程序时不需要保存该uuid的chat记录，只有本次开启服务后该uuid实际调用OpenAI的api得到回答这里才设为true
            pop:           0,                              // 如果只是提问而没有实际调用OpenAI api获取答案，则舍弃最后的连续的提问，这里记录要从messages最后移除的message数量，最后是答案则该值重置为0，否则累加连续的问题数
            is_incognito:  false,                          // 是否无痕模式，true则关闭服务时不保存该对话，直接舍弃，如果是基于之前保存的对话继续提问，则本次新的问答不会保存；false则像常规对话那样，关闭服务时保存至本地
            approved:      None,                           // call tool approval
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
                    match serde_json::from_str::<Self>(&s) {
                        Ok(mut s) => {
                            // 这里要更新msg_len
                            s.msg_len = s.messages.iter().filter(|m| !m.data.is_hide()).count();
                            // 这里要更新num_q的qa数
                            s.num_q.1 = s.get_qa_num_by_idx(s.messages.len()-1).0;
                            // 更新每个message的idx_qa（该message属于第几个Q&A对）和idx_m（该message属于第几条信息）
                            s.update_qa_msg_idx();
                            s
                        },
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
                    // 更新非Hide信息数
                    if !self.messages[self.messages.len()-1].data.is_hide() {
                        self.msg_len -= 1;
                    }
                    // 删除最后的信息
                    self.messages.pop();
                }
            } else {
                for _ in 0..self.pop {
                    // 更新非Hide信息数
                    if !self.messages[self.messages.len()-1].data.is_hide() {
                        self.msg_len -= 1;
                    }
                    // 删除最后的信息
                    self.messages.pop();
                }
            }
            // pop完成后需要将pop归零
            self.pop = 0;
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
        // 加上指定的输出路径
        let file_with_path = format!("{}/{}", PARAS.outpath, self.file);
        // 保存chat记录
        write(&file_with_path, chat_log_json_str).map_err(|e| MyError::WriteFileError{file: file_with_path, error: e})
    }

    /// 从messages中提取所有的message，返回Vec<ChatMessage>
    /// 这里skip_pre和skip_suf不会考虑信息是否是hide，直接对总messages进行截取，截取后的信息再过滤掉hide信息
    fn get_inner_messages(&self, skip_pre: usize, skip_suf: usize) -> Vec<ChatMessage> {
        if skip_pre == 0 && skip_suf == 0 {
            //self.messages.iter().map(|m| m.message.clone()).collect()
            self.messages.iter().filter(|m| !m.data.is_hide()).map(|m| m.get_real_message()).collect() // 过滤掉hide的信息
        } else {
            //self.messages.iter().skip(skip_pre).map(|m| m.message.clone()).collect()
            //self.messages[skip_pre..(self.messages.len()-skip_suf)].iter().map(|m| m.message.clone()).collect()
            self.messages[skip_pre..(self.messages.len()-skip_suf)].iter().filter(|m| !m.data.is_hide()).map(|m| m.get_real_message()).collect() // 先截取信息，然后再过滤掉截取后的信息中hide的信息
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
            let mut is_answer = false; // 是否是回答

            let mut keep_msg_num = 0; // 要保留的消息数量
            let mut keep_msg_hide_num = 0; // 要保留的消息数量对应的隐藏信息数

            let mut skip_last_answer_num = 0; // 要忽略的最后连续一个或多个的回答数量
            let mut skip_last_answer_hide_num = 0; // 要忽略的最后连续一个或多个的回答数量对应的隐藏信息数

            for m in self.messages.iter().rev() {
                if let &ChatMessage::Assistant{..} = &m.message {
                    if keep_qa_num == 0 { // 该回答是最后一对问答的回答
                        if keep_msg_num == 0 { // 最后一个信息是回答（或连续多个都是回答），用户在最后一个回答之后没有输入新问题，此时用户可能对最后一个问题的答案（一个或连续多个）不满意，要对最后一个问题再回答一次
                            if m.data.is_hide() {
                                skip_last_answer_hide_num += 1;
                            } else {
                                skip_last_answer_num += 1;
                            }
                            continue
                        } else { // 最后一个信息不是回答，用户在最后一个回答之后输入了新问题；或者用户在最后一个回答之后没有输入新问题，想要对最后一个问题再回答一次
                            if !m.data.is_hide() {
                                keep_qa_num = 2; // 此时还没有获取新答案，但也要算一对Q&A，因此这里设为2。比如`self.qa_msg_p.0`是1，则最终keep_msg_num就是最后一个回答之后的所有问题
                            }
                        }
                    } else {
                        if !is_answer && !m.data.is_hide() { // 这里is_answer是true说明上一条信息也是回答，连续回答视为一对问答，因此只统计最后一个，即最后一个回答和问题，以及中间其他回答算作一对问答
                            keep_qa_num += 1; // 一对完整问答只统计最后一个回答，中间其他回答不统计
                        }
                    }
                    if keep_qa_num > self.qa_msg_p.0 {
                        break
                    }
                    if !is_answer && !m.data.is_hide() {
                        is_answer = true;
                    }
                } else if is_answer && !m.data.is_hide() {
                    is_answer = false;
                }
                if m.data.is_hide() {
                    keep_msg_hide_num += 1;
                } else {
                    keep_msg_num += 1;
                }
            }
            (self.messages.len() - keep_msg_num - keep_msg_hide_num - skip_last_answer_num - skip_last_answer_hide_num, keep_msg_num + keep_msg_hide_num, skip_last_answer_num + skip_last_answer_hide_num)
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
            let mut keep_msg_hide_num = 0; // 要保留的消息数量对应的隐藏信息数

            let mut skip_last_answer_num = 0; // 要忽略的最后连续一个或多个的回答数量
            let mut skip_last_answer_hide_num = 0; // 要忽略的最后连续一个或多个的回答数量对应的隐藏信息数

            for m in self.messages.iter().rev() {
                if let &ChatMessage::Assistant{..} = &m.message {
                    if keep_msg_num == 0 { // 最后一个信息是回答，用户在最后一个回答之后没有输入新问题，此时用户可能对最后一个问题的答案（一个或连续多个）不满意，要对最后一个问题再回答一次
                        if m.data.is_hide() {
                            skip_last_answer_hide_num += 1;
                        } else {
                            skip_last_answer_num += 1;
                        }
                        continue
                    }
                }
                if m.data.is_hide() {
                    keep_msg_hide_num += 1;
                } else {
                    keep_msg_num += 1;
                }
                if keep_msg_num >= self.qa_msg_p.1 {
                    break
                }
            }
            (self.messages.len() - keep_msg_num - keep_msg_hide_num - skip_last_answer_num - skip_last_answer_hide_num, keep_msg_num + keep_msg_hide_num, skip_last_answer_num + skip_last_answer_hide_num)
        }
    }

    /*
    /// 获取下一条信息是第几对Q&A，指定的参数表示下一个message是否是问题
    /// 这种方法是从头统计一遍
    /// 如果最后一个message是回答，is_q为true则返回当前Q&A对数量+1，is_q为false则返回当前Q&A对数量
    /// 如果最后一个message是问题，is_q无效，返回当前Q&A对数量
    fn get_qa_num(&self, is_q: bool) -> usize {
        if self.messages.len() == 0 {
            1
        } else {
            let mut qa_num = 0; // 问答对数量
            let mut is_answer = false; // 是否是回答
            for m in self.messages.iter().rev() {
                if let &ChatMessage::Assistant{..} = &m.message {
                    if is_answer { // 上一条是回答，这一条还是回答，连续的回答属于同一QA对，不增加计数
                        continue
                    } else { // 上一条不是回答，这一条是回答，是新的QA对，计数加1
                        qa_num += 1;
                        is_answer = true;
                    }
                } else if is_answer {
                    is_answer = false;
                }
            }
            if let &ChatMessage::Assistant{..} = self.messages.last().unwrap().message { // 最后一条信息是回答
                if is_q { // 下一条插入的是问题，则QA对加1；下一条插入的是回答，则QA不变
                    qa_num += 1;
                }
            }
            qa_num
        }
    }
    */

    /// 计算指定索引位置信息是第几对Q&A，以及最后一条非隐藏的信息是否是问题
    /// 这种方法是从头统计一遍，因为可能信息被Hide了
    fn get_qa_num_by_idx(&self, idx: usize) -> (usize, bool) {
        if self.messages.len() == 0 || self.messages.iter().all(|m| m.data.is_hide()) {
            (0, false)
        } else {
            let mut qa_num = 0; // 问答对数量
            let mut is_answer = false; // 是否是回答
            let mut last_is_q = false; // 最后一条非隐藏的信息是否是问题
            for m in self.messages[0..=idx].iter().rev() {
                if !m.data.is_hide() {
                    if let &ChatMessage::Assistant{..} = &m.message {
                        if is_answer { // 上一条是回答，这一条还是回答，连续的回答属于同一QA对，不增加计数
                            continue
                        } else { // 上一条不是回答，这一条是回答，是新的QA对，计数加1
                            qa_num += 1;
                            is_answer = true;
                        }
                    } else { // 这是问题
                        if is_answer {
                            is_answer = false;
                        }
                        if qa_num == 0 { // 最后一条是问题，qa数至少是1
                            qa_num = 1;
                            last_is_q = true;
                        }
                    }
                }
            }
            (qa_num, last_is_q)
        }
    }

    /// 获取下一条信息是第几对Q&A，指定的参数表示下一个message是否是问题
    /// 如果最后一个message是回答，is_q为true则返回当前Q&A对数量+1，is_q为false则返回当前Q&A对数量
    /// 如果最后一个message是问题，is_q无效，返回当前Q&A对数量
    fn get_qa_num(&self, is_q: bool) -> usize {
        if self.messages.len() == 0 {
            1
        } else {
            /*
            // 这种方法只需要根据当前最后一条信息中存储的是第几个QA对，接着往上加1就可以，但前面信息如果被用户删除则需要更新
            let mut qa_num = 1;
            for m in self.messages.iter().rev() {
                if !m.data.is_hide() {
                    if let ChatMessage::Assistant{..} = m.message { // 最后一条信息是回答
                        if is_q { // 下一条插入的是问题，则下一条新的QA计数加1
                            qa_num = m.idx_qa + 1;
                        } else { // 下一条插入的是回答，则下一条信息父QA计数不变
                            qa_num = m.idx_qa;
                        }
                    } else { // 最后一条信息是问题，则下一条信息的QA计数不变
                        qa_num = m.idx_qa;
                    }
                    break
                }
            }
            */
            let (mut qa_num, last_is_q) = self.get_qa_num_by_idx(self.messages.len()-1);
            if !last_is_q && is_q { // 下一条插入的是问题，则QA对加1；下一条插入的是回答，则QA不变
                qa_num += 1;
            }
            qa_num
        }
    }

    /*
    /// 获取最后连续的问题数
    fn get_latest_query_num(&self) -> usize {
        let mut num = 0;
        for m in self.messages.iter().rev() {
            if let &ChatMessage::User{..} = &m.message {
                if !m.data.is_hide() {
                    num += 1;
                }
            } else {
                break
            }
        }
        num
    }
    */

    /// 将指定idx的信息设为隐藏，这样已经插入的信息的索引不变，前端id也不需要变，成功则返回true，失败返回false
    fn hide_msg(&mut self, idx: usize) -> bool {
        if self.messages.len() > idx {
            if self.messages[idx].data.to_hide() {
                // 比指定idx大的信息的idx_m都要减1，idx_qa要重新计算
                for i in (0..self.messages.len()).rev() {
                    if i <= idx {
                        break
                    } else if !self.messages[i].data.is_hide() {
                        self.messages[i].idx_m -= 1;
                        self.messages[i].idx_qa = self.get_qa_num_by_idx(i).0;
                    }
                }
                // 非Hide的信息数减1
                self.msg_len -= 1;
                true
            } else { // 该信息已经是Hide，没做转换，非Hide的信息数不变，也返回true，因为不是错误
                true
            }
        } else {
            false
        }
    }

    /// 更新每个message的idx_qa（该message属于第几个Q&A对）和idx_m（该message属于第几条信息）
    fn update_qa_msg_idx(&mut self) {
        let mut idx_m = 0;
        for i in 0..self.messages.len() {
            // 更新idx_m（该message属于第几条信息）
            self.messages[i].idx_qa = self.get_qa_num_by_idx(i).0;
            // 更新idx_qa（该message属于第几个Q&A对）
            if !self.messages[i].data.is_hide() {
                idx_m += 1;
                self.messages[i].idx_m = idx_m;
            }
        }
    }
}

/// 全局变量，可以修改，存储每个用户uuid的对话记录
pub static DATA: Lazy<Mutex<HashMap<String, Info>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 向DATA中指定uuid中插入新ChatMessage，uuid不存在则创建
pub fn insert_message(uuid: &str, message: ChatMessage, msg_token: Option<(u32, u32, u32)>, time: String, is_web: bool, query: DataType, qa_msg_p: Option<(usize, usize, bool)>, model: &str, chat_name: Option<String>) {
    let mut data = DATA.lock().unwrap();
    // 如果指定uuid不在服务端，则从本地log文件加载或创建新Info对象
    match data.get_mut(uuid) {
        Some(info) => match chat_name { // update chat name
            Some(n) => {
                if info.chat_name != n {
                    info.chat_name = n;
                }
            },
            None => {
                if !info.chat_name.is_empty() {
                    info.chat_name = "".to_string();
                }
            },
        },
        None => {
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
        },
    }
    let info = data.get_mut(uuid).unwrap();
    // 在插入新message之前先更新限制的问答对数量、限制的消息数量、提问是否包含prompt
    if let Some((qa, msg, with_prompt)) = qa_msg_p {
        // 更新限制的问答对数量
        if qa != info.qa_msg_p.0 {
            info.qa_msg_p.0 = qa;
        }
        // 更新限制的限制的消息数量
        if msg != info.qa_msg_p.1 {
            info.qa_msg_p.1 = msg;
        }
        // 更新提问是否包含prompt
        if with_prompt != info.qa_msg_p.2 {
            info.qa_msg_p.2 = with_prompt;
        }
    }
    // 获取下一条信息（即插入当前信息后）是第几对Q&A
    let qa_num = if qa_msg_p.is_some() { // 目前用户提出的问题都是Some
        info.get_qa_num(true)
    } else { // 目前模型回答的内容都是None
        info.get_qa_num(false)
    };
    // 更新问题数和最后是否保存该uuid的chat记录
    info.num_q.1 = qa_num;
    match message {
        ChatMessage::User{..} => {
            info.num_q.0 += 1;
            info.pop += 1; // 累加最后的连续问题数
        },
        _ => {
            info.save = true; // 不是用户输入的问题，则最后停止程序前需要保存该uuid的chat记录
            info.pop = 0; // 新插入的是答案，pop重置为0
        },
    }
    // 最后更新总信息数
    info.msg_len += 1;
    // 插入本次的message、时间、原始问题、是否网络搜索、message属于第几个Q&A对
    let mut chat_data = if qa_msg_p.is_some() { // 目前用户提出的问题都是Some，不需要加模型名称
        ChatData::new(uuid, info.messages.len(), message, time, query, is_web, qa_num, info.msg_len)
    } else { // 目前模型回答的内容都是None
        ChatData::new(uuid, info.messages.len(), message, format!("{} {}", time, model), query, is_web, qa_num, info.msg_len) // 在时间后面加上当前调用的模型名称，这样在同一对话中调用不同模型可以区分开
    };
    // 更新总输入、总输出、当前回复message的token数
    if let Some((prompt_tokens, completion_tokens, total_tokens)) = msg_token {
        chat_data.token = completion_tokens;
        info.token[0] += prompt_tokens;
        info.token[1] += completion_tokens;
        info.context_token = total_tokens;
    }
    // 插入message
    info.messages.push(chat_data);
}

/// update approval for call tools
pub fn update_approval(uuid: &str, approval: Option<bool>) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        info.approved = approval;
    }
}

/// check approved
pub fn approved(uuid: &str) -> Option<bool> {
    let data = DATA.lock().unwrap();
    if let Some(info) = data.get(uuid) {
        if let Some(tf) = &info.approved {
            Some(*tf)
        } else {
            None
        }
    } else {
        None
    }
}

/// update input total token, output total token, context token
pub fn update_token(uuid: &str, usage: (u32, u32, u32)) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        info.token[0] += usage.0;
        info.token[1] += usage.1;
        info.context_token = usage.2;
    }
}


/// get context token
pub fn get_context_token(uuid: &str) -> u32 {
    let data = DATA.lock().unwrap();
    if let Some(info) = data.get(uuid) {
        info.context_token
    } else {
        0
    }
}

/// 客户端下拉选项`上下文消息数`改变时更新限制的问答对数量、限制的消息数量、提问是否包含prompt
pub fn update_qa_msg_num(uuid: &str, qa_msg_p: Option<(usize, usize, bool)>) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        if let Some((qa, msg, with_prompt)) = qa_msg_p {
            // 更新限制的问答对数量
            if qa != info.qa_msg_p.0 {
                info.qa_msg_p.0 = qa;
            }
            // 更新限制的限制的消息数量
            if msg != info.qa_msg_p.1 {
                info.qa_msg_p.1 = msg;
            }
            // 更新提问是否包含prompt
            if with_prompt != info.qa_msg_p.2 {
                info.qa_msg_p.2 = with_prompt;
            }
        }
    }
}

/// 在跳转到其他uuid或下载该chat记录之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
pub fn pop_message_before_end(uuid: &str) {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        info.pop_message();
    }
}

/// 获取指定uuid客户端提交的问题数量，以及属于第几对Q&A，用于服务端命令行显示
pub fn get_query_num(uuid: &str) -> (usize, usize) {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => { // uuid已存在
            info.num_q
        },
        None => { // uuid不存在
            (0, 0)
        },
    }
}

// 从服务端指定uuid中删除指定id的信息，这里id格式是“d”+序号索引，比如“d0”表示第一条信息
// 成功则第1项返回true，失败第1项返回false，第2项返回错误信息
pub fn delete_msg_by_id(uuid: &str, id: &str) -> (bool, Option<String>) {
    let mut data = DATA.lock().unwrap();
    if let Some(id_num) = id.strip_prefix("d") { // 含有“d”前缀
        if let Ok(idx) = id_num.parse::<usize>() { // 是数值
            if let Some(info) = data.get_mut(uuid) {
                if info.hide_msg(idx) {
                    (true, None)
                } else {
                    (false, Some(format!("index {id} >= total messages number"))) // 索引出界
                }
            } else { // 服务端没有该uuid
                (false, Some(format!("uuid {uuid} not in server"))) // uuid不存在
            }
        } else { // 数值id转usize报错
            (false, Some(format!("convert id {id} to number error"))) // id转usize错误
        }
    } else { // 指定id不是以“d”开头
        (false, Some(format!("id {id} not starts with \"d\""))) // id第一个字符不是“d”
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
/*
pub fn remove_uuid(uuid: &str) {
    let mut data = DATA.lock().unwrap();
    if data.contains_key(uuid) {
        data.remove(uuid);
    }
}
*/

/// 从DATA中获取指定uuid的ChatMessage
/// info.qa_msg_p.2表示是否将prompt作为第一个message，不计算在问答对或消息数量内，即最终返回`1个prompt + num个问答对`或`1个prompt + num个message`
pub fn get_messages(uuid: &str) -> Vec<ChatMessage> {
    let mut data = DATA.lock().unwrap();
    match data.get_mut(uuid) {
        Some(info) => {
            if info.qa_msg_p.0 == usize::MAX && info.qa_msg_p.1 == usize::MAX { // 没有对问答对或消息数进行限制
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
            }
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

/// 检查指定uuid是否设置了无痕，如果是无痕，则清空该uuid的Info，返回是否已从服务的删除该uuid
pub fn check_incognito(uuid: &str) -> bool {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get(uuid) {
        if info.is_incognito {
            data.remove(uuid);
            // 还要从gragh中删除
            // 如果上次提问没设置无痕，再次跳转到该对话提问时设置了无痕，此时该uuid已经在gragh的相关uuid中，会出现在下拉uuid中，点击跳转会重新生成uuid，导致服务的与页面id不对应而报错
            graph_remove_uuid(uuid);
            true
        } else {
            false
        }
    } else {
        true
    }
}

/// 更新无痕模式，返回更新后的值
pub fn set_incognito(uuid: &str) -> Option<bool> {
    let mut data = DATA.lock().unwrap();
    if let Some(info) = data.get_mut(uuid) {
        info.is_incognito = !info.is_incognito;
        Some(info.is_incognito)
    } else {
        None
    }
}

/// 是否无痕模式
pub fn is_incognito(uuid: &str) -> bool {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => info.is_incognito,
        None => false,
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
        if v.save && !v.is_incognito { // 如果只是提问，没有实际调用OpenAI的api进行回答，则最后退出程序时不需要保存该uuid的chat记录，只有本次开启服务后该uuid实际调用OpenAI的api得到回答这里才是true
            if let Err(e) = create_uuid_dir(k) {
                event!(Level::ERROR, "{}", e);
            }
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
        // 由于在不同电脑间同步，保存路径可能不一致，因此在这里才加上路径前缀
        let file_with_path = format!("{}/{}", PARAS.outpath, log_file);
        if let Err(e) = write(file_with_path.replace(".log", ".html"), html_str) {
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
        .same_site(SameSite::Strict) // Strict, Lax, None, 不设置则客户端浏览器会警告：由于 Cookie “srx-tzn”缺少正确的“sameSite”属性值，缺少“SameSite”或含有无效值的 Cookie 即将被视作指定为“Lax”，该 Cookie 将无法发送至第三方上下文中。若您的应用程序依赖这组 Cookie 以在不同上下文中工作，请添加“SameSite=None”属性。若要了解“SameSite”属性的更多信息，请参阅：https://developer.mozilla.org/docs/Web/HTTP/Headers/Set-Cookie/SameSite
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

/// 获取当前uuid最后一个message的token数
/*
pub fn get_last_msg_token(uuid: &str) -> usize {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => match info.messages.last() {
            Some(m) => m.token,
            None => 0,
        },
        None => 0,
    }
}
*/

/// 获取当前uuid指定位置message的token数
/// pos>=0表示索引位置，pos<0表示倒数第几个，比如0表示第1个，1表示第2个，-1表示最后一个，-2表示倒数第个
pub fn get_msg_token(uuid: &str, pos: isize) -> u32 {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => {
            let idx = if pos >= 0 {
                pos as usize
            } else {
                info.messages.len() - (-pos) as usize
            };
            info.messages[idx].token
        },
        None => 0,
    }
}

/// 获取当前uuid的问题和答案的总token数
pub fn get_token(uuid: &str) -> [u32; 2] {
    let data = DATA.lock().unwrap();
    match data.get(uuid) {
        Some(info) => info.token,
        None => [0, 0],
    }
}

/// get image file name
fn get_image_name(message: &ChatMessage) -> Option<String> {
    match message {
        ChatMessage::System{content, ..} => if let ChatMessageContent::Text(t) = content {
            Some(t.clone())
        } else {
            None
        },
        ChatMessage::User{content, ..} => if let ChatMessageContent::Text(t) = content {
            Some(t.clone())
        } else {
            None
        },
        ChatMessage::Assistant{content, ..} => if let Some(ChatMessageContent::Text(t)) = content {
            Some(t.clone())
        } else {
            None
        },
        ChatMessage::Developer{content, ..} => if let ChatMessageContent::Text(t) = content {
            Some(t.clone())
        } else {
            None
        },
        ChatMessage::Tool{..} => None,
    }
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
pub fn get_latest_log_file(uuid: &str) -> String {
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
    pub id:       usize,  // 作为html中tag的id的序号
    pub time:     String, // 时间
    pub is_img:   bool,   // 是否是图片base64
    pub is_voice: bool,   // 是否是语音base64
    pub is_web:   bool,   // 是否网络搜索
    pub idx_qa:   usize,  // 该message属于第几个Q&A对
    pub idx_m:    usize,  // 该message属于第几条信息
    pub token:    u32,    // 该message的token数
}

/// 读取指定uuid最新问答记录，提取字符串，用于在chat页面显示
/// 注意如果是网络搜索的问题或zip压缩包代码，则不使用message中的内容，而是用记录的原始提问内容
/// 如果该uuid是新创建的，指定了prompt，则显示prompt，没指定prompt，则显示示例问答
/// for_template: 是否是给模板使用，即访问chat页面使用于模板渲染
/// 如果是true则需要将“`”替换为“\\”，“</scrip”替换为“/scrip”
/// 如果是false则需要将“\n”替换为“srxtzn”
/// 返回(下一个信息的id序号, 信息数, 问答对数量, 每条信息的内容)
pub fn get_log_for_display(uuid: &str, for_template: bool) -> (usize, usize, usize, Vec<DisplayInfo>) {
    //let mut logs: Vec<(bool, String, String, String)> = vec![]; // (是否是提问, 问题或答案字符串, 作为html中tag的id的序号, 时间)
    let mut logs: Vec<DisplayInfo> = vec![]; // 是否是提问、问题或答案字符串、作为html中tag的id的序号、时间、是否是图片base64、是否是语音base64
    // 获取指定uuid的chat记录
    let mut data = DATA.lock().unwrap();
    if !data.contains_key(uuid) { // 该uuid不在服务端，则尝试从服务端指定路径加载
        data.insert(uuid.to_string(), Info::load_or_init(uuid, None));
    }
    let info = data.get_mut(uuid).unwrap(); // 此时该uuid一定在服务端data中，这里直接unwrap
    for (i, m) in info.messages.iter().enumerate() {
        if m.data.is_hide() {
            continue
        }
        let tmp_time = m.time.clone();
        match &m.message {
            ChatMessage::System{content, ..} => match content {
                ChatMessageContent::Text(t) => {
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((false, t.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, t.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\n", "srxtzn"),
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
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
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, all_res.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  all_res.replace("\n", "srxtzn"),
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    }
                },
                ChatMessageContent::None => logs.push(DisplayInfo{is_query: false, content: "".to_string(), id: i, time: tmp_time, is_img: false, is_voice: false, is_web: m.is_web, idx_qa: m.idx_qa, idx_m: m.idx_m, token: m.token}),
            },
            ChatMessage::User{content, ..} => match content {
                ChatMessageContent::Text(t) => {
                    let (tmp, is_img) = match &m.data {
                        DataType::Raw(s) => (s.clone(), false), // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
                        DataType::Image(s) => (s.clone(), true), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
                        DataType::Normal | DataType::Voice => (t.clone(), false), // 常规问题
                        DataType::Hide(_) => unreachable!(),
                    };
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((true, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       i,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((true, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\n", "srxtzn"),
                            id:       i,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
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
                        DataType::Hide(_) => unreachable!(),
                    };
                    if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                        //logs.push((true, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                            id:       i,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((true, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: true,
                            content:  tmp.replace("\n", "srxtzn"),
                            id:       i,
                            time:     tmp_time,
                            is_img,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    }
                },
                ChatMessageContent::None => logs.push(DisplayInfo{is_query: true, content: "".to_string(), id: i, time: tmp_time, is_img: false, is_voice: false, is_web: m.is_web, idx_qa: m.idx_qa, idx_m: m.idx_m, token: m.token}),
            },
            ChatMessage::Assistant{content, ..} => match content {
                Some(c) => match c {
                    ChatMessageContent::Text(t) => {
                        let (tmp, is_img, is_voice) = match &m.data {
                            DataType::Raw(s) => (s.clone(), false, false), // 要进行网络搜索、解析url、解析上传的html、从上传的pdf提取内容、从上传的zip文件提取内容时，存储输入要搜索的问题、url、html文件名、pdf文件名、zip文件名。展示chat记录时展示这个内容，而不是搜索、解析、提取的内容
                            DataType::Image(s) => (s.clone(), true, false), // 图片base64字符串，该图片存储在服务端当前uuid路径下。上传的图片或生成的图片
                            DataType::Voice => (VOICE.to_string(), false, true), // 传输音频图标base64
                            DataType::Normal => (t.clone(), false, false), // 常规问题
                            DataType::Hide(_) => unreachable!(),
                        };
                        if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                            //logs.push((false, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                                id:       i,
                                time:     tmp_time,
                                is_img,
                                is_voice,
                                is_web:   m.is_web,
                                idx_qa:   m.idx_qa,
                                idx_m:    m.idx_m,
                                token:    m.token,
                            });
                        } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                            //logs.push((false, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\n", "srxtzn"),
                                id:       i,
                                time:     tmp_time,
                                is_img,
                                is_voice,
                                is_web:   m.is_web,
                                idx_qa:   m.idx_qa,
                                idx_m:    m.idx_m,
                                token:    m.token,
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
                            DataType::Hide(_) => unreachable!(),
                        };
                        if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                            //logs.push((false, tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                                id:       i,
                                time:     tmp_time,
                                is_img,
                                is_voice: false,
                                is_web:   m.is_web,
                                idx_qa:   m.idx_qa,
                                idx_m:    m.idx_m,
                                token:    m.token,
                            });
                        } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                            //logs.push((false, tmp.replace("\n", "srxtzn"), tmp_id, tmp_time));
                            logs.push(DisplayInfo{
                                is_query: false,
                                content:  tmp.replace("\n", "srxtzn"),
                                id:       i,
                                time:     tmp_time,
                                is_img,
                                is_voice: false,
                                is_web:   m.is_web,
                                idx_qa:   m.idx_qa,
                                idx_m:    m.idx_m,
                                token:    m.token,
                            });
                        }
                    },
                    ChatMessageContent::None => logs.push(DisplayInfo{is_query: false, content: "".to_string(), id: i, time: tmp_time, is_img: false, is_voice: false, is_web: m.is_web, idx_qa: m.idx_qa, idx_m: m.idx_m, token: m.token}),
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
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, t.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  t.replace("\n", "srxtzn"),
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
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
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                        //logs.push((false, all_res.replace("\n", "srxtzn"), tmp_id, tmp_time));
                        logs.push(DisplayInfo{
                            is_query: false,
                            content:  all_res.replace("\n", "srxtzn"),
                            id:       i,
                            time:     tmp_time,
                            is_img:   false,
                            is_voice: false,
                            is_web:   m.is_web,
                            idx_qa:   m.idx_qa,
                            idx_m:    m.idx_m,
                            token:    m.token,
                        });
                    }
                },
                ChatMessageContent::None => logs.push(DisplayInfo{is_query: false, content: "".to_string(), id: i, time: tmp_time, is_img: false, is_voice: false, is_web: m.is_web, idx_qa: m.idx_qa, idx_m: m.idx_m, token: m.token}),
            },
            ChatMessage::Tool{content: ChatMessageContent::Text(content_str), ..} => logs.push(DisplayInfo{is_query: false, content: content_str.clone(), id: i, time: tmp_time, is_img: false, is_voice: false, is_web: m.is_web, idx_qa: m.idx_qa, idx_m: m.idx_m, token: m.token}),
            _ => unreachable!(),
        }
    }
    // 如果该uuid是新建的，且指定了prompt，只是还没有保存对话，则写入prompt
    let m_num = if logs.len() == 0 {
        if let Some(p) = &info.prompt_str { // 该uuid有prompt，则展示prompt
            if !p[1].is_empty() {
                if for_template { // 给模板使用，注意这里对“`”做转义，因为js代码中两个“`”之间的字符串可以含有多行，“{”和“}”也做转义，html的“<script>”标签中的js代码中不能出现“</script>”，否则会报错，因此这里也对“</script>”做修改
                    //logs.push((true, p[1].replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"), "m0".to_string(), info.messages[0].time.clone()));
                    logs.push(DisplayInfo{
                        is_query: true,
                        content:  p[1].replace("\\", "\\\\").replace("`", "\\`").replace("{", "\\{").replace("}", "\\}").replace("</scrip", "/scrip"),
                        id:       0,
                        time:     info.messages[0].time.clone(),
                        is_img:   false,
                        is_voice: false,
                        is_web:   false,
                        idx_qa:   1,
                        idx_m:    1,
                        token:    0,
                    });
                } else { // 通过stream响应给客户端，需要将`\n`替换为`srxtzn`，客户端js会替换回来
                    //logs.push((true, p[1].replace("\n", "srxtzn"), "m0".to_string(), info.messages[0].time.clone()));
                    logs.push(DisplayInfo{
                        is_query: true,
                        content:  p[1].replace("\n", "srxtzn"),
                        id:       0,
                        time:     info.messages[0].time.clone(),
                        is_img:   false,
                        is_voice: false,
                        is_web:   false,
                        idx_qa:   1,
                        idx_m:    1,
                        token:    0,
                    });
                }
                1
            } else {
                0
            }
        } else {
            0
        }
    } else {
        logs.len()
    };
    // 如果该uuid没有之前的chat记录，也不是新建的有prompt的uuid，则写入默认对话
    if logs.len() == 0 {
        // 问题1
        logs.push(DisplayInfo{
            is_query: true,
            content:  "Hello".to_string(),
            id:       usize::MAX-3,
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
            is_web:   false,
            idx_qa:   0,
            idx_m:    0,
            token:    0,
        });
        // 回答1
        logs.push(DisplayInfo{
            is_query: false,
            content:  "Hello! How are you doing today? If there's anything you'd like to discuss or ask, feel free to let me know.".to_string(),
            id:       usize::MAX-2,
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
            is_web:   false,
            idx_qa:   0,
            idx_m:    0,
            token:    0,
        });
        // 问题2
        logs.push(DisplayInfo{
            is_query: true,
            content:  "what is chatgpt?".to_string(),
            id:       usize::MAX-1,
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
            is_web:   false,
            idx_qa:   0,
            idx_m:    0,
            token:    0,
        });
        // 回答2
        logs.push(DisplayInfo{
            is_query: false,
            content:  "ChatGPT is a conversational AI model developed by OpenAI. It's based on the GPT (Generative Pre-trained Transformer) architecture, specifically designed to understand and generate natural language text. ChatGPT can engage in conversations, answer questions, provide explanations, and assist with a wide range of inquiries. It's trained on diverse datasets from the internet, allowing it to generate human-like responses on various topics. However, it doesn't have real-time access to current events or the ability to browse the web, and its knowledge is based on information available up to its last training cut-off.".to_string(),
            id:       usize::MAX,
            time:     Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            is_img:   false,
            is_voice: false,
            is_web:   false,
            idx_qa:   0,
            idx_m:    0,
            token:    0,
        });
    }
    (info.messages.len(), m_num, info.get_qa_num(true)-1, logs)
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
    let mut last_q = None;
    for m in info.messages.iter().rev() {
        if !m.data.is_hide() {
            if let ChatMessage::User{content, ..} = &m.message { // 必须是User
                match &m.data {
                    DataType::Raw(s) => last_q = Some(s.clone()),
                    DataType::Normal => if let ChatMessageContent::Text(c) = content {
                        last_q = Some(c.clone())
                    },
                    _ => (),
                }
            }
        }
    }
    last_q
}

/// 获取最后一个上传的图片
pub fn get_latest_image(uuid: &str) -> Option<String> {
    let data = DATA.lock().unwrap();
    let info = data.get(uuid).unwrap(); // 调用该函数则该uuid一定在服务端data中，这里直接unwrap
    for m in info.messages.iter().rev() { // 反序遍历
        if m.data.is_hide() {
            continue
        }
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
        if m.data.is_hide() {
            continue
        }
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
