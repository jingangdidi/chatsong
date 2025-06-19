use std::collections::HashMap;

use reqwest::blocking::Client;
use serde_json::{from_str, Value};

/// PARAS: 参数
/// parse_html: 解析指定的html文件
/// error: 定义的错误类型，用于错误传递
use crate::{
    parse_paras::PARAS,
    web::parse_html::parse_single_html_str,
    error::MyError,
};

/// 获取指定url页面的所有内容
/// 需要使用：`reqwest = { version = "0.12", features = ["blocking"] }`
/// https://docs.rs/reqwest/0.12.7/reqwest/blocking/struct.Client.html
/// https://stackoverflow.com/questions/58906965/could-not-find-blocking-in-reqwest
fn request_html_page(url: &str) -> Result<(bool, String), MyError> {
    let client = Client::new();
    /* 这里请求失败直接报错退出了
    let response = client.get(url).send().map_err(|e| MyError::SendRequestError{url: url.to_string(), error: e})?;
    Ok(response.text().map_err(|e| MyError::GetResponseTextError{url: url.to_string(), error: e})?)
    */
    // 请求失败不报错退出，返回(是否成功, 相应内容或错误信息)，后面会解析相应内容或打印错误信息
    match client.get(url).send() {
        Ok(response) => Ok((true, response.text().map_err(|e| MyError::GetResponseTextError{url: url.to_string(), error: e})?)),
        Err(e) => Ok((false, format!("{:?}", e))),
    }
}

/// 解析指定的多个url，返回每个url解析结果字符串Vec
pub fn parse_all_url(urls: &Vec<String>) -> Result<Vec<String>, MyError> {
    // 存储每个url的解析结果字符串
    let mut result: Vec<String> = vec![];

    // 遍历指定的每个url进行解析
    for (_i, u) in urls.iter().enumerate() {
        let (ok, page_content) = request_html_page(u)?; // 获取url页面内容
        if ok {
            result.push(parse_single_html_str(&page_content, false)?); // 解析url页面内容
            //println!("[✓] parse url {} success: {}", i+1, u);
        } else {
            result.push("".to_string()); // 解析url页面内容
            //println!("[x] parse url {} failed: {} {}", i+1, u, page_content);
        }
    }
    Ok(result)
}

