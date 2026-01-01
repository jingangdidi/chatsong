use std::io;
use std::num::ParseIntError;
use std::string::FromUtf8Error;

use axum::{
    http::{StatusCode, Error as http_error},
    response::{Response, IntoResponse},
    extract::multipart::MultipartError,
};
use base64::DecodeError;
use glob::PatternError;
use grep::regex::Error as grep_error;
use openai_dive::v1::{
    resources::{
        chat::ChatCompletionParametersBuilderError,
        image::{
            CreateImageParametersBuilderError,
            EditImageParametersBuilderError,
        },
        audio::{
            AudioSpeechParametersBuilderError,
            AudioTranslationParametersBuilderError,
            AudioTranscriptionParametersBuilderError,
        },
    },
    error::APIError,
};
use pdf_extract::OutputError;
use reqwest::Error as reqwest_error;
use serde_json::Error as json_error;
use thiserror::Error;
use zip::result::ZipError;

/// srx添加，自定义的错误类型，方便传递错误
/// 参考：https://github.com/dtolnay/thiserror
/// 参考：https://crates.io/crates/thiserror
/// 参考：https://juejin.cn/post/7272005801081126968
/// 参考：https://www.shakacode.com/blog/thiserror-anyhow-or-how-i-handle-errors-in-rust-apps/
/// 参考：https://rustcc.cn/article?id=1e20f814-c7d5-4aca-bb67-45dcfb65d9f9
#[derive(Debug, Error)]
pub enum MyError {
    // 读取文件错误
    #[error("Error - fs::read {file}: {error}")]
    ReadFileError{file: String, error: io::Error},

    // 打开文件错误
    #[error("Error - fs::File::open {file}: {error}")]
    OpenFileError{file: String, error: io::Error},

    // 创建文件错误
    #[error("Error - fs::create {file}: {error}")]
    CreateFileError{file: String, error: io::Error},

    // 创建路径错误
    #[error("Error - fs::create_dir_all {dir_name}: {error}")]
    CreateDirAllError{dir_name: String, error: io::Error},

    // 创建文件(一次写入)错误
    #[error("Error - fs::write {file}: {error}")]
    WriteFileError{file: String, error: io::Error},

    // 按行读取文件错误
    #[error("Error - read lines {file}: {error}")]
    LinesError{file: String, error: io::Error},

    // 获取指定路径下所有项错误
    #[error("Error - read_dir {dir}: {error}")]
    ReadDirError{dir: String, error: io::Error},

    // 删除文件夹错误
    #[error("Error - fs::remove_dir {dir}: {error}")]
    RemoveDirError{dir: String, error: io::Error},

    // 删除文件错误
    #[error("Error - fs::remove_file {file}: {error}")]
    RemoveFileError{file: String, error: io::Error},

    // 字符串转指定类型错误
    #[error("Error - parse {from} -> {to}: {error}")]
    ParseStringError{from: String, to: String, error: ParseIntError},

    // 路径不存在
    #[error("Error - {dir} does not exist")]
    DirNotExistError{dir: String},

    // 文件不存在
    #[error("Error - {file} does not exist")]
    FileNotExistError{file: String},

    // 读取文件转为UTF-8错误
    #[error("Error - {file} to UTF-8: {error}")]
    FileContentToUtf8Error{file: String, error: FromUtf8Error},

    // 初始化chat错误
    #[error("Error - ChatCompletionParametersBuilder: {error}")]
    ChatCompletionError{error: ChatCompletionParametersBuilderError},

    // 初始化绘图错误
    #[error("Error - CreateImageParametersBuilder: {error}")]
    CreateImageError{error: CreateImageParametersBuilderError},

    // 改图错误
    #[error("Error - EditImageParametersBuilderError: {error}")]
    EditImageError{error: EditImageParametersBuilderError},

    // 初始化声音错误
    #[error("Error - AudioSpeechParametersBuilder: {error}")]
    AudioSpeechError{error: AudioSpeechParametersBuilderError},

