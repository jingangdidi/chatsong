use std::fs::File;
use std::io::Write;

use futures::{
    future,
    stream::StreamExt,
};
use openai_dive::v1::{
    api::Client,
    helpers::generate_file_name, // 生成由大写英文字母构成的指定长度随机文件名
    models::TTSModel, // Tts1, Tts1HD
    resources::audio::{
        AudioSpeechParametersBuilder,
        AudioSpeechResponseFormat, // Mp3, Opus, Aac, Flac, Wav, Pcm
        AudioVoice, // Alloy, Echo, Fable, Onyx, Nova, Shimmer
    },
};
use tracing::{event, Level};

/// error: 定义的错误类型，用于错误传递
use crate::{
    error::MyError,
};

/// 调用openai的api生成speech
/// q: 要生成声音的文本内容
/// voice: 选择的声音
/// speech_file: 保存生成的声音文件，如果已存在则覆盖，不保留历史记录
/// 返回不含路径的音频文件名，如果有报错，则返回报错字符串
pub async fn create_speech(uuid: &str, q: String, voice: usize, outpath: &str, endpoint: &str, api_key: String) -> Result<String, MyError> {
    // 指定的声音参数
    let tmp_voice = match voice {
        1 => AudioVoice::Alloy,
        2 => AudioVoice::Echo,
        3 => AudioVoice::Fable,
        4 => AudioVoice::Onyx,
        5 => AudioVoice::Nova,
        6 => AudioVoice::Shimmer,
        _ => AudioVoice::Alloy, // 这里默认用Alloy
    };
    // 使用api key初始化
    let mut client = Client::new(api_key);
    client.set_base_url(endpoint); // 从0.7.0开始舍弃了new_with_base
    // 参数
    let parameters = AudioSpeechParametersBuilder::default()
        .model(TTSModel::Tts1.to_string()) // 模型，支持：Tts1, Tts1HD
        .input(&q) // 文本内容，最长4096个字符，例如：The quick brown fox jumped over the lazy dog.
        .voice(tmp_voice) // 声音，支持：Alloy, Echo, Fable, Onyx, Nova, Shimmer
        .response_format(AudioSpeechResponseFormat::Mp3) // 格式，支持：Mp3, Opus, Aac, Flac, Wav, Pcm
        .speed(1.0) // 速度，支持：0.25-4.0，默认1.0
        .build().map_err(|e| MyError::AudioSpeechError{error: e})?;
    // 创建声音文件
    let uuid_path = format!("{}/{}", outpath, uuid);
    let speech_file = generate_file_name(&uuid_path, 16, "mp3"); // 生成由大写英文字母构成的指定长度随机文件名
    let mut file = File::create(&speech_file).map_err(|e| MyError::CreateFileError{file: speech_file.clone(), error: e})?;
    // 提交请求
    let stream = client.audio().create_speech_stream(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;
    // 保存声音
    stream.for_each(|chunk| {
        match chunk {
            Ok(c) => {
                //println!("Received chunk of {} bytes", c.bytes.len());
                file.write_all(&c.bytes).unwrap();
            },
            Err(e) => event!(Level::ERROR, "{} create speech stream error: {}", uuid, e),
        }
        future::ready(())
    }).await;
    Ok(speech_file.replace(&format!("{}/{}/", outpath, uuid), ""))
}
