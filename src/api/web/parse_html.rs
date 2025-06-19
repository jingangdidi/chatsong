use std::fs::read;

use scraper::{Html, node::Node};

/// error: 定义的错误类型，用于错误传递
use crate::error::MyError;

/// 解析单个html页面字符串，提取Text，例如SingleFile保存的单个html
// +---------------------------------------------------+
// | pub enum Node {                                   |
// |     Document,                                     |
// |     Fragment,                                     |
// |     Doctype(Doctype),                             |
// |     Comment(Comment),                             |
// |     Text(Text),                                   |
// |     Element(Element),                             |
// |     ProcessingInstruction(ProcessingInstruction), |
// | }                                                 |
// +----------------------------+----------------------+
// | pub struct Element {       | https://docs.rs/scraper/0.20.0/scraper/node/struct.Element.html#
// |     pub name: QualName,    |
// |     pub attrs: Attributes, |
// |     /* private fields */   |
// | }                          |
// +----------------------------+
// // node格式示例：
// +--------------------------------------------------------------------------+
// | NodeRef {                                                                | https://docs.rs/ego-tree/0.6.2/ego_tree/struct.NodeRef.html
// |     id: NodeId(4207),                                                    |
// |     tree: Tree {                                                         |
// |         vec: [                                                           |
// |             Node {                                                       |
// |                 parent: None,                                            |
// |                 prev_sibling: None,                                      |
// |                 next_sibling: None,                                      |
// |                 children: Some((NodeId(2), NodeId(3))),                  |
// |                 value: Document                                          |
// |             },                                                           |
// |             Node {                                                       |
// |                 parent: Some(NodeId(1)),                                 |
// |                 prev_sibling: None,                                      |
// |                 next_sibling: Some(NodeId(3)),                           |
// |                 children: None,                                          |
// |                 value: Doctype(<!DOCTYPE html PUBLIC "" "">)             |
// |             },                                                           |
// |             Node {                                                       |
// |                 parent: Some(NodeId(3)),                                 |
// |                 prev_sibling: None,                                      |
// |                 next_sibling: Some(NodeId(5)),                           |
// |                 children: None,                                          |
// |                 value: Comment(<!-- "\n Page saved with SingleFile" -->) |
// |             },                                                           |
// |             Node {                                                       |
// |                 parent: Some(NodeId(5)),                                 |
// |                 prev_sibling: None,                                      |
// |                 next_sibling: Some(NodeId(7)),                           |
// |                 children: None,                                          |
// |                 value: Element(<meta charset="utf-8">)                   |
// |             },                                                           |
// |             Node {                                                       |
// |                 parent: Some(NodeId(5)),                                 |
// |                 prev_sibling: Some(NodeId(6)),                           |
// |                 next_sibling: Some(NodeId(8)),                           |
// |                 children: None,                                          |
// |                 value: Text("\n")                                        |
// |             },                                                           |
// +--------------------------------------------------------------------------+
pub fn parse_single_html_str(html_str: &str, debug: bool) -> Result<String, MyError> {
    // 存储解析结果字符串
    let mut result: String = "".to_string();

    // 解析读取的html
    let document = Html::parse_document(html_str);

    // 绝对每个node的value是否保留
    let mut last_white_space = false; // 上一个Text是否是空格或换行，用于去除连续的空格和换行
    let mut current_white_space: bool; // 当前Text是否是空格或换行，用于去除连续的空格和换行
    let mut tmp_last: bool; // 如果保存当前Text，则将当前赋给last_white_space
    let mut keep: bool; // 是否要保存本次，当上一次是true，本次还是true，则去除当前，否则保存当前，并将当前赋给last_white_space

    // 遍历每个node
    // 关于nodes和values：https://docs.rs/ego-tree/0.6.2/ego_tree/struct.Tree.html#method.nodes
    for (v, n) in document.tree.values().zip(document.tree.nodes()) {
        if let Node::Text(t) = v { // 仅获取Text，其他类型舍弃
            // 判断当前node的Text是否与上次保存的Text是连续的空格或换行，决定是否保存当前Text
            current_white_space = t.trim().is_empty(); // 本次是否是空格或换行
            (tmp_last, keep) = match (last_white_space, current_white_space) { // 是否要保存本次
                (true, true)   => (true, false), // 上次和本次都是空格或换行，不保存本次
                (true, false)  => (false, true), // 上次是空格或换行，本次不是，保存本次，并将last_white_space设为false供下次使用
                (false, true)  => (true, true),  // 上次不是空格或换行，本次是，保存本次，并将last_white_space设为true供下次使用
                (false, false) => (false, true), // 上次和本次都不是空格或换行，保存本次
            };
            // 当前node的parent是否是style（用于过滤掉style），以及parent的Element的name（用于debug）
            let (not_style, parent_value) = match n.parent() {
                //Some(pv) => Some(pv.value()), // 不进一步解析
                Some(pv) => match pv.value() { // 进一步解析当前node的parent的Element
                    Node::Element(e) => {
                        let tmp_name = e.name();
                        match tmp_name {
                            "style" | "script" | "button" | "svg" => (false, Some(tmp_name)),
                            "footer" => {
                                if debug {
                                    (false, Some(tmp_name))
                                } else { // 舍弃footer之后的内容
                                    break
                                }
                            },
                            _ => (true, Some(tmp_name)),
                        }
                    },
                    _ => (true, None),
                },
                None => (true, None),
            };
            // 输出结果
            if debug { // 每次打印3行，第1行Text内容，第2行srxsrx，第3行当前node的parent的Element，多次打印之间空行间隔
                println!("{:?}\nsrxsrx\n{:?}\n", t, parent_value);
            } else {
                if not_style && keep {
                    //print!("{}", t.to_string());
                    result += &t.to_string();
                    last_white_space = tmp_last; // 真正保存了当前Text时，更新last_white_space
                } else if tmp_last {
                    last_white_space = tmp_last; // 当前Text是空格或换行，也更新last_white_space
                }
            }
        }
    }
    Ok(result)
}

/// 解析指定的多个html文件，返回每个文件解析结果字符串Vec
pub fn parse_all_html(html_files: &Vec<String>) -> Result<Vec<String>, MyError> {
    // 存储每个html文件的解析结果字符串
    let mut result: Vec<String> = vec![];

    // 遍历指定的每个html文件进行解析
    for h in html_files {
        let html_content = String::from_utf8(read(h)?).map_err(|e| MyError::FileContentToUtf8Error{file: h.to_string(), error: e})?; // 读取单个html文件，例如使用SingleFile保存的单个html文件
        result.push(parse_single_html_str(&html_content, false)?);
    }
    Ok(result)
}
