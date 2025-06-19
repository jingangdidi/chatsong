use openai_dive::v1::{
    api::Client,
    models::TranscriptionModel,
    resources::{
        audio::{
            AudioOutputFormat, // Json, Text, Srt(字幕), VerboseJson, Vtt(字幕)
            AudioTranscriptionParametersBuilder,
        },
        shared::FileUpload,
    },
};

/// error: 定义的错误类型，用于错误传递
use crate::error::MyError;

/// 调用openai的api从音频提取文本
pub async fn create_transcription(uuid: &str, audio: Option<String>, outpath: &str, endpoint: &str, api_key: String) -> Result<String, MyError> {
    if let Some(a) = audio {
        // 使用api key初始化
        let mut client = Client::new(api_key);
        client.set_base_url(endpoint); // 从0.7.0开始舍弃了new_with_base
        // 参数
        let parameters = AudioTranscriptionParametersBuilder::default()
            .file(FileUpload::File(format!("{}/{}/{}", outpath, uuid, a))) // 指定的音频文件，例如："./audio/micro-machines.mp3"
            .model(TranscriptionModel::Whisper1.to_string())
            //.language("en".to_string()) // 音频的语言（ISO-639-1格式），指定该参数会提高准确度，例如：zh(汉语)、de(德语)、fr(法语)、en(英语)、it(意大利语)、ja(日语)、ko(朝鲜语)
            .response_format(AudioOutputFormat::Text) // 输出格式，支持：Json, Text, Srt(字幕), VerboseJson, Vtt(字幕)
            .build().map_err(|e| MyError::AudioTranscriptionError{error: e})?;
        // 提取文本
        let result = client.audio().create_transcription(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;
        Ok(result)
    } else {
        Err(MyError::ParaError{para: "you need upload voice file first".to_string()})
    }
}
