use std::collections::{HashMap, HashSet};
use std::fs::{write, read_to_string, copy, remove_file};
use std::path::Path;
//use std::sync::Mutex;
use std::sync::RwLock;

use chrono::Local;
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use tracing::{event, Level};

use crate::{
    info::{
        get_chat_name,   // 获取指定uuid对话的名称
        get_prompt_name, // 获取当前uuid的prompt名称
        get_latest_file, // 获取指定输出路径下最近的指定格式后缀的文件路径，文件名为时间戳
    },
    parse_paras::PARAS,
    error::MyError,
};

//----------------------------------------------------------------------------------------------------------------
/// 全局变量，可以修改，存储所有uuid的图结构，用于获取指定uuid直接和间接相关的uuid，这里限定不使用pub
/// 指定了uuid的graph图文件，则导入该文件，没有指定uuid的graph图文件则在指定输出路径下搜索最新的图文件（“时间戳.graph”），没有搜索到则初始化空的图结构
/// Mutex:
///     不区分读取还是写入，都需要lock
/// RwLock:
///     read: 可以同时多个读，如果正在被其他线程写，则等待其他线程的操作结束后才返回RwLockReadGuard
///     write: 写时不能有其他读或写，如果正在被其他线程读或写，则等待其他线程的操作结束后才返回RwLockWriteGuard
/// static GRAPH: Lazy<Mutex<Graph>> = Lazy::new(|| Mutex::new(Graph::load_graph(&PARAS.graph, &PARAS.outpath)));
static GRAPH: Lazy<RwLock<Graph>> = Lazy::new(|| RwLock::new(Graph::load_graph(&PARAS.graph, &PARAS.outpath)));

/// 保存当前图结构
pub fn save_graph() {
    //let data = GRAPH.lock().unwrap(); // 使用Mutex，写与读均上锁
    let data = GRAPH.read().unwrap(); // 使用RwLock，保证一写多读，只要不在写，就可以同时多个读取
    if let Err(e) = data.save_graph(&PARAS.outpath) {
        event!(Level::ERROR, "{}", e);
    }
    event!(Level::INFO, "save graph file done");
}

/// 添加新的连接关系，双向的
pub fn add_edge(uuid1: &str, uuid2: &str, is_direct: bool) {
    if !uuid1.is_empty() && !uuid2.is_empty() {
        //let mut data = GRAPH.lock().unwrap(); // 使用Mutex，写与读均上锁
        let mut data = GRAPH.write().unwrap(); // 使用RwLock，保证一写多读，只要不在写，就可以同时多个读取
        data.add_edge(uuid1, uuid2, is_direct);
    }
}

/// 获取与指定uuid相关的所有uuid，返回Vec<(相关的uuid, uuid对应的prompt和对话名称)>
pub fn get_all_related_uuid(uuid: &str) -> Vec<(String, String)> {
    //let data = GRAPH.lock().unwrap(); // 使用Mutex，写与读均上锁
    let data = GRAPH.read().unwrap(); // 使用RwLock，保证一写多读，只要不在写，就可以同时多个读取
    data.get_all_related_uuid(uuid)
}

