use std::collections::HashMap;
use std::fs::{self, read};
use std::io;
use std::path::{Path, PathBuf};

use glob::Pattern;
use ignore::WalkBuilder;
use serde_json::Value;
use termtree::Tree;
use tracing::{event, Level};
use zip::ZipArchive;

/// error: 定义的错误类型，用于错误传递
use crate::error::MyError;

/// 返回结果
pub enum StrucResult {
    Json(Value), // 保存为有层级关系的json格式
    Text(Vec<(String, String)>), // 保存为普通文本格式或没有层级的json格式
}

/// 递归获取指定项目路径下所有文件
pub fn traverse_directory(root_path: &str, include: &Vec<(String, Pattern)>, exclude: &Vec<(String, Pattern)>, mat: bool, check_bytes: usize, max_size: u64, is_json: bool, hierarchical: bool) -> Result<(String, StrucResult), MyError> {
    let root_path = Path::new(root_path);
    // 初始化
    let mut files_json: Value = Value::Array(vec![]); // 存储有层级关系的文件路径、代码内容
    let mut files_text: Vec<(String, String)> = Vec::new(); // 存储Vec<json(代码所在文件的相对路径, 代码内容)>
    let canonical_root_path = root_path.canonicalize()?; // 获取绝对路径
    let parent_directory = match &canonical_root_path.file_name() { // 获取指定路径的文件夹名，`file_name`获取指定path的最后一项
        Some(name) => name.to_string_lossy().to_string(), // 返回指定path的最后一项，可能是文件，也可能是文件夹
        None => canonical_root_path.to_str().unwrap().to_string(), // 指定的path是`/`或以`..`结尾时`file_name`会返回None，此时直接返回指定的path字符串
    };
    let mut not_binary = true; // 用于判断文件是否是二进制文件
    let mut file_size: u64 = 0; // 存储文件大小
    let mut json_map: HashMap<PathBuf, usize> = HashMap::new(); // 存储为有层级关系的json时，存储文件所在路径和所在层级的索引，{key: 路径, value: 所在层级的索引}
    // 创建tree
    let tree = WalkBuilder::new(&canonical_root_path)
        .git_ignore(true)
        .build()
        .filter_map(|e| e.ok())
        .fold(Tree::new(parent_directory.to_owned()), |mut root, entry| { // 遍历指定路径下每一项，以指定路径作为根路径，递归添加子项
            let path = entry.path(); // 当前项的路径
            if let Ok(relative_path) = path.strip_prefix(&canonical_root_path) { // 获取相对路径
                if should_include_file(relative_path, include, exclude, mat) { // 检查当前项是否需要包含
                    // 递归获取指定路径下所有项，创建树结构，用于显示在生成文件的起始
                    let mut current_tree = &mut root; // 当前树结构可变引用
                    let relative_path_components = relative_path.components();
                    let components_num = relative_path_components.clone().count();
                    for component in relative_path_components.clone() { // 遍历当前相对路径的每个父文件夹
                        let component_str = component.as_os_str().to_string_lossy().to_string(); // 转为String
                        // 从当前树结构中获取当前component节点的可变引用，不存在则插入
                        current_tree = if let Some(pos) = current_tree
                            .leaves // Vec<Tree>
                            .iter_mut() // 遍历当前树结构中每个叶子节点
                            .position(|child| child.root == component_str) // 获取叶子节点的父节点与当前component相同的节点在Vec<Tree>中的索引
                        {
                            &mut current_tree.leaves[pos] // 找到pos索引，则当前树结构更新为以该叶子节点为root的树结构，返回可变引用
                        } else { // 此时说明当前component不在当前树结构中
                            let new_tree = Tree::new(component_str.clone()); // 以当前component创建新的tree
                            current_tree.leaves.push(new_tree); // 将刚创建的tree作为叶子节点加入到当前树结构中
                            current_tree.leaves.last_mut().unwrap() // 返回当前树结构中新增的节点的可变引用
                        };
                    }
                    // 当前项如果是文件，且大小在指定上限范围内，则读取代码内容，保存为json，存储到Vec<json>中
                    if path.is_file() {
                        // 判断文件大小
                        file_size = path.metadata().unwrap().len();
                        if file_size <= max_size {
                            // 读取该文件
                            let code_bytes = read(path).unwrap();
                            // 检查该文件是否是二进制文件
                            // 读取文件开头指定数量byte，判断是否`<=0x08`（在ASCII码中该字符及其前面的字符不会出现在文本文件中），满足则说明该文件是二进制文件
                            // https://github.com/dalance/amber/blob/master/src/pipeline_matcher.rs
                            not_binary = true;
                            for byte in code_bytes.iter().take(check_bytes) {
                                if byte <= &0x08 {
                                    //println!("{:?}, {:?}", &0x08, byte);
                                    not_binary = false;
                                    break;
                                }
                            }
                            if not_binary {
                                let code = String::from_utf8_lossy(&code_bytes); // 转为UTF-8
                                // 代码都是有效字符则保存，否则报错
                                if !code.trim().is_empty() {
                                    // 代码中含有无效UTF-8字符则报错，REPLACEMENT_CHARACTER表示无效字符“�”
                                    if code.contains(char::REPLACEMENT_CHARACTER) {
                                        //println!("[warning]: invalid UTF-8 in {}", relative_path.display());
                                        event!(Level::DEBUG, "invalid UTF-8 in {}", relative_path.display());
                                    }
                                    // 保存该文件的文件路径和代码内容
                                    let tmp_code_path = if cfg!(target_os = "windows") { // windows时“\”替换为“/”
                                        format!("{}/{}", parent_directory, relative_path.display()).replace("\\", "/") // 当前文件基于指定路径的相对路径
                                    } else {
                                        format!("{}/{}", parent_directory, relative_path.display()) // 基于指定路径的相对路径
                                    };
                                    let tmp_code_str  = if is_json || hierarchical {
                                        code.to_string() // 输出json格式不需要在前后加“```”，直接输出代码内容
                                    } else {
                                        format!("```{}\n{}\n```", path.extension().and_then(|ext| ext.to_str()).unwrap_or(""), code) // 代码内容
                                    };
                                    if hierarchical { // 保存为有层级关系的json格式
                                        // https://stackoverflow.com/questions/59047280/how-to-build-json-arrays-or-objects-dynamically-with-serde-json
                                        // https://docs.rs/serde_json/1.0.128/serde_json/enum.Value.html#
                                        // https://users.rust-lang.org/t/modify-one-field-of-type-option-serde-json-value-in-a-struct/66349/5
                                        // 获取当前文件所在层级深度
                                        let tmp_path = PathBuf::from(""); // 当前文件各层级路径
                                        let mut tmp_idx: Vec<usize> = vec![]; // 存储当前文件所在各层级路径的索引
                                        // 先遍历当前文件的各层路径（不包括最后一项文件名），不存在则插入空层级
                                        if let Value::Array(ref mut v) = files_json {
                                            for p in relative_path_components.take(components_num-1) {
                                                let _ = tmp_path.join(p);
                                                match json_map.get_mut(&tmp_path) { // 该路径还没写入，则添加
                                                    Some(idx) => {
                                                        tmp_idx.push(*idx);
                                                    },
                                                    None => {
                                                        v.push(Value::Array(vec![])); // 插入空层级
                                                        json_map.insert(tmp_path.clone(), v.len()-1);
                                                        tmp_idx.push(v.len()-1);
                                                    },
                                                }
                                            }
                                            // 将当前文件插入到所在层级
                                            if tmp_idx.len() == 0 { // 指定路径根路径下的文件，直接插入
                                                v.push(Value::Array(vec![Value::String(tmp_code_path), Value::String(tmp_code_str)]));
                                            } else { // 不在指定路径根路径下的其他层级，则需要遍历获取到所在层级然后插入
                                                let mut tmp_v = &mut v[tmp_idx[0]]; // 当前文件在指定路径根路径下的文件夹Value
                                                for i in &tmp_idx[1..] { // 遍历其余层级
                                                    tmp_v = &mut tmp_v[i]; // 获取该层级文件夹Value
                                                }
                                                if let Value::Array(ref mut v_inner) = tmp_v { // 该Value是当前文件所在的文件夹，插入
                                                    v_inner.push(Value::Array(vec![Value::String(tmp_code_path), Value::String(tmp_code_str)]));
                                                }
                                            }
                                        }
                                    } else { // 保存为无层级关系的普通文本或json格式
                                        files_text.push((
                                            tmp_code_path, // 当前文件基于指定路径的相对路径
                                            // 将代码内容前后加上“```”，并在第一行代码前插入一行该代码所在文件的路径，用于插入到结果文件中
                                            // 例如rust脚本，输出为：
                                            // +--------+
                                            // | ```rs  |
                                            // | xxxxxx |
                                            // | xxxxxx |
                                            // | xxxxxx |
                                            // | ```    |
                                            // +--------+
                                            tmp_code_str, // 代码内容
                                        ));
                                    }
                                }
                            } else {
                                //println!("[skip]: this file might be a binary file: {}", relative_path.display());
                                event!(Level::DEBUG, "[skip]: this file might be a binary file: {}", relative_path.display());
                            }
                        } else {
                            //println!("[skip]: file size {} > {}, {}", file_size, max_size, relative_path.display());
                            event!(Level::DEBUG, "[skip]: file size {} > {}, {}", file_size, max_size, relative_path.display());
                        }
                    }
                }
            }
            root
        });
    if hierarchical {
        Ok((tree.to_string(), StrucResult::Json(files_json)))
    } else {
        Ok((tree.to_string(), StrucResult::Text(files_text)))
    }
}

