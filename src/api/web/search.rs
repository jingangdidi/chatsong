use std::fs::write;

use chrono::Local;
use tracing::{event, Level};

/// info: 记录所有用户的信息
/// web: 网络搜索、解析指定的url和html
/// error: 定义的错误类型，用于错误传递
use crate::{
    info::chinese_ratio, // 计算指定字符串中含有的非ACSII英文字符的比例
    parse_paras::PARAS,
    web::request_url::{parse_all_url, request_search_api}, // 解析指定的url，以及使用google api进行搜索
    error::MyError,
};

/// 解析客户端输入的内容，使用网络搜索、解析url，返回界限结果和报错字符串
/// 格式：`[url] xxx`
/// 说明：
///     1. `[]`表示可选
///     2. `[url]`指定url，多个用空格间隔，url以http开头才会识别到
///     3. 如果指定了`[url]`，则不对query进行网络搜索，而是基于解析的url内容进行提问
///     4. 服务端会根据指定问题中含有的非ASCII英文字母占比来判断使用中文还是英文进行提问，默认>0.1则使用中文，否则使用英文
///     5. 搜索或解析结果会保存至`指定输出路径/uuid/时间戳.search`
/// 示例：
///     1. 以`xxx`作为问题进行网络搜索，默认前10个结果：`xxx`
///     2. 解析3个url，以`xxx`为问题对3个url的解析结果进行提问：`http://url1 https://url2 http://url3 xxx`
/// 判断过程：
///     根据空格拆分，判断每项是否以http开头，如果以httl开头，则作为url进行解析，如果不以http开头，则该项及之后的所有想合并作为问题
///     a. 如果提取到url，则解析url，不进行网络搜索，用非url的部分作为问题，对解析的url内容进行提问
///     b. 如果没有提取到url，即空格间隔的第1项不以http开头，则提问内容都作为问题，进行网络搜索
pub fn get_search_parse_result(uuid: &str, q: String) -> (Option<String>, String) {
    let para_vec: Vec<&str> = q.split(" ").collect();
    let mut urls: Vec<String> = vec![];
    let mut query = "".to_string();
    for (i, p) in para_vec.iter().enumerate() {
        if p.starts_with("http") {
            urls.push(p.to_string());
        } else {
            query = para_vec[i..].join(" ").to_string(); // 把之后所有项合并起来作为问题
            break
        }
    }
    // 是否使用英文
    let is_en = if chinese_ratio(&query) < 0.1 {
        true
    } else {
        false
    };
    if urls.len() == 0 { // 没有指定url，直接用问题进行网络搜索
        if PARAS.engine_key.is_empty() {
            (None, "When using a web search, it is essential to specify the -e parameter in command line or google_engine_key in config.txt".to_string())
        } else if PARAS.search_key.is_empty() {
            (None, "When using a web search, it is essential to specify the -s parameter in command kine or google_search_key in config.txt".to_string())
        } else {
            match search_web(uuid, &query, "10", is_en) { // 这里固定取前10个搜索结果
                Ok(res) => (Some(res), "".to_string()),
                Err(e) => {
                    event!(Level::ERROR, "{} {}", uuid, e);
                    (None, format!("Search error: {}", e))
                },
            }
        }
    } else { // 指定了url，则解析url，用问题基于解析的url内容进行提问
        match search_url_html(uuid, &query, &urls, is_en) {
            Ok(res) => (Some(res), "".to_string()),
            Err(e) => {
                event!(Level::ERROR, "{} {}", uuid, e);
                (None, format!("Search error: {}", e))
            },
        }
    }
}