/// 如果指定文件不在指定uuid的路径下，则去该uuid所有相关uuid路径下寻找，复制到指定uuid路径下
pub fn copy_file_from_related_uuid(uuid: &str, name: &str) {
    let tmp_target_file = format!("{}/{}/{}", PARAS.outpath, uuid, name);
    let tmp_path = Path::new(&tmp_target_file);
    if !(tmp_path.exists() && tmp_path.is_file()) { // 先判断指定的文件是否在指定的uuid路径下，若不在，则从相关的其他uuid路径下寻找
        for (u, _) in get_all_related_uuid(uuid) {
            if u != uuid {
                let tmp_file = format!("{}/{}/", PARAS.outpath, u);
                let tmp_path = Path::new(&tmp_file);
                if tmp_path.exists() && tmp_path.is_dir() {
                    if let Ok(uuid_dirs) = tmp_path.read_dir() { // 读取当前uuid路径
                        for i in uuid_dirs { // 遍历当前uuid路径下每项
                            if let Ok(entry) = i {
                                let uuid_path = entry.path();
                                if uuid_path.is_file() { // 判断是否是文件
                                    if uuid_path.file_name().unwrap().to_str().unwrap() == name {
                                        // 将找到文件复制到指定uuid的路径下
                                        if let Err(e) = copy(&uuid_path, &tmp_target_file) {
                                            event!(Level::ERROR, "copy {} from {} to {} error: {}", name, u, uuid, e);
                                        }
                                        return
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 删除指定uuid
pub fn graph_remove_uuid(uuid: &str) {
    let mut data = GRAPH.write().unwrap(); // 使用RwLock，保证一写多读，只要不在写，就可以同时多个读取
    data.graph_remove_uuid(uuid)
}

//----------------------------------------------------------------------------------------------------------------
/// 每个uuid直接和间接相关的uuid向量
#[derive(Serialize, Deserialize)]
struct RelatedNodes {
    direct:   HashMap<String, (i64, String)>, // 每个uuid直接相关的uuid，value是用于排序的时间戳和prompt，存储在指定uuid页面创建新对话生成的uuid，这些uuid肯定与指定uuid是同一用户的，因此递归获取所有直接相关的uuid
    indirect: HashMap<String, (i64, String)>, // 每个uuid间接相关的uuid，value是用于排序的时间戳和prompt，存储由指定uuid页面跳转页面时输入的uuid，由于这些uuid可能是其他用户分享的，因此不会去递归获取这些uuid相关的uuid，保证数据安全
}

/// uuid图结构
#[derive(Serialize, Deserialize)]
struct Graph {
    related: HashMap<String, (RelatedNodes, i64)>, // 每个uuid直接或间接相关的uuid，key: uuid，value: (与key的uuid直接或间接相关的uuid, 每个uuid用于排序的时间戳)
}

impl Graph {
    /// 初始化uuid图结构
    fn new() -> Self {
        Graph {related: HashMap::new()}
    }

    /// 添加新的连接关系，单向的
    fn uni_directional(&mut self, src_node: &str, tgt_node: &str, tgt_time: i64, is_direct: bool) {
        // add_edge在调用该方法前已经将不存在的node插入了，因此这里不会是None
        if let Some(rn) = self.related.get_mut(src_node) {
            if is_direct {
                if rn.0.indirect.contains_key(tgt_node) { // 如果node已在间接关系中，则从间接关系中删除该node，并添加到直接关系中
                    rn.0.indirect.remove(tgt_node);
                }
                rn.0.direct.insert(tgt_node.to_string(), (tgt_time, get_prompt_name(tgt_node)));
            } else {
                if !rn.0.direct.contains_key(tgt_node) { // 如果node已在直接关系中，则不向间接关系中添加
                    rn.0.indirect.insert(tgt_node.to_string(), (tgt_time, get_prompt_name(tgt_node)));
                }
            }
        }
    }

    /// 添加新的连接关系，直接关系是双向的，间接关系是单向的
    fn add_edge(&mut self, node1: &str, node2: &str, is_direct: bool) {
        // 获取node1的时间戳，该node不在图中则生成当前时间戳
        let node1_time = match self.related.get(node1) {
            Some(t) => t.1,
            None => {
                let t = Local::now().timestamp();
                self.related.insert(node1.to_string(), (RelatedNodes{direct: HashMap::new(), indirect: HashMap::new()}, t));
                t
            },
        };
        // 获取node2的时间戳，该node不在图中则生成当前时间戳
        let node2_time = match self.related.get(node2) {
            Some(t) => t.1,
            None => {
                let t = Local::now().timestamp()+1; // 这里将node2的时间戳加1，防止node1和node2都是新生成时间戳时，与node1相同，保证node1排序后在node2之前
                self.related.insert(node2.to_string(), (RelatedNodes{direct: HashMap::new(), indirect: HashMap::new()}, t));
                t
            },
        };
        // 更新node1 --> node2
        self.uni_directional(node1, node2, node2_time, is_direct);
        // 直接关系是双向的，间接关系是单向的
        if is_direct || PARAS.share { // 如果是直接关系，或间接关系但允许间接关系反向连接（node2关联到node1），此时需要建立node2到node1的关系
            // 更新node2 --> node1
            self.uni_directional(node2, node1, node1_time, is_direct);
        }
    }

    /// 使用`Depth-First Search (DFS)`算法递归获取指定uuid相关的所有uuid
    fn dfs(&self, node: &str, prompt: &str, visited: &mut HashSet<String>, related: &mut Vec<(String, i64, String)>) {
        if visited.contains(node) {
            return;
        }
        visited.insert(node.to_string());
        let time = match self.related.get(node) {
            Some(t) => t.1,
            None => return, // 图中没有该节点，直接返回
        };
        related.push((node.to_string(), time, prompt.to_string()));
        if let Some(neighbors) = self.related.get(node) {
            for (neighbor, (_, prt)) in &neighbors.0.direct { // 这里只对直接相关的node进行递归搜索
                self.dfs(&neighbor, &prt, visited, related);
            }
        }
    }

    /// 获取与指定uuid相关的所有uuid，返回Vec<(相关的uuid, uuid对应的prompt和对话名称)>
    fn get_all_related_uuid(&self, start_node: &str) -> Vec<(String, String)> {
        let mut related = Vec::new(); // (uuid, 时间, prompt)
        // 如果图中含有该node且该node与其他node有关联，则获取关联的node
        if self.related.contains_key(start_node) && self.related.get(start_node).unwrap().0.direct.len() + self.related.get(start_node).unwrap().0.indirect.len() > 0 {
            let mut visited = HashSet::new();
            // 先递归获取所有直接相关的uuid
            let prompt = get_prompt_name(start_node);
            self.dfs(start_node, &prompt, &mut visited, &mut related);
            // 再把该uuid间接相关的uuid加上
            if let Some(neighbors) = self.related.get(start_node) {
                for (neighbor, (time, prt)) in &neighbors.0.indirect { // 这里遍历间接相关的node，添加到输出向量中
                    if !visited.contains(neighbor) {
                        related.push((neighbor.to_string(), *time, prt.to_string()))
                    }
                }
            }
        }
        // 根据时间戳进行排序
        related.sort_by(|a, b| a.2.cmp(&b.2)); // 根据节点的时间戳由小到大排序
        related.into_iter().map(|u| {
            let tmp_chat_name = get_chat_name(&u.0);
            if tmp_chat_name.is_empty() {
                (u.0, u.2)
            } else {
                (u.0, format!("{}---{}", u.2, tmp_chat_name))
            }
        }).collect() // 舍弃时间戳，只返回Vec<(相关的uuid, uuid对应的prompt和对话名称)>
    }

    /// 保存当前图结构
    fn save_graph(&self, outpath: &str) -> Result<(), MyError> {
        // 图结构转json字符串
        let graph_json_str = serde_json::to_string(&self).map_err(|e| MyError::ToJsonStirngError{uuid: "save graph".to_string(), error: e})?;
        // 保存图结构的json字符串
        let graph_file = format!("{}/{}.graph", outpath, Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
        write(&graph_file, graph_json_str).map_err(|e| MyError::WriteFileError{file: graph_file, error: e})?;
        keep_latest_gragh(5)
    }

    /// 导入图结构
    fn load_graph(graph_file: &str, outpath: &str) -> Graph {
        if graph_file.is_empty() { // 没有指定uuid的graph图文件则在指定输出路径下搜索最新的图文件（“时间戳.graph”），没有搜索到则初始化空的图结构
            let latest_file = get_latest_file(outpath.to_string(), ".graph");
            if latest_file.is_empty() { // 没有搜索到则初始化空的图结构
                Graph::new()
            } else {
                load_graph_file(&latest_file)
            }
        } else { // 指定了uuid的graph图文件，则导入该文件
            load_graph_file(graph_file)
        }
    }

    /// 遍历当前图结构中每个uuid，如果对应文件夹不存在（每次启动服务会删除指定输出路径下不含有chat记录的uuid文件夹），则将该uuid从图中删除（包括直接和间接关系中的节点）
    /// 由于图中所有节点肯定都是related的key，因此第一次遍历先检查related的key，记录要删除的uuid
    /// 然后遍历每个节点，删除内部direct和indirect中无效节点
    /// 最后删除related的无效节点
    fn remove_invalid_node(&mut self) {
        // 第一次遍历先检查related的key，记录要删除的uuid
        let mut invalid: HashSet<String> = HashSet::new();
        let mut all_keys = vec![];
        for (k, _) in &self.related {
            let tmp_uuid_path = format!("{}/{}", PARAS.outpath, k);
            let tmp_path = Path::new(&tmp_uuid_path);
            if !(tmp_path.exists() && tmp_path.is_dir()) {
                invalid.insert(k.to_string());
            }
            all_keys.push(k.to_string());
        }
        // 遍历每个节点，删除内部direct和indirect中无效节点
        for k in &all_keys {
            // 删除direct的node在invalid中的node
            for i in &invalid {
                self.related.get_mut(k).unwrap().0.direct.remove(i);
            }
            // 删除indirect的node在invalid中的node
            for i in &invalid {
                self.related.get_mut(k).unwrap().0.indirect.remove(i);
            }
        }
        // 最后再删除related的无效节点
        for k in &invalid {
            self.related.remove(k);
        }
    }

    /// 删除指定uuid
    pub fn graph_remove_uuid(&mut self, uuid: &str) {
        // 删除指定的uuid节点
        self.related.remove(uuid);
        // 遍历每个节点，获取内部direct和indirect含有指定uuid节点
        let mut remove_direct: HashSet<String> = HashSet::new();
        let mut remove_indirect: HashSet<String> = HashSet::new();
        for (k, v) in &self.related {
            if v.0.direct.contains_key(uuid) {
                remove_direct.insert(k.clone());
            } else if v.0.indirect.contains_key(uuid) {
                remove_indirect.insert(k.clone());
            }
        }
        // 最后删除direct和indirect中指定uuid节点
        for k in &remove_direct {
            self.related.get_mut(k).unwrap().0.direct.remove(uuid);
        }
        for k in &remove_indirect {
            self.related.get_mut(k).unwrap().0.indirect.remove(uuid);
        }
    }
}

/// 导入图结构
fn load_graph_file(graph_file: &str) -> Graph {
    let tmp_path = Path::new(graph_file);
    if tmp_path.exists() && tmp_path.is_file() {
        match read_to_string(graph_file) {
            Ok(s) => {
                match serde_json::from_str::<Graph>(&s) {
                    Ok(mut g) => {
                        event!(Level::INFO, "load graph file success: {}", graph_file);
                        // 检查uuid对应的路径是否都存在，不存在则从图中删掉
                        g.remove_invalid_node();
                        g
                    },
                    Err(e) => {
                        event!(Level::INFO, "warning: uuid graph string to json error: {:?}, init new Graph", e);
                        Graph::new()
                    },
                }
            },
            Err(e) => {
                event!(Level::INFO, "warning: read {} to string error: {:?}, init new Graph", graph_file, e);
                Graph::new()
            },
        }
    } else {
        event!(Level::INFO, "warning: no such file: {}, init new Graph", graph_file);
        Graph::new()
    }
}

/// 获取输出路径下所有gragh文件，保留最新的num个，删除其余的
fn keep_latest_gragh(num: usize) -> Result<(), MyError> {
    let outpath = Path::new(&PARAS.outpath);
    let dir = outpath.read_dir().map_err(|e| MyError::ReadDirError{dir: PARAS.outpath.clone(), error: e})?;
    let mut graph_files: Vec<String> = vec![]; // 存储gragh文件，格式“时间戳.gragh”，例如：“2025-04-08_09-34-11.graph”
    // 遍历输出路径下所有项
    for file in dir {
        if file.is_err() {
            continue;
        }
        let file = file.unwrap();
        let metadata = file.metadata();
        if metadata.is_err() {
            continue;
        }
        let metadata = metadata.unwrap();
        // 是文件，且格式后缀是“.graph”
        if metadata.is_file() {
            if let Some(s) = file.file_name().to_str() {
                if s.ends_with(".graph") {
                    graph_files.push(s.to_string());
                }
            }
        }
    }
    if graph_files.len() > num {
        graph_files.sort(); // 排序
        for i in graph_files.iter().rev().skip(num) {
            //println!("{}", i);
            let tmp_file = format!("{}/{}", PARAS.outpath, i);
            remove_file(&tmp_file).map_err(|e| MyError::RemoveFileError{file: tmp_file, error: e})?
        }
    }
    Ok(())
}
