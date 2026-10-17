use tracing::{event, Level};

use super::select::{Criteria, Item, evaluate_matched};
use crate::{
    error::MyError,
    memory::{
        MEMORY,
        final_memory_prompt,
    },
};

/// 获取所有记忆
fn all_memory() -> Result<Vec<Item>, MyError> {
    let data = MEMORY.lock().unwrap();
    match data.get("local") {
        Some(memory) => Ok(memory.get_all_memory_vec().into_iter().map(|m| Item::new(m.clone(), String::new(), m)).collect::<Vec<Item>>()),
        None => Err(MyError::OtherError{info: "no local memory for system one".to_string()}),
    }
}

/// 获取与 `question` 相关的记忆，返回相关记忆
pub async fn get_relevant_memory(url: &str, question: &str) -> Result<Option<String>, MyError> {
    let items: Vec<Item> = all_memory()?;
    let criteria = Criteria::new(
        "description描述的记忆，是否与question相关？".to_string(),
        &[
            ("relevant", "description描述的记忆，与question相关，需要用到该记忆"),
            ("possibly_relevant", "description描述的记忆，可能会被用到"),
            ("not_relevant", "description描述的记忆，与question无关，不会被用到"),
        ],
        &["relevant"],
    );
    let picked = evaluate_matched(url, question, &criteria, &items).await?;
    if picked.is_empty() {
        event!(Level::INFO, "no relevant memory by system one");
        Ok(None)
    } else {
        event!(Level::INFO, "got memory:\n{}", picked.iter().map(|m| format!("- relevant probability: {:.4}, memory: {}", m.2, m.0)).collect::<Vec<_>>().join("\n"));
        let memory = picked.into_iter().map(|m| m.0).collect();
        Ok(Some(final_memory_prompt(memory)))
    }
}
