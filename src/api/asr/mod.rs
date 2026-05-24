use std::fs::{write, create_dir_all};
use std::path::Path;
use std::process::exit;
use std::sync::RwLock;

use chrono::Local;
use once_cell::sync::Lazy;
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
use qwen3_asr::{
    AsrInference,
    TranscribeOptions,
    best_device,
};
use serde_json::{json, Value};
use tokenizers::{
    AddedToken,
    Tokenizer,
    decoders::byte_level::ByteLevel as ByteLevelDecoder,
    models::bpe::BPE,
    pre_tokenizers::byte_level::ByteLevel,
};
use tokio::sync::mpsc::{
    channel,
    Sender,
    Receiver,
};
use tokio::sync::oneshot;
use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    error::MyError,
    openai::for_chat::not_use_stream,
};

mod microphone;
mod simple_vad;
mod utils;

use microphone::start_audio_capture;
use simple_vad::voice_activity_detector::split_audio_data_vad;
use utils::{resample_audio, write_wav_sample};

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use rodio::{
    buffer::SamplesBuffer,
    //Decoder,
    OutputStreamBuilder,
    Sink,
};

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use tokio::time::{sleep, Duration};

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use crate::tts::omni_voice::run_omni_tts;

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use std::io::Write;

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use std::fs::OpenOptions;

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use std::path::PathBuf;

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use std::sync::Arc;

#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
use utils::read_wav_sample_resample;

/// 全局变量，可以修改，存储当前开启语音模式的uuid
/// Mutex:
///     不区分读取还是写入，都需要lock
/// RwLock:
///     read: 可以同时多个读，如果正在被其他线程写，则等待其他线程的操作结束后才返回RwLockReadGuard
///     write: 写时不能有其他读或写，如果正在被其他线程读或写，则等待其他线程的操作结束后才返回RwLockWriteGuard
pub static AUDIO: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

static START_WORDS: &[&str; 8] = &["你好，", "你好", "狗屁，", "狗屁", "hello,", "hello", "Hello,", "Hello"];
static STOP_WORDS: &[&str; 8] = &["，结束。", "。结束。", "结束。", "，结束", "。结束", "结束？", "stop", "stop."];

//const CHAT_PROMPT: &str = r###"你扮演严世蕃，我是《大明王朝1566》爱好者，你需要以李世民的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演李世民，我是历史爱好者，你需要以李世民的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演武松，我是水浒爱好者，你需要以武松的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演宋江，我是水浒爱好者，你需要以宋江的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演孙悟空，我是西游记爱好者，你需要以孙悟空的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演猪八戒，我是西游记爱好者，你需要以猪八戒的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演曹操，我是三国爱好者，你需要以曹操的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演关羽，我是三国爱好者，你需要以关羽的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演张飞，我是三国爱好者，你需要以张飞的身份与我进行日常聊天。回复时请严格遵守以下规则：
//const CHAT_PROMPT: &str = r###"你扮演诸葛亮，我是三国爱好者，你需要以诸葛亮的身份与我进行日常聊天。回复时请严格遵守以下规则：
const CHAT_PROMPT: &str = r###"你是日常聊天助手。回复时请严格遵守以下规则：
1. 纯文本格式：不要使用任何 Markdown 语法（如标题、列表、加粗、斜体、代码围栏、引用等），也不要使用 Emoji 或特殊符号（如☺️、❤️、→等）。
2. 每句一行：不要回复包含多个句子的一大段内容，请每行一句话。确保每句表达一个完整的子主题或意思。
3. 自然口语：使用自然、流畅的日常口语风格，就像和朋友聊天一样。"###;
//2. 分段输出：如果回复内容较长（超过 3 句话），请将其拆分为多个短小段落。每个段落包含 2~3 句话，段落之间用空行分隔。确保每段表达一个完整的子主题或意思。