/// 根据指定include和exclude，判断指定文件是否要包含在结果中
fn should_include_file(path: &Path, include_patterns: &[(String, Pattern)], exclude_patterns: &[(String, Pattern)], mat: bool) -> bool {
    // 获取指定路径字符串
    let path_str = path.to_str().unwrap();
    // 检查该文件是否满足include和exclude中指定的条件
    let included = include_patterns.iter().any(|pattern| pattern.1.matches(path_str));
    let excluded = exclude_patterns.iter().any(|pattern| pattern.1.matches(path_str));
    // 打印include和exclude匹配结果
    if mat {
        // 打印include匹配结果
        for (p_str, pattern) in include_patterns.iter() {
            println!("[include]: pattern={}, match={}, path={}", p_str, pattern.matches(path_str), path_str);
        }
        // 打印exclude匹配结果
        for (p_str, pattern) in exclude_patterns.iter() {
            println!("[exclude]: pattern={}, match={}, path={}", p_str, pattern.matches(path_str), path_str);
        }
    }
    // 判断指定文件是否要包含在结果中
    match (included, excluded) {
        (true, true) => true, // 如果include和exclude都满足，include优先
        (true, false) => true, // 仅满足include
        (false, true) => false, // 仅满足exclude
        (false, false) => include_patterns.is_empty(), // 都不满足，且没有指定include，则都包含
    }
}

