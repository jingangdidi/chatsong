use std::collections::HashMap;

use tracing::{event, Level};

/// Handler for `/嵌套的前缀/items` GET
/// 解析url中的参数，存储到HashMap
/// 例如访问：http://127.0.0.1:3380/items?cx=912b8adxxxx8e41a9&q=how+to+use+cubecl&num=10&key=AIzaSyAOi2Dxxxxrv0cZKcl0RX8WLs70-vQwiBM
/// 解析得到：{"cx": "912b8adxxxx8e41a9", "q": "how to use cubecl", "num": "10", "key": "AIzaSyAOi2Dxxxxrv0cZKcl0RX8WLs70-vQwiBM"}
pub async fn get_items(axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>, uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "GET {}", uri.path());
    format!("Get items with query params: {:?}", params)
}