/// Automatic Speech Recognition
pub async fn auto_speech_rec() -> Result<(), MyError> {
    // channel for audio data from mcrophone
    let (tx, mut rx) = channel::<Vec<f32>>(1);

    // channel for start/stop receive input audio
    let (tx_start, rx_start) = channel::<(bool, oneshot::Sender<()>)>(1);
    let tx_start_clone = tx_start.clone();

    // start audio capture
    let (sample_rate, channels) = start_audio_capture(tx, rx_start).await?;
    // qwen3-asr need 16000, omni-voice return 24000
    let max_sample_rate = std::cmp::max(sample_rate, 24000);
    let max_sample_rate_f64 = max_sample_rate as f64;

    // 音频播放线程（独立线程），不要放到`tokio::spawn(async move {})`内
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let (play_tx, play_rx) = std::sync::mpsc::channel::<(Vec<f32>, u32)>();

    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    std::thread::spawn(move || {
        // play audio
        let stream_handle = OutputStreamBuilder::open_default_stream().map_err(|e| MyError::AudioStreamError{error: e}).unwrap();
        let sink = Sink::connect_new(stream_handle.mixer());

        while let Ok((audio_data, sample_rate)) = play_rx.recv() {
            let source = SamplesBuffer::new(1, sample_rate, audio_data);
            sink.append(source);
            //sink.sleep_until_end();
        }
    });

    // channel for send audio data to asr
    let (tx_asr, rx_asr) = channel::<(Vec<f32>, Vec<f32>, bool)>(1);
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    {
        if let Some(ref_path) = PARAS.ref_audio.clone() {
            let ref_audio_data = read_wav_sample_resample(&ref_path, 16000)?;
            if let Err(_) = tx_asr.send((ref_audio_data, Vec::new(), true)).await {
                event!(Level::ERROR, "asr receiver dropped");
            }
        }
    }

    // channel for receive string from asr
    let (tx_asr_string, mut rx_asr_string) = channel::<(String, String, Vec<f32>, bool)>(10);

    // channel for send (LLM anwser string, ref_audio, ref_text, voice_design, language) to tts
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let (tx_tts_string, rx_tts_string) = channel::<(String, Option<String>, Option<String>)>(100);

    // channel for receive audio data and sample rate from tts
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let (tx_tts_audio, mut rx_tts_audio) = channel::<(Vec<f32>, usize, oneshot::Sender<()>)>(100);

    // asr model
    let asr_dir = PARAS.asr_dir.clone();
    tokio::spawn(async move {
        if let Err(e) = run_asr(asr_dir, rx_asr, tx_asr_string, tx_start).await {
            event!(Level::ERROR, "{}", e);
            exit(1);
        }
    });

    // tts声音克隆要用的参考音频
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let ref_audio: Option<PathBuf> = PARAS.ref_audio.clone();
    // tts声音克隆要用的参考音频文本
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let ref_text: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let ref_text_clone = ref_text.clone();

    // tts model
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    let tts_dir = PARAS.tts_dir.clone();
    #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
    tokio::spawn(async move {
        loop {
            let tmp_ref_text = ref_text.read().unwrap().clone();
            if ref_audio.is_some() && tmp_ref_text.is_some() {
                break
            } else {
                sleep(Duration::from_millis(500)).await;
            }
        }
        let tmp_ref_text = ref_text.read().unwrap().clone();
        if let Err(e) = run_omni_tts(&tts_dir, ref_audio, tmp_ref_text, rx_tts_string, tx_tts_audio).await {
            event!(Level::ERROR, "tts error: {}", e);
            exit(1);
        }
    });

    // receive string from asr
    let role = format!("扮演{}", PARAS.role);
    let outpath = PARAS.outpath.clone();
    tokio::spawn(async move {
        let mut start = false;
        let mut run_llm_tts = false;
        let mut with_history = true;
        let mut audio_id = get_audio_id(); // 音频问答编号
        let mut whole_string = String::new(); // 存储提问的字符串数据
        let mut whole_uer_audio: Vec<f32> = Vec::new(); // 存储提问的音频数据
        #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
        let mut whole_tts_audio: Vec<f32> = Vec::new(); // 存储回答的音频数据
        let mut history_msg: Vec<ChatMessage> = vec![ // 存储聊天记录
            ChatMessage::User{
                content: ChatMessageContent::Text(CHAT_PROMPT.replace("是日常聊天助手", &role)),
                name: None,
            },
        ];
        let mut today = "".to_string();
        let mut today_current: String;
        let mut audio_save_path = format!("{}/audio_{}", outpath, today);

        /*
        // play audio
        #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
        let stream_handle = OutputStreamBuilder::open_default_stream().map_err(|e| MyError::AudioStreamError{error: e}).unwrap();
        #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
        let sink = Sink::connect_new(stream_handle.mixer());
        */

        while let Some((asr_string, _language, audio_chunk_24, is_ref)) = rx_asr_string.recv().await {
            event!(Level::INFO, "part: {}", asr_string);
            if is_ref {
                #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
                {
                    let mut tmp_text = ref_text_clone.write().unwrap();
                    *tmp_text = Some(asr_string);
                    drop(tmp_text);
                }
            } else {
                /*
                #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
                if let Err(e) = run_tts_voice_clone("./Qwen3-TTS-12Hz-0.6B-CustomVoice".to_string(), &asr_string) {
                    // tts error: Error - Qwen3-TTS: DriverError(CUDA_ERROR_UNSUPPORTED_PTX_VERSION, "the provided PTX was compiled with an unsupported toolchain.")
                    event!(Level::ERROR, "tts error: {}", e);
                }
                */
                if start {
                    for (i, w) in STOP_WORDS.iter().enumerate() {
                        if asr_string.ends_with(w) {
                            start = false;
                            run_llm_tts = true;
                            whole_string += asr_string.strip_suffix(w).unwrap();
                            break
                        }
                        if i+1 == STOP_WORDS.len() {
                            whole_string += &asr_string;
                        }
                    }
                    whole_uer_audio.extend(audio_chunk_24);
                } else {
                    for (i, w1) in START_WORDS.iter().enumerate() {
                        if asr_string.starts_with(w1) {
                            start = true;
                            whole_uer_audio = audio_chunk_24;
                            for (j, w2) in STOP_WORDS.iter().enumerate() {
                                if asr_string.ends_with(w2) {
                                    start = false;
                                    run_llm_tts = true;
                                    whole_string = asr_string.strip_prefix(w1).unwrap().strip_suffix(w2).unwrap().to_string();
                                    break
                                }
                                if j+1 == STOP_WORDS.len() {
                                    whole_string = asr_string.strip_prefix(w1).unwrap().to_string();
                                }
                            }
                            break
                        }
                        if i+1 == START_WORDS.len() {
                            event!(Level::INFO, "discard: {}", asr_string);
                        }
                    }
                    /*
                    if START_WORDS.iter().any(|w| asr_string.starts_with(w)) {
                        start = true;
                        whole_string = asr_string;
                        if STOP_WORDS.iter().any(|w| whole_string.ends_with(w)) {
                            start = false;
                            run_llm_tts = true;
                        }
                    } else {
                        event!(Level::INFO, "discard: {}", asr_string);
                        //whole_string += &asr_string;
                    }
                    */
                }
                if run_llm_tts {
                    if whole_string.contains("开启新对话") || whole_string.contains("start new chat") {
                        history_msg = vec![
                            ChatMessage::User{
                                content: ChatMessageContent::Text(CHAT_PROMPT.replace("是日常聊天助手", &role)),
                                name: None,
                            },
                        ];
                        run_llm_tts = false;
                    } else if whole_string.contains("不记录历史") || whole_string.contains("without history") {
                        with_history = false;
                    } else if whole_string.contains("记录历史") || whole_string.contains("with history") {
                        with_history = true;
                    }
                }
                if run_llm_tts {
                    // 检查日期是否变更
                    today_current = Local::now().format("%Y-%m-%d").to_string();
                    if today_current != today {
                        today = today_current;
                        audio_save_path = format!("{}/audio_{}", outpath, today);
                        let tmp_audio_path = Path::new(&audio_save_path);
                        if !(tmp_audio_path.exists() && tmp_audio_path.is_dir()) {
                            if let Err(e) = create_dir_all(&tmp_audio_path) {
                                event!(Level::ERROR, "create_dir_all error: {:?}", e);
                            }
                        }
                    }
                    // save user question
                    if let Err(e) = write(format!("{}/me_{}.txt", audio_save_path, audio_id), &whole_string) {
                        event!(Level::ERROR, "save user question error: {:?}", e);
                    }
                    // save user audio
                    if let Err(e) = write_wav_sample(&whole_uer_audio, max_sample_rate, audio_id, true, &audio_save_path) {
                        event!(Level::ERROR, "save user audio error: {:?}", e);
                    }
                    event!(Level::INFO, "whole asr string: {}", whole_string);
                    run_llm_tts = false;
                    if with_history {
                        history_msg.push(
                            ChatMessage::User{
                                content: ChatMessageContent::Text(whole_string.clone()),
                                name: None,
                            },
                        );
                    } else {
                        history_msg = vec![
                            ChatMessage::User{
                                content: ChatMessageContent::Text(CHAT_PROMPT.replace("是日常聊天助手", &role)),
                                name: None,
                            },
                            ChatMessage::User{
                                content: ChatMessageContent::Text(whole_string.clone()),
                                name: None,
                            },
                        ];
                    }
                    if let Ok(answer) = run_llm(history_msg.clone()).await {
                        event!(Level::INFO, "llm: {}", answer);
                        if with_history {
                            history_msg.push(
                                ChatMessage::Assistant {
                                    content: Some(ChatMessageContent::Text(answer.clone())),
                                    reasoning_content: None,
                                    refusal: None,
                                    name: None,
                                    audio: None,
                                    tool_calls: None,
                                },
                            );
                        }

                        #[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
                        {
                            let tmp_uuid = AUDIO.read().unwrap().clone(); // 使用RwLock，保证一写多读，只要不在写，就可以同时多个读取
                            if tmp_uuid.is_none() {
                                //for t in answer.replace("\n\n", "\n").replace("\n", "。").split("。") {
                                let sub_answer: Vec<String> = answer.replace("\n\n", "\n").split("\n").map(|a| a.to_string()).collect();
                                let mut sub_len = sub_answer.len();
                                for (i, t) in sub_answer.into_iter().enumerate() {
                                    if i == 0 {
                                        // 第一段拆分出第一个逗号或句号前的内容，先处理这部分，减少语音回复的等待时间
                                        let seps = ["，", "。", "！", "？"];
                                        let mut idx_sep = (usize::MAX, "");
                                        for s in seps {
                                            if let Some(idx) = t.find(s) {
                                                if idx < idx_sep.0 {
                                                    idx_sep = (idx, s);
                                                }
                                            }
                                        }
                                        if idx_sep.1.is_empty() {
                                            if let Err(_) = tx_tts_string.send((t, None, Some(_language.clone()))).await {
                                                event!(Level::ERROR, "tts receiver dropped");
                                            }
                                        } else {
                                            let parts = t.split(idx_sep.1).map(|s| s.to_string()).collect::<Vec<_>>();
                                            sub_len += parts.len()-1;
                                            for p in parts {
                                                if let Err(_) = tx_tts_string.send((p, None, Some(_language.clone()))).await {
                                                    event!(Level::ERROR, "tts receiver dropped");
                                                }
                                            }
                                        }
                                    } else {
                                        if let Err(_) = tx_tts_string.send((t, None, Some(_language.clone()))).await {
                                            event!(Level::ERROR, "tts receiver dropped");
                                        }
                                    }
                                    /*
                                    //if let Err(e) = run_tts_voice_clone("./moss-tts-nano-candle".to_string(), &t) {
                                    if let Err(e) = run_tts_voice_clone("./OmniVoice".to_string(), &t, Some("Chinese".to_string())) {
                                        // tts error: Error - Qwen3-TTS: DriverError(CUDA_ERROR_UNSUPPORTED_PTX_VERSION, "the provided PTX was compiled with an unsupported toolchain.")
                                        event!(Level::ERROR, "tts error: {}", e);
                                    }
                                    */
                                }
                                for _i in 0..sub_len {
                                    if let Some((audio_data, sample_rate, ack_tts_tx)) = rx_tts_audio.recv().await {
                                        ack_tts_tx.send(()).unwrap(); // 发送确认
                                        whole_tts_audio.extend(audio_data.clone());
                                        /*
                                        let source = SamplesBuffer::new(1, sample_rate as u32, audio_data);
                                        sink.append(source);
                                        */
                                        if let Err(_) = play_tx.send((audio_data, sample_rate as u32)) {
                                            event!(Level::ERROR, "play tts receiver dropped");
                                        }
                                    }
                                }
                                // save tts anwser
                                if let Err(e) = write(format!("{}/model_{}.txt", audio_save_path, audio_id), &answer) {
                                    event!(Level::ERROR, "save tts anwser error: {:?}", e);
                                }
                                // save tts audio
                                if let Err(e) = write_wav_sample(&whole_tts_audio, max_sample_rate, audio_id, false, &audio_save_path) {
                                    event!(Level::ERROR, "save tts audio error: {:?}", e);
                                }
                                // save user question and llm answer to ``
                                if let Err(e) = append_line(&audio_save_path, format!("{}\t{}\t{}", audio_id, whole_string, answer.replace("\n", ""))) {
                                    event!(Level::ERROR, "append line to {}/all.txt error: {:?}", audio_save_path, e);
                                }
                                whole_tts_audio.clear();
                                // The sound plays in a separate thread. This call will block the current thread until the sink has finished playing all its queued sounds.
                                //sink.sleep_until_end();
                                /*
                                if let Err(e) = run_tts_voice_clone("./OmniVoice".to_string(), &answer, Some("Chinese".to_string())) {
                                    // tts error: Error - Qwen3-TTS: DriverError(CUDA_ERROR_UNSUPPORTED_PTX_VERSION, "the provided PTX was compiled with an unsupported toolchain.")
                                    event!(Level::ERROR, "tts error: {}", e);
                                }
                                */
                            }
                        }
                    }
                    whole_string = "".to_string();
                    whole_uer_audio.clear();
                    audio_id += 1;
                }
            }
            // start receive microphone audio
            let (ack_tx, ack_rx) = oneshot::channel(); // 创建应答通道
            if let Err(e) = tx_start_clone.try_send((true, ack_tx)) {
                event!(Level::INFO, "asr finished, start receive microphone audio error: {:?}", e);
            }
            ack_rx.await.unwrap(); // 等待接收方确认
            event!(Level::INFO, "asr waiting new audio chunk...");
        }
    });

    let mut audio_data_f32_buffer: Vec<f32> = Vec::new();
    let mut audio_data_f32_norm: Vec<f32>;
    let mut audio_data_f32_trim: Vec<f32> = Vec::new();

    // silence frame
    let silence_length = (2 * sample_rate) as usize; // 2 second frames length
    let half_silence_length = silence_length / 2;

    // receive user audio, convert to base64, send requenst to api
    while let Some(audio_chunk) = rx.recv().await {
        // waiting user audio data
        //println!("audio_chunk {}, {}, {}", audio_chunk.len(), silence_length, audio_data_f32_buffer.len());
        audio_data_f32_buffer.extend(
            if channels == 2 { // convert to mono
                audio_chunk.chunks_exact(2).map(|pair| (pair[0] + pair[1]) / 2.0).collect()
            } else {
                audio_chunk
            }
        );

        // normalize audio data
        let for_norm = f32::max(audio_data_f32_buffer.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(), 0.1);
        audio_data_f32_norm = audio_data_f32_buffer.iter().map(|x| x / for_norm).collect();

        // activity start and end index
        let segments: Vec<(usize, usize)> = split_audio_data_vad(audio_data_f32_norm, sample_rate, false)?; // Vec<(start, end)>

        // find silence
        match segments.last() {
            Some((_, end)) => {
                //println!("buffer length: {}, segments: {:?}", audio_data_f32_buffer.len(), segments);
                if audio_data_f32_buffer.len() - end > silence_length {
                    //println!("sum: {}", segments.iter().fold(0, |acc, x| acc + x.1 - x.0));
                    if segments.iter().fold(0, |acc, x| acc + x.1 - x.0) > 5000 {
                        //let _ = tx_start.try_send(false);
                        audio_data_f32_trim = audio_data_f32_buffer[segments[0].0..(*end+half_silence_length)].to_vec();
                    } else {
                        audio_data_f32_buffer.drain(0..*end);
                    }
                }
            },
            None => {
                if audio_data_f32_buffer.len() > silence_length {
                    audio_data_f32_buffer.drain(0..half_silence_length);
                }
                //audio_data_f32_buffer.clear();
                continue
            },
        }

        if !audio_data_f32_trim.is_empty() {
            // start receive new input audio
            //println!("audio_data_f32_buffer: {}", audio_data_f32_buffer.len());
            //println!("audio_data_f32_trim: {}", audio_data_f32_trim.len());

            // resample sample_rate to 16000 and max sample rate
            (audio_data_f32_trim, audio_data_f32_buffer) = {
                // qwen3-asr need 16000, omni-voice return 24000
                match (sample_rate == 16000, sample_rate == max_sample_rate) {
                    (true, false) => (
                        audio_data_f32_trim.clone(),
                        resample_audio(audio_data_f32_trim, sample_rate as f64, max_sample_rate_f64)?,
                    ),
                    (true, true) => unreachable!(),
                    (false, false) => (
                        resample_audio(audio_data_f32_trim.clone(), sample_rate as f64, 16000.0)?,
                        resample_audio(audio_data_f32_trim, sample_rate as f64, max_sample_rate_f64)?,
                    ),
                    (false, true) => (
                        resample_audio(audio_data_f32_trim.clone(), sample_rate as f64, 16000.0)?,
                        audio_data_f32_trim,
                    ),
                }
            };

            // speech to string
            if let Err(_) = tx_asr.send((audio_data_f32_trim.drain(..).collect(), audio_data_f32_buffer.drain(..).collect(), false)).await {
                event!(Level::ERROR, "asr receiver dropped");
            }

            // clear vec, waiting next microphone audio
            audio_data_f32_buffer.clear();
            audio_data_f32_trim.clear();
        }
    }
    Ok(())
}