    // 初始化声音翻译错误
    #[error("Error - AudioTranslationParametersBuilder: {error}")]
    AudioTranslationError{error: AudioTranslationParametersBuilderError},

    // 初始化声音提取文本错误
    #[error("Error - AudioTranscriptionParametersBuilder: {error}")]
    AudioTranscriptionError{error: AudioTranscriptionParametersBuilderError},

    // api错误
    #[error("Error - {uuid} OpenAI APIError: {error}")]
    ApiError{uuid: String, error: APIError},

    // 数据转为json字符串错误
    #[error("Error - to json string: {error}")]
    ToJsonStirngError{uuid: String, error: json_error},

    // json转字符串错误
    #[error("Error - serde_json::to_string: {error}")]
    JsonToStringError{error: io::Error},

    // parse json string error
    #[error("Error - serde_json::from_str: {error}")]
    SerdeJsonFromStrError{error: json_error},

    // struct json Value error
    #[error("Error - struct to json Value: {error}")]
    StructToJsonValueError{error: json_error},

    // parse json Value to struct error
    #[error("Error - serde_json::from_value: {error}")]
    SerdeJsonToStructError{error: json_error},

    // Response错误
    #[error("Error - {uuid} build Response: {error}")]
    ResponseError{uuid: String, error: http_error},

    // Tokenizer错误
    #[error("Error - Initialize {tokenizer} tokenizer: {error}")]
    TokenizerError{tokenizer: String, error: anyhow::Error},

    // 向指定url发送请求错误
    #[error("Error - sending request {url}: {error}")]
    SendRequestError{url: String, error: reqwest_error},

    // 获取客户端上传文件错误
    #[error("Error - parsing multipart/form-data requests: {error}")]
    ParseMultipartError{error: MultipartError},

    // 响应内容转text错误
    #[error("Error - get response text {url}: {error}")]
    GetResponseTextError{url: String, error: reqwest_error},

    // 响应内容text转json错误
    #[error("Error - response text to json: {error}")]
    ResponseTextToJsonError{error: json_error},

    // 网络搜索错误
    #[error("Error - {info}")]
    WebSearchError{info: String},

    // 读取zip压缩文件错误
    #[error("Error - ZipArchive::new({file}): {error}")]
    ZipArchiveError{file: String, error: ZipError},

    // 创建glob的pattern错误
    #[error("Error - create glob pattern {pattern}: {error}")]
    CreatePatternError{pattern: String, error: PatternError},

    // 从pdf文件提取内容错误
    #[error("Error - extract content from {file}: {error}")]
    ExtractPdfError{file: String, error: OutputError},

    // base64解码为图片错误
    #[error("Error - decode base64 to image {file}: {error}")]
    Base64DecodeError{file: String, error: DecodeError},

    // tool id not exist
    #[error("Error - Tool {id} ({info}) not exist")]
    ToolNotExistError{id: String, info: String},

    // plan mode error
    #[error("Error - {info}")]
    PlanModeError{info: String},

    // run command error
    #[error("Error - {info}")]
    CommandError{info: String},

    // get stdout error
    #[error("Error - {info}")]
    StdOutError{info: String},

    // get stderr error
    #[error("Error - {info}")]
    StdErrError{info: String},

    // grep error
    #[error("Error - grep error: {error}")]
    GrepError{error: grep_error},

    // MCP error
    #[error("Error - {info}")]
    McpError{info: String},

    // other error
    #[error("Error - {info}")]
    OtherError{info: String},

    // 参数使用错误
    #[error("Error - {para}")]
    ParaError{para: String},

    // 常规io::Error，这里可以改为向上面那样将错误传过来，但不知道还能否使用`#[from]`
    #[error("I/O error occurred")]
    IoError(#[from] io::Error),
}

/// 为MyError实现IntoResponse，这样在axum中也可以使用
impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", self)).into_response()
    }
}
