use axum::{
    extract::OriginalUri,
    response::Html,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{event, Level};
use uuid::Uuid;

/// info::create_cookie: 创建cookie
/// html_page: 生成主页面html字符串
use crate::{
    info::{
        create_cookie, // 根据指定uuid创建cookie
        update_cookie_max_age, // 更新指定CookieJar的max-age
        contain_uuid, // 判断指定uuid是否已存在于DATA中，不存在则尝试从服务端存储路径下读取
    },
    html_page::{create_main_page_ch, create_main_page_en},
    parse_paras::PARAS, // 存储命令行参数的全局变量
};

/// Handler for `/嵌套的前缀` GET
/// 访问chat界面
pub async fn index(uri: OriginalUri, jar: CookieJar) -> (CookieJar, Html<String>) {
    event!(Level::INFO, "GET {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    // 获取cookie，即uuid
    let uuid = match jar.get("srx-tzn") { // 获取cookie
        Some(c) => c.value().to_string(), // 有cookie则cookie作为uuid，不重新生成uuid
        None => "".to_string(), // 没有cookie
    };
    // 创建返回的模板
    let tmp_tpl = if PARAS.english {
        create_main_page_en(&uuid, uri.path().to_string())
    } else {
        create_main_page_ch(&uuid, uri.path().to_string())
    };
    // 如果uuid为空，则重新生成一个uuid作为cookie
    let cookie_jar = if uuid.is_empty() {
        jar.add(create_cookie(Uuid::new_v4().to_string()))
    } else {
        let _ = contain_uuid(&uuid);
        update_cookie_max_age(jar) // 仅修改内部cookie的max-age
    };
    (cookie_jar, tmp_tpl.into())
}
