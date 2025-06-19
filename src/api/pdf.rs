use std::fs::write;
use std::path::Path;

use pdf_extract::extract_text;
//use pdf_extract::extract_text_from_mem;

/// error: 定义的错误类型，用于错误传递
use crate::error::MyError;

/// 读取pdf文件，提取文本内容，保存至文本文件
/// 如果遇到特殊字符或错误信息，pdf_extract包内部会打印显示，这个没法设置关闭
pub fn extract_pdf_content(uuid: &str, outpath: &str, pdf_file: &str) -> Result<String, MyError> {
    let pdf_file = format!("{}/{}/{}", outpath, uuid, pdf_file);
    // 检查pdf文件是否在服务端
    let tmp_path = Path::new(&pdf_file);
    if !(tmp_path.exists() && tmp_path.is_file()) {
        return Err(MyError::ParaError{para: format!("no such file in server: {}", pdf_file)})
    }

    // 直接读取pdf文件并提取内容
    let content = extract_text(&pdf_file).map_err(|e| MyError::ExtractPdfError{file: pdf_file.clone(), error: e})?;

    // 去除特殊字符，比如：0x00、0x2009，https://symbl.cc/en/unicode-table/
    let content = content.replace("\u{0000}", "").replace("\u{2009}", "");

    /*
    // 读取pdf文件
    let bytes = read(&paras.pdf).map_err(|e| MyError::ReadFileError{file: paras.pdf_str.clone(), error: e})?;

    // 提取内容
    let content = extract_text_from_mem(&bytes).map_err(|e| MyError::ExtractPdfError{file: paras.pdf_str.clone(), error: e})?;
    */

    // 保存提取的内容，文件名同pdf文件，只是格式后缀改为txt
    let outfile = pdf_file.replace(".pdf", ".txt");
    if let Err(e) = write(&outfile, &content) {
        return Err(MyError::WriteFileError{file: outfile, error: e})
    }

    Ok(content)
}