/// 解压缩zip文件，https://github.com/zip-rs/zip2/blob/master/examples/extract.rs
pub fn unzip(zip_file: &str, uuid: &str, outpath: &str) -> Result<(), MyError> {
    let zip_file_path = Path::new(zip_file);
    let file = fs::File::open(zip_file_path).map_err(|e| MyError::OpenFileError{file: zip_file.to_string(), error: e})?;
    let mut archive = ZipArchive::new(file).map_err(|e| MyError::ZipArchiveError{file: zip_file.to_string(), error: e})?;
    // 遍历每项
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| MyError::ZipArchiveError{file: zip_file.to_string(), error: e})?;
        let outpath: PathBuf = match file.enclosed_name() {
            Some(path) => [outpath, uuid, path.to_str().unwrap()].iter().collect(), // 加上`指定输出路径/uuid`前缀
            None => continue,
        };

        /*
        {
            let comment = file.comment();
            if !comment.is_empty() {
                println!("File {i} comment: {comment}");
            }
        }
        */

        if file.is_dir() { // 是路径，则创建路径
            fs::create_dir_all(&outpath).map_err(|e| MyError::CreateDirAllError{dir_name: outpath.to_str().unwrap().to_string(), error: e})?;
        } else { // 是文件，则保存文件
            //println!("File {} extracted to \"{}\" ({} bytes)", i, outpath.display(), file.size());
            event!(Level::DEBUG, "File {} extracted to \"{}\" ({} bytes)", i, outpath.display(), file.size());
            // 该文件所在路径不存在则创建
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| MyError::CreateDirAllError{dir_name: p.to_str().unwrap().to_string(), error: e})?;
                }
            }
            // 保存该文件
            let mut outfile = fs::File::create(&outpath).map_err(|e| MyError::CreateFileError{file: outpath.to_str().unwrap().to_string(), error: e})?;
            io::copy(&mut file, &mut outfile)?;
        }

        // Get and Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }
    }
    Ok(())
}
