use openai_dive::v1::{
    api::Client,
    models::TranscriptionModel,
    resources::{
        audio::{
            AudioOutputFormat, // Json, Text, Srt(字幕), VerboseJson, Vtt(字幕)
            AudioTranslationParametersBuilder
        },
        shared::FileUpload,
    },
};

/// error: 定义的错误类型，用于错误传递
use crate::error::MyError;

/// 调用openai的api将音频翻译为指定语言的文本
pub async fn create_translation(uuid: &str, audio: Option<String>, outpath: &str, endpoint: &str, api_key: String) -> Result<String, MyError> {
    if let Some(a) = audio {
        // 使用api key初始化
        let mut client = Client::new(api_key);
        client.set_base_url(endpoint); // 从0.7.0开始舍弃了new_with_base
        // 参数
        let parameters = AudioTranslationParametersBuilder::default()
            .file(FileUpload::File(format!("{}/{}/{}", outpath, uuid, a))) // 指定的音频文件，例如："./audio/multilingual.mp3"
            .model(TranscriptionModel::Whisper1.to_string())
            .response_format(AudioOutputFormat::Text) // 输出格式，支持：Json, Text, Srt(字幕), VerboseJson, Vtt(字幕)
            .build().map_err(|e| MyError::AudioTranslationError{error: e})?;
        // 翻译
        let result = client.audio().create_translation(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;
        Ok(result)
    } else {
        Err(MyError::ParaError{para: "you need upload voice file first".to_string()})
    }
}