/// 向搜索引擎api提交搜索
/// +------------------+
/// | c2coff           | 启用或停用简体中文和繁体中文搜索。1表示已停用，0表示已启用（默认）
/// | cr               | 将搜索结果限制为来自特定国家/地区的文档。例如：中国(countryCN)、法国(countryFR)、德国(countryDE)
/// | cx               | 用于此请求的可编程搜索引擎ID。
/// | dateRestrict     | 根据日期将结果限制为网址。支持的值包括：
/// |                  | d[number]：请求过去指定天数的结果。
/// |                  | w[number]：请求过去指定周数的结果。
/// |                  | m[number]：请求过去指定月份数的结果。
/// |                  | y[number]：请求过去指定年份的结果。
/// | exactTerms       | 标识搜索结果中所有文档必须包含的词组。
/// | excludeTerms     | 标识不应在搜索结果的任何文档中出现的字词或词组。
/// | fileType         | 将结果限制为指定扩展名的文件。例如：.pdf、.csv、.xls、.xlsx、.ppt、.pptx、.doc、.docx、.txt、.py
/// | filter           | 用于开启或关闭重复内容过滤器的控件。默认情况下，Google会对所有搜索结果应用过滤，以提高这些结果的质量。0表示关闭重复内容过滤器，1表示开启重复内容过滤器。
/// | gl               | 最终用户的地理位置。两个字母的国家/地区代码。例如：中国(cn)、法国(fr)、德国(de)
/// | googlehost       | 已弃用。使用gl形参可实现类似的效果。用于执行搜索的本地Google网域，例如：google.com、香港(google.hk)、法国(google.fr)、德国(google.de)
/// | highRange        | 指定搜索范围的结束值。使用lowRange和highRange可为查询附加lowRange...highRange的包容性搜索范围。
/// | hl               | 设置界面语言。明确设置此参数可提高搜索结果的效果和质量。例如：中文（简体）(zh-CN)、中文（繁体）(zh-TW)、英语(en)、法语(fr)、德语(de)
/// | hq               | 向查询附加指定的查询字词，就像使用逻辑AND运算符合并这些字词一样。
/// | imgColorType     | 返回黑白、灰度、透明或彩色图片。可接受的值为：color、gray、mono(黑白)、trans(透明背景)
/// | imgDominantColor | 返回特定主色的图片。可接受的值为：black、blue、brown、gray、green、orange、pink、purple、red、teal、white、yellow
/// | imgSize          | 返回指定尺寸的图片。可接受的值为：huge、icon、large、medium、small、xlarge、xxlarge
/// | imgType          | 返回某个类型的图片。可接受的值为：clipart、face、lineart、stock、photo、animated
/// | linkSite         | 指定所有搜索结果都应包含指向特定网址的链接。
/// | lowRange         | 指定搜索范围的起始值。使用lowRange和highRange可为查询附加lowRange...highRange的包容性搜索范围。
/// | lr               | 将搜索范围限制为以特定语言撰写的文档。例如：lr=lang_zh-CN、lr=lang_en、lr=lang_de
/// |                  | 可接受的值为：lang_ar(阿拉伯语)、lang_bg(保加利亚语)、lang_ca(加泰罗尼亚语)、lang_cs(捷克语)、lang_da(丹麦语)、lang_de(德语)、lang_el(希腊语)、lang_en(英语)、lang_es(西班牙语)、lang_et(爱沙尼亚语)、lang_fi(芬兰语)、lang_fr(法语)、lang_hr(克罗地亚语)、lang_hu(匈牙利语)、lang_id(印度尼西亚语)、lang_is(冰岛语)、lang_it(意大利语)、lang_iw(希伯来语)、lang_ja(日语)、lang_ko(韩语)、lang_lt(立陶宛语)、lang_lv(拉脱维亚语)、lang_nl(荷兰语)、lang_no(挪威语)、lang_pl(波兰语)、lang_pt(葡萄牙语)、lang_ro(罗马尼亚语)、lang_ru(俄语)、lang_sk(斯洛伐克语)、lang_sl(斯洛文尼亚语)、lang_sr(塞尔维亚语)、lang_sv(瑞典语)、lang_tr(土耳其语)、lang_zh-CN(简体中文)、lang_zh-TW(繁体中文)
/// | num              | 要返回的搜索结果数。有效值为1到10之间的整数（包括1和10）。
/// | orTerms          | 提供要在文档中检查的其他搜索字词，其中搜索结果中的每个文档都必须至少包含一个其他搜索字词。
/// | q                | 查询的内容
/// | relatedSite      | 已弃用
/// | rights           | 基于许可的过滤条件。支持的值包括：cc_publicdomain、cc_attribute、cc_sharealike、cc_noncommercial、cc_nonderived，以及这些值的组合。
/// | safe             | 搜索安全级别。可接受的值为：active(启用安全搜索过滤功能)、off(停用安全搜索过滤功能，默认)
/// | searchType       | 指定搜索类型：image。如果未指定，则结果将仅限于网页。可接受的值为：image(自定义图片搜索)
/// | siteSearch       | 指定应始终从结果中包含或排除的给定网站。
/// | siteSearchFilter | 控制是否包含或排除siteSearch参数中指定的网站的结果。可接受的值为：e(排除)、i(包含)
/// | sort             | 要应用于结果的排序表达式。排序参数指定根据指定表达式对结果进行排序，即按日期排序。例如：sort=date
/// | start            | 要返回的第一个结果的索引。每页的默认结果数为10，因此&start=11将从结果第二页的顶部开始。注意：JSON API绝不会返回超过100个结果，即使与查询匹配的文档超过100个，因此将start+num的总和设置为大于100的数字也会产生错误。另请注意，num的最大值为10。
/// | key:             | API密钥
/// +------------------+
/// 注意：搜索请求的长度限制应在2048个字符以内。
/// url格式：https://www.googleapis.com/customsearch/v1?[parameters]
/// 例如：https://www.googleapis.com/customsearch/v1?key=YOUR_API_KEY&cx=SEARCH_ENGINE_KEY&q=QUERY
/// 实际示例：https://www.googleapis.com/customsearch/v1?cx=912b8adxxxx8e41a9&q=how+to+use+cubecl&num=10&key=AIzaSyAOi2Dxxxxrv0cZKcl0RX8WLs70-vQwiBM
/// 参数详见：https://developers.google.com/custom-search/v1/reference/rest/v1/cse/list?hl=zh-cn
pub fn request_search_api(query: &str, num: &str) -> Result<Vec<(String, String)>, MyError> {
    // 搜索引擎地址
    let base_url: &str = "https://www.googleapis.com/customsearch/v1";

    // 准备搜索参数
    // https://webscraping.ai/faq/reqwest/how-do-i-pass-parameters-in-a-reqwest-get-request
    // https://stackoverflow.com/questions/71746830/build-query-string-with-param-having-multiple-values-for-a-reqwest-client
    let mut params: HashMap<&str, &str> = HashMap::new();
    params.insert("cx", &PARAS.engine_key);
    params.insert("key", &PARAS.search_key);
    params.insert("num", num);
    params.insert("q", query);
    //params.insert("gl", "cn"); // 好像没用
    //params.insert("lr", "lang_zh-CN"); // 好像没用

    // 提交搜索
    let client = Client::new();
    // let response = client.get(base_url).query(&params).send().map_err(|e| MyError::SendRequestError{url: base_url.to_string(), error: e})?; // 这里一次请求失败就报错退出
    // 尝试请求5次，依然失败再报错退出
    let mut counter = 0;
    let response = loop {
        counter += 1;
        match client.get(base_url).query(&params).send() {
            Ok(res) => break res, // 发送请求成功，跳出循环并返回响应结果
            Err(e) => { // 发送请求失败
                if counter < 6 { // 尝试了5次以内，打印提示，继续
                    println!("[x] try{}, sending request to google api error: {}", counter, e);
                } else { // 已尝试了5次还是失败，报错退出
                    return Err(MyError::SendRequestError{url: base_url.to_string(), error: e})
                }
            },
        }
        if counter == 5 { // 已尝试5次则报错退出
            return Err(MyError::WebSearchError{info: "Already tried to send request 5 times but still failed".to_string()})
        }
    };

    // 搜索结果转json，示例见`google_search_测试结果.json`
    // https://www.shuttle.rs/blog/2024/01/18/parsing-json-rust
    let response_text = response.text().map_err(|e| MyError::GetResponseTextError{url: base_url.to_string(), error: e})?;
    let json_result: Value = from_str(&response_text).map_err(|e| MyError::ResponseTextToJsonError{error: e})?;

    // 存储搜索结果
    let mut result: Vec<(String, String)> = vec![]; // 存储每个搜索结果的title和link，Vec<(title1, link1), (title2, link2), ...>

    // 获取每个搜索结果的title和link
    if let Some(items) = json_result["items"].as_array() {
        for i in items {
            result.push((
                i["title"].to_string().trim_matches('"').to_string(), // 去除json字符串前后的`"`
                i["link"].to_string().trim_matches('"').to_string(), // 去除json字符串前后的`"`
            ));
        }
    } else {
        //println!("[!] Warning: no search result");
        return Err(MyError::WebSearchError{info: "no search result".to_string()})
    }

    Ok(result)
}