/// 对指定内容进行网络搜索，返回搜索结果字符串
fn search_web(uuid: &str, query: &str, num: &str, is_en: bool) -> Result<String, MyError> {
    // 先调用google api对指定问题进行搜索，获取指定数量搜索结果的url
    let search_result = request_search_api(query, num)?; // Vec<(title, link)>
    let all_links: Vec<String> = search_result.iter().map(|res| res.1.to_string()).collect();

    // 解析每个url，返回每个url解析结果字符串Vec
    let parsed_result = parse_all_url(&all_links)?;

    // 保存搜索结果至`指定输出路径/uuid/时间戳.search`，存储问题以及每个搜索结果的title、link、解析结果
    let search_outfile = format!("{}/{}/{}.search", PARAS.outpath, uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    if let Err(e) = write(&search_outfile, format!("{}\n\n{}\n", query, search_result.into_iter().zip(parsed_result.clone()).map(|(res, parse)| res.0+"\t"+&res.1+"\n"+&parse+"\n").collect::<Vec<_>>().join("\n"))) {
        return Err(MyError::WriteFileError{file: search_outfile, error: e})
    }

    // 在解析结果的最后加上问题
    add_prompt(query, parsed_result, is_en)
}

/// 对指定url进行解析，返回解析结果字符串
fn search_url_html(uuid: &str, query: &str, urls: &Vec<String>, is_en: bool) -> Result<String, MyError> {
    // 对指定url进行解析
    let parsed_url_result = parse_all_url(urls)?;

    // 检查是否有结果
    if parsed_url_result.len() == 0 {
        return Err(MyError::WebSearchError{info: "no search result".to_string()})
    };

    // 保存搜索结果至`指定输出路径/uuid/时间戳.search`，存储问题、每个url解析结果
    let search_outfile = format!("{}/{}/{}.search", PARAS.outpath, uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    if let Err(e) = write(&search_outfile, format!("{}\n\n{}\n", query, urls.iter().zip(parsed_url_result.clone()).map(|(u, parse)| u.to_string()+"\n"+&parse+"\n").collect::<Vec<_>>().join("\n"))) {
        return Err(MyError::WriteFileError{file: search_outfile, error: e})
    }

    // 在解析结果的最后加上问题
    add_prompt(query, parsed_url_result, is_en)
}

/// 对指定url进行解析，返回解析结果字符串
/*
fn search_url(uuid: &str, query: &str, urls: &Vec<String>, is_en: bool) -> Result<String, MyError> {
    // 对指定url进行解析
    let parsed_result = parse_all_url(urls)?;

    // 保存搜索结果至`指定输出路径/uuid/时间戳.search`，存储问题、每个url、解析结果
    let search_outfile = format!("{}/{}/{}.search", PARAS.outpath, uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    if let Err(e) = write(&search_outfile, format!("{}\n\n{}\n", query, urls.iter().zip(parsed_result.clone()).map(|(url, parse)| url.to_string()+"\n"+&parse+"\n").collect::<Vec<_>>().join("\n"))) {
        return Err(MyError::WriteFileError{file: search_outfile, error: e})
    }

    // 在解析结果的最后加上问题
    add_prompt(query, parsed_result, is_en)
}
*/

/// 对指定html文件进行解析，返回解析结果字符串
/*
fn parse_html_file(uuid: &str, query: &str, htmls: &Vec<String>, is_en: bool) -> Result<String, MyError> {
    // 对指定html文件进行解析
    let parsed_result = parse_all_html(htmls)?;

    // 保存搜索结果至`指定输出路径/uuid/时间戳.search`，存储问题、每个url、解析结果
    let search_outfile = format!("{}/{}/{}.search", PARAS.outpath, uuid, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    if let Err(e) = write(&search_outfile, format!("{}\n\n{}\n", query, htmls.iter().zip(parsed_result.clone()).map(|(html, parse)| html.to_string()+"\n"+&parse+"\n").collect::<Vec<_>>().join("\n"))) {
        return Err(MyError::WriteFileError{file: search_outfile, error: e})
    }

    // 在解析结果的最后加上问题
    add_prompt(query, parsed_result, is_en)
}
*/

/// 在解析结果的最后加上问题
fn add_prompt(query: &str, parsed_result: Vec<String>, is_en: bool) -> Result<String, MyError> {
    // 合并所有解析结果
    let mut result = parsed_result.join("\n");

    // 提问的prompt
    let prompt = if is_en { // 英文prompt，参考：https://www.reddit.com/r/ChatGPT/comments/11twe7z/prompt_to_summarize/
        "\nBased on the given text, provide a concise and comprehensive answer for my question, ensure that the answer is well-organized and easy to read. My question is: ".to_string() + query
    } else { // 中文prompt
        "\n基于以上内容回答我的问题，答案要简洁、明确，但不能丢失核心内容，也绝对不能杜撰内容，同时要易于理解。我的问题是：".to_string() + query
    };

    // 最后加上prompt
    result += &prompt;
    Ok(result)
}
