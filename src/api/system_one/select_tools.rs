use super::select::{Criteria, Item, evaluate_matched};
use crate::{
    error::MyError,
    parse_paras::PARAS,
};

/// 内置工具 + MCP 工具
fn all_tools() -> Vec<Item> {
    let mut tools: Vec<Item> = PARAS.tools.get_all_tools_info().into_iter().map(|t| Item::new(t.0, t.1, t.2)).collect();
    let mut mcp: Vec<Item> = PARAS.mcp_servers.get_all_tools_info().into_iter().map(|t| Item::new(t.0, t.1, t.2)).collect();
    tools.append(&mut mcp);
    tools
}

/// 获取与 `question` 相关的工具，返回 `Vec<工具id, 工具名, 概率>`
pub async fn get_relevant_tools(url: &str, question: &str) -> Result<Vec<(String, String, f64)>, MyError> {
    let items: Vec<Item> = all_tools();
    let criteria = Criteria::new(
        "description描述的工具功能，是否与question相关？".to_string(),
        &[
            ("relevant", "description描述的工具功能，与question相关，需要调用该工具"),
            ("possibly_relevant", "description描述的工具功能，可能会被用到"),
            ("not_relevant", "description描述的工具功能，与question无关，不会被用到"),
        ],
        &["relevant"],
    );
    let picked = evaluate_matched(url, question, &criteria, &items).await?;
    // sub_agent 由调用方固定提供，不参与选择
    Ok(picked.into_iter().filter(|p| p.1 != "sub_agent").map(|p| p).collect())
}
