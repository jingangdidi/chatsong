use std::fs::write;
use std::path::Path;

use glob::Pattern;
use serde_json::json;

/// traverse: 递归获取指定路径下所有文件，并读取其中的代码
/// error: 定义的错误类型，用于错误传递
use crate::{
    code::traverse::{StrucResult, traverse_directory, unzip},
    graph::copy_file_from_related_uuid, // 如果指定文件不在指定uuid的路径下，则去该uuid所有相关uuid路径下寻找，复制到指定uuid路径下
    error::MyError,
};

/// 从上传的代码压缩包中获取所有脚本的代码，合并到一起，作为提问的问题
pub fn merge_code(uuid: &str, query: &str, outpath: &str) -> Result<String, MyError> {
    // 解析参数，以`code `起始，中间可选的是参数，最后是压缩文件名，全部用空格间隔
    // 格式：`code [include:xxx] [exclude:xxx] [check_bytes:xxx] [max_size:xxx] [to_json:xxx] [hierarchical:xxx] [contain_tree:xxx] xxx.zip`
    // include: 指定的匹配模式，用于获取要包含的文件
    // exclude: 指定的匹配模式，用于获取要排除的文件
    // check_bytes: 检查文件前几个byte来缺失是否是二进制文件，默认50
    // max_size: 要包含的文件大小上限，大小大于该值的文件不读取，支持4种单位b、k(1024b)、m(1024k)、g(1024m)，都是小写，例如：15b、500k、200m、4g，默认1m，0表示无限制（此时单位无所谓）
    // to_json: 获取为json格式，yes或no，默认yes
    // hierarchical: 是否获取为有层级结构的json格式，yes或no，默认yes
    // contain_tree: 是否在获取的结果中包含脚本文件树结构，yes或no，默认yes
    let mut include: Vec<(String, Pattern)> = vec![]; // (指定的pattern字符串, 创建的pattern对象)
    let mut exclude: Vec<(String, Pattern)> = vec![]; // (指定的pattern字符串, 创建的pattern对象)
    let mut check_bytes: usize = 50;
    let mut max_size: u64 = 1024*1024;
    let mut to_json = true;
    let mut hierarchical = true;
    let mut contain_tree = true;
    let mut tmp_para: Vec<&str> = query.split(" ").collect();
    // 最后一项作为zip文件
    let zip_file = tmp_para.pop().unwrap(); // 肯定不是空向量，因此这里直接unwrap
    copy_file_from_related_uuid(uuid, zip_file);
    let code_zip: String = format!("{}/{}/{}", outpath, uuid, zip_file); // 肯定不是空向量，因此这里直接unwrap
    if !code_zip.ends_with(".zip") {
        return Err(MyError::ParaError{para: format!("The last parameter is not a compressed file: {}", code_zip)})
    }
    // 检查zip文件是否在服务端
    let tmp_path = Path::new(&code_zip);
    if !(tmp_path.exists() && tmp_path.is_file()) {
        return Err(MyError::ParaError{para: format!("no such file in server: {}", code_zip)})
    }
    // 遍历解析其他参数
    for para in &tmp_para[1..] {
        if para.starts_with("include:") {
            for p in para.strip_prefix("include:").unwrap().split(",") {
                include.push((p.to_string(), Pattern::new(p).map_err(|e| MyError::CreatePatternError{pattern: p.to_string(), error: e})?));
            }
        } else if para.starts_with("exclude:") {
            for p in para.strip_prefix("exclude:").unwrap().split(",") {
                exclude.push((p.to_string(), Pattern::new(p).map_err(|e| MyError::CreatePatternError{pattern: p.to_string(), error: e})?));
            }
        } else if para.starts_with("check_bytes:") {
            let num = para.strip_prefix("check_bytes:").unwrap();
            check_bytes = match num.parse::<usize>() {
                Ok(b) => {
                    if b > 0 {
                        b
                    } else {
                        return Err(MyError::ParaError{para: "check_bytes must > 0".to_string()})
                    }
                },
                Err(e) => return Err(MyError::ParseStringError{from: num.to_string(), to: "usize".to_string(), error: e}),
            };
        } else if para.starts_with("max_size:") {
            let mut num = para.strip_prefix("max_size:").unwrap().to_string();
            if let Some(p) = num.pop() {
                max_size = match num.parse::<u64>() { // 这里p是指定参数的最后一个字符
                    Ok(n) => match p { // 这里n是指定参数的数值
                        'b' => {
                            if n == 0 {
                                u64::MAX
                            } else {
                                n
                            }
                        },
                        'k' => n*1024,
                        'm' => n*1024*1024,
                        'g' => n*1024*1024*1024,
                        _ => return Err(MyError::ParaError{para: format!("max_size suffix only support b, k, m, g, not {}", p)}),
                    },
                    Err(e) => return Err(MyError::ParseStringError{from: num.to_string(), to: "u64".to_string(), error: e}),
                }
            }
        } else if *para == "to_json:no" {
            to_json = false;
        } else if *para == "hierarchical:no" {
            hierarchical = false;
        } else if *para == "contain_tree:no" {
            contain_tree = false;
        } else {
            return Err(MyError::ParaError{para: format!("no such parameter for `code` mode: {}", para)})
        }
    }
    // 解压缩指定的代码压缩文件
    unzip(&code_zip, uuid, outpath)?;
    let code_path = code_zip.strip_suffix(".zip").unwrap();

    // 支持的pattern，用于选取文件
    // https://docs.rs/glob/latest/glob/struct.Pattern.html#method.matches
    /*
    1. `?` matches any single character.
    2. `*` matches any (possibly empty) sequence of characters.
    3. `**` matches the current directory and arbitrary subdirectories. This sequence must form a single path component, so both `**a` and `b**` are invalid and will result in an error. A sequence of more than two consecutive `*` characters is also invalid.
    4. `[...]` matches any character inside the brackets. Character sequences can also specify ranges of characters, as ordered by Unicode, so e.g. `[0-9]` specifies any character between 0 and 9 inclusive. An unclosed bracket is invalid.
    5. `[!...]` is the negation of `[...]`, i.e. it matches any characters not in the brackets.
    6. The metacharacters `?`, `*`, `[`, `]` can be matched by using brackets (e.g. `[?]`). When a `]` occurs immediately following `[` or `[!` then it is interpreted as being part of, rather then ending, the character set, so `]` and NOT `]` can be matched by `[]]` and `[!]]` respectively. The `-` character can be specified inside a character sequence pattern by placing it at the start or the end, e.g. `[abc-]`.
    */

    // 递归指定的路径
    let (tree, result_files) = traverse_directory(code_path, &include, &exclude, false, check_bytes, max_size, to_json, hierarchical)?;

    // 获取结果
    let result = match result_files {
        StrucResult::Json(files) => { // 保存为有层级的json格式
            let json_output = if contain_tree {
                json!({
                    "tree": tree,
                    "code": files,
                })
            } else {
                json!(files)
            };
            serde_json::to_string_pretty(&json_output).map_err(|e| MyError::JsonToStringError{error: e.into()})?
        },
        StrucResult::Text(files) => { // 保存为没有层级的json格式
            if to_json {
                let json_output = if contain_tree {
                    json!({
                        "tree": tree,
                        "code": files,
                    })
                } else {
                    json!(files)
                };
                serde_json::to_string_pretty(&json_output).map_err(|e| MyError::JsonToStringError{error: e.into()})?
            } else { // 保存为普通文本格式
                format!("Source Tree:\n```\n{}```\n\n{}", tree, files.into_iter().map(|v| format!("`{}`\n{}\n", v.0, v.1)).collect::<Vec<_>>().join("\n"))
            }
        },
    };

    // 保存至输出文件
    let outfile = code_zip.replace(".zip", ".txt");
    if let Err(e) = write(&outfile, &result) {
        return Err(MyError::WriteFileError{file: outfile, error: e})
    }

    Ok(result)
}
