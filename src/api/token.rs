use std::process::exit;

use tiktoken_rs::{
    o200k_base, // GPT-4o models
    cl100k_base, // ChatGPT models text-embedding-ada-002
    p50k_base, // Code models, text-davinci-002, text-davinci-003
    p50k_edit, // edit models like text-davinci-edit-001, code-davinci-edit-001
    r50k_base, // GPT-3 models like davinci, also known as gpt2
    CoreBPE,
};

/// error: 定义的错误类型，用于错误传递
use crate::error::MyError;

/// 根据指定编码类型，返回CoreBPE对象
pub fn get_tokenizer(encoding: &str) -> CoreBPE {
    match encoding {
        "o200k" => match o200k_base() {
            Ok(e) => e,
            Err(e) => {
                println!("{}", MyError::TokenizerError{tokenizer: "o200k_base".to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                exit(1);
            },
        },
        "cl100k" => match cl100k_base() {
            Ok(e) => e,
            Err(e) => {
                println!("{}", MyError::TokenizerError{tokenizer: "cl100k_base".to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                exit(1);
            },
        },
        "p50k" => match p50k_base() {
            Ok(e) => e,
            Err(e) => {
                println!("{}", MyError::TokenizerError{tokenizer: "p50k_base".to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                exit(1);
            },
        },
        "p50k_edit" => match p50k_edit() {
            Ok(e) => e,
            Err(e) => {
                println!("{}", MyError::TokenizerError{tokenizer: "p50k_edit".to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                exit(1);
            },
        },
        "r50k" | "gpt2" => match r50k_base() {
            Ok(e) => e,
            Err(e) => {
                println!("{}", MyError::TokenizerError{tokenizer: "r50k_base".to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                exit(1);
            },
        },
        _ => match o200k_base() {
            Ok(e) => e,
            Err(e) => {
                println!("{}", MyError::TokenizerError{tokenizer: "o200k_base".to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                exit(1);
            },
        },
    }
}