/// Qwen3-ASR
async fn run_asr(model_dir: String, mut rx_asr: Receiver<(Vec<f32>, Vec<f32>, bool)>, tx_asr_string: Sender<(String, String, Vec<f32>, bool)>, tx_start: Sender<(bool, oneshot::Sender<()>)>) -> Result<(), MyError> {
    // 下面 load 方法内部需要 tokenizer.json 而原模型没有提供，因此这里根据 vocab.json、merges.txt、tokenizer_config.json 创建
    // https://docs.rs/qwen3-asr/0.2.2/src/qwen3_asr/inference.rs.html#98-119
    let tokenizer_json = model_dir.clone()+"tokenizer.json";
    let tokenizer_json = Path::new(&tokenizer_json);
    if !tokenizer_json.exists() {
        create_tokenizer_json(model_dir.clone())?;
    }

    let device = best_device(); // automatically selects CUDA → Metal → CPU
    let engine = AsrInference::load(Path::new(&model_dir), device).map_err(|e| MyError::Qwen3AsrError{error: e})?;

    event!(Level::INFO, "asr waiting audio chunk...");
    while let Some((audio_chunk_16, audio_chunk_24, is_ref)) = rx_asr.recv().await {
        event!(Level::INFO, "asr received audio chunk, length: {}", audio_chunk_16.len());
        // stop receive microphone audio
        let (ack_tx, ack_rx) = oneshot::channel(); // 创建应答通道
        let _ = tx_start.try_send((false, ack_tx));
        ack_rx.await.unwrap(); // 等待接收方确认

        let result = engine.transcribe_samples(&audio_chunk_16, TranscribeOptions::default()).map_err(|e| MyError::Qwen3AsrError{error: e})?;
        //println!("Language: {}", result.language);
        //println!("Text: {}", result.text);
        if result.text.is_empty() {
            // start receive microphone audio
            let (ack_tx, ack_rx) = oneshot::channel(); // 创建应答通道
            let _ = tx_start.try_send((true, ack_tx));
            ack_rx.await.unwrap(); // 等待接收方确认
            event!(Level::INFO, "asr waiting audio chunk...");
        } else {
            if let Err(_) = tx_asr_string.send((result.text, result.language, audio_chunk_24, is_ref)).await {
                event!(Level::ERROR, "asr string receiver dropped");
            }
        }
    }
    Ok(())
}

