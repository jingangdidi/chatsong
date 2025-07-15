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
        check_incognito, // 如果设置了无痕，则把当前uuid的问答信息都清空，返回true
        get_latest_log_file, // 获取指定输出路径下最近的chat记录文件路径，例如：`2024-04-04_12-49-50.log`
    },
    //html_page::{create_main_page_ch, create_main_page_en},
    html_page::create_main_page,
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
    // 如果uuid为空，则重新生成一个uuid作为cookie
    let cookie_jar = if uuid.is_empty() {
        jar.add(create_cookie(Uuid::new_v4().to_string()))
    } else {
        // 如果设置了无痕，且uuid路径下没有log文件（有则说明是基于之前对话记录接着提问，但指定了无痕，此时不需要把该uuid对应的Info和graph移除，只是最后不保存新问答内容，这样有一点不同，就是刷新页面该uuid内容都还存在，相关uuid下拉选项中也能跳转到该uuid），则把当前uuid的问答信息都清空，返回true
        if check_incognito(&uuid) && get_latest_log_file(&uuid).is_empty() {
            jar.add(create_cookie(uuid.clone())) // 还用这个uuid，无痕模式的对话已被丢弃
        } else {
            let _ = contain_uuid(&uuid);
            update_cookie_max_age(jar) // 仅修改内部cookie的max-age
        }
    };
    // 创建返回的模板
    let tmp_tpl = create_main_page(&uuid, uri.path().to_string());
    (cookie_jar, tmp_tpl.into())
}