/// 使用 vocab.json、merges.txt、tokenizer_config.json 创建 tokenizer.json
/// https://github.com/jhqxxx/aha/blob/main/src/tokenizer/mod.rs
fn create_tokenizer_json(path: String) -> Result<(), MyError> {
    let vocab_file = path.clone() + "vocab.json";
    let merges_file = path.clone() + "merges.txt";
    let config_file = path.clone() + "tokenizer_config.json";

    // 创建 BPE 模型
    let bpe = BPE::from_file(&vocab_file, &merges_file).build().map_err(|e| MyError::TokenizerError{error: e})?;

    // 创建分词器
    let mut tokenizer = Tokenizer::new(bpe);
    // 添加字节级预分词器，这会处理换行符等特殊字符
    let byte_level_pre_tokenizer = ByteLevel::new(false, true, false);
    tokenizer.with_pre_tokenizer(Some(byte_level_pre_tokenizer));
    tokenizer.with_decoder(Some(ByteLevelDecoder::default()));

    let config_content = std::fs::read_to_string(&config_file)?;
    let config: Value = serde_json::from_str(&config_content).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
    if let Some(added_tokens_decoder) = config.get("added_tokens_decoder") {
        let mut special_tokens = Vec::new();
        if let Value::Object(tokens_map) = added_tokens_decoder {
            for (_, token_info) in tokens_map {
                if let Value::Object(token_obj) = token_info
                    && let Some(content_val) = token_obj.get("content")
                    && let Some(content) = content_val.as_str()
                {
                    let special = token_obj
                        .get("special")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let added_token = AddedToken::from(content.to_string(), special);
                    special_tokens.push(added_token);
                }
            }
        }

        // 添加所有特殊标记
        if !special_tokens.is_empty() {
            tokenizer.add_special_tokens(&special_tokens);
        }
    }

    // 保存为 tokenizer.json
    tokenizer.save(path + "tokenizer.json", false).map_err(|e| MyError::TokenizerError{error: e})
}

/// run llm
async fn run_llm(messages: Vec<ChatMessage>) -> Result<String, MyError> {
    // 获取指定模型的api-key
    let (api_key, endpoint, model, thinking) = PARAS.api.get_model_by_usize(17)?;
    // 使用api key初始化
    let mut client = Client::new(api_key.clone());
    client.set_base_url(&endpoint);
    let mut para_builder = ChatCompletionParametersBuilder::default();
    para_builder.model(model.clone()); // 指定模型
    para_builder.response_format(ChatCompletionResponseFormat::Text);
    // 对思维链模型设置effort
    let lowercase_model = model.to_lowercase();
    if thinking {
        para_builder.reasoning_effort(ReasoningEffort::Low); // 设置使用思维链，Low（思考的少，简单问答）, Medium（思考适中，多步骤推理）, High（思考更多，复杂逻辑推导）
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
        }
    } else {
        // 关闭思考，不同模型思考的设置不同
        if lowercase_model.starts_with("deepseek") {
            // deepseek: https://api-docs.deepseek.com/
            para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
        } else if lowercase_model.starts_with("qwen") {
            // Qwen: https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions#05cfceb898csa
            para_builder.extra_body(json!({"enable_thinking": false}));
        } else if lowercase_model.starts_with("kimi") {
            // kimi: https://platform.kimi.com/docs/api/models-overview
            para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
        } else if lowercase_model.starts_with("glm") {
            // glm: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction
            para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
        }
    }
    para_builder.messages(messages);
    match para_builder.build() {
        Ok(parameters) => {
            match not_use_stream("run_llm_for_tts".to_string(), client, parameters, &model, false).await {
                Ok((result, _resoning)) => {
                    if result.is_empty() {
                        event!(Level::ERROR, "run llm for tts no response");
                        Err(MyError::OtherError{info: "run llm for tts no response".to_string()})
                    } else {
                        Ok(result)
                    }
                },
                Err(e) => {
                    event!(Level::ERROR, "run llm for tts error: {}", e);
                    Err(MyError::OtherError{info: format!("run llm for tts error: {}", e)})
                },
            }
        },
        Err(e) => {
            event!(Level::ERROR, "run llm for tts builder error: {:?}", e);
            Err(MyError::OtherError{info: format!("run llm for tts builder error: {:?}", e)})
        },
    }
}

/// 获取输出路径下音频编号
fn get_audio_id() -> usize {
    let mut audio_id = 0;
    let today = Local::now().format("%Y-%m-%d").to_string();
    let audio_path_str = format!("{}/audio_{}", PARAS.outpath, today);
    let audio_path = Path::new(&audio_path_str);
    if let Ok(files) = audio_path.read_dir() {
        for i in files {
            if let Ok(entry) = i {
                let file_path = entry.path();
                let tmp = file_path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .split("_")
                    .last()
                    .unwrap()
                    .split(".")
                    .next()
                    .unwrap();
                if let Ok(n) = tmp.parse::<usize>() {
                    if n+1 > audio_id {
                        audio_id = n+1;
                    }
                }
            }
        }
    }
    audio_id
}

/// 向指定文件追加一行文本（如果文件不存在则自动创建）
#[cfg(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal"))]
fn append_line(file_path: &str, line: String) -> Result<(), MyError> {
    let mut file = OpenOptions::new()
        .append(true) // 以追加模式打开
        .create(true) // 如果文件不存在则创建
        .open(format!("{}/all.txt", file_path))?;

    writeln!(file, "{}", line)?; // 自动添加换行符
    Ok(())
}
