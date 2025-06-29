use std::collections::HashMap;
use std::env::current_exe;
use std::fs::{write, read_to_string, create_dir_all, remove_dir_all}; // remove_dir只删除空文件夹
use std::path::{Path, PathBuf};
use std::process::exit;

use argh::FromArgs;
use chrono::NaiveDateTime;
use once_cell::sync::Lazy;
use ron::de::from_str;
use serde::Deserialize;
use tiktoken_rs::CoreBPE;
use time::Duration;

/// token: 编码类型，返回CoreBPE对象
/// prompt: 内置的prompt
/// error: 定义的错误类型，用于错误传递
use crate::{
    token::get_tokenizer,
    prompt::create_prompt,
    error::MyError,
};

/// 全局变量，可以修改，存储解析的命令行参数，在解析命令行参数时初始化
pub static PARAS: Lazy<ParsedParas> = Lazy::new(|| {
    match parse_para() {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
            exit(1);
        },
    }
});

#[derive(FromArgs)]
/// http server for LLM api
struct Paras {
    /// config file, contain api_key, endpoint, model name
    #[argh(option, short = 'c')]
    config: Option<String>,

    /// ip address, default: 127.0.0.1
    #[argh(option, short = 'a')]
    addr: Option<String>,

    /// port, default: 8080
    #[argh(option, short = 'p')]
    port: Option<u16>,

    /// search engine key, used for google search
    #[argh(option, short = 'e')]
    engine_key: Option<String>,

    /// search api key, used for google search
    #[argh(option, short = 's')]
    search_key: Option<String>,

    /// graph file, default: search for the latest *.graph file in the output path
    #[argh(option, short = 'g')]
    graph: Option<String>,

    /// cookie max age, default: 1DAY, support: SECOND, MINUTE, HOUR, DAY, WEEK
    #[argh(option, short = 'm')]
    maxage: Option<String>,

    /// allow sharing of all chat logs
    #[argh(switch, short = 'r')]
    share: bool,

    /// chat page show english
    #[argh(switch, short = 'l')]
    english: bool,

    /// output path, default: ./chat-log
    #[argh(option, short = 'o')]
    outpath: Option<String>,
}

/// 存储解析后的命令行参数
///#[derive(Debug, Default)]
pub struct ParsedParas {
    pub api:        Api,                         // 各api的信息
    pub addr:       [u8; 4],                     // 要监听的地址，默认127.0.0.1，解析为[127, 0, 0, 1]
    pub addr_str:   String,                      // 要监听的地址，默认127.0.0.1
    pub port:       u16,                         // 要监听的端口，默认8080
    pub engine_key: String,                      // 搜索引擎的key，去google开启并免费获取，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
    pub search_key: String,                      // 搜索api的key，去google开启并免费获取，每天免费100次搜索，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
    pub bpe:        CoreBPE,                     // tokenizer编码类型，用于计算token数，目前固定为o200k，详见token.rs
    pub prompt:     HashMap<usize, [String; 2]>, // 存储prompt
    pub graph:      String,                      // 图文件，默认在指定输出路径下搜索最新的“时间戳.graph”，指定的文件不存在或没有搜索到则创建空图结构，每次使用`ctrl-c`停止服务时程序会自动保存图文件
    pub maxage:     Duration,                    // cookie过期时间，默认1DAY，支持的单位：SECOND、MINUTE、HOUR、DAY、WEEK
    pub share:      bool,                        // 用户A将自己的uuid-a分享给用户B，用户B将自己的uuid-b与uuid-a建立间接关系（用户B在uuid-b页面左侧“uuid”中输入uuid-a），如果使用该参数，此时用户A可以看到用户B的uuid-b，如果不使用该参数，则用户A看不到用户B的uuid-b，即使用该参数则间接关系是双向的（互相可以看到建立间接关系的uuid-a和uuid-b），不使用该参数则间接关系是单向的（用户B可以看到uuid-a但用户A看不到uuid-b）
    pub english:    bool,                        // 是否展示英文界面，不指定则展示中文界面
    pub outpath:    String,                      // 输出结果路径，不存在则创建，已存在则删除其中的空uuid文件夹，默认./chat-log，不需要加上`/`或`\`后缀（加上了会自动去除），保存chat记录、生成的图片、音频等
}

/// 解析参数
pub fn parse_para() -> Result<ParsedParas, MyError> {
    let para: Paras = argh::from_env();
    let (api, other_para) = Api::new(para.config)?;
    let out: ParsedParas = ParsedParas{
        api: api,
        addr: match &para.addr { // 要监听的地址，默认127.0.0.1，解析为[127, 0, 0, 1]
            Some(a) => get_addr(a)?,
            None => {
                if other_para.ip_address.is_empty() {
                    [127, 0, 0, 1]
                } else {
                    get_addr(&other_para.ip_address)?
                }
            },
        },
        addr_str: match para.addr { // 要监听的地址，默认127.0.0.1
            Some(a) => a,
            None => {
                if other_para.ip_address.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    other_para.ip_address
                }
            },
        },
        port: match para.port { // 要监听的端口，默认8080
            Some(p) => p,
            None => {
                if other_para.port > 0 {
                    other_para.port
                } else {
                    8080
                }
            },
        },
        engine_key: match para.engine_key { // 搜索引擎的key，去google开启并免费获取，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
            Some(e) => e,
            None => other_para.google_engine_key,
        },
        search_key: match para.search_key { // 搜索api的key，去google开启并免费获取，每天免费100次搜索，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
            Some(s) => s,
            None => other_para.google_search_key,
        },
        bpe: get_tokenizer("o200k"), // tokenizer编码类型，用于计算token数，目前固定为o200k，详见token.rs
        prompt: if other_para.prompt.len() == 0 {
            create_prompt() // 参数文件没指定prompt，则使用默认prompt，使用索引获取
        } else {
            other_para.prompt
        },
        graph: match para.graph { // 图文件，默认在指定输出路径下搜索最新的“时间戳.graph”，指定的文件不存在或没有搜索到则创建空图结构，每次使用`ctrl-c`停止服务时程序会自动保存图文件
            Some(g) => g,
            None => "".to_string(),
        },
        maxage: match para.maxage { // cookie过期时间，默认1DAY，支持的单位：SECOND、MINUTE、HOUR、DAY、WEEK，不区分大小写
            Some(m) => get_maxage(&m)?,
            None => {
                if other_para.maxage.is_empty() {
                    get_maxage(&other_para.maxage)?
                } else {
                    Duration::DAY
                }
            },
        },
        share: para.share, // 用户A将自己的uuid-a分享给用户B，用户B将自己的uuid-b与uuid-a建立间接关系（用户B在uuid-b页面左侧“uuid”中输入uuid-a），如果使用该参数，此时用户A可以看到用户B的uuid-b，如果不使用该参数，则用户A看不到用户B的uuid-b，即使用该参数则间接关系是双向的（互相可以看到建立间接关系的uuid-a和uuid-b），不使用该参数则间接关系是单向的（用户B可以看到uuid-a但用户A看不到uuid-b）
        english: if para.english { // 是否展示英文界面，不指定则展示中文界面
            true
        } else {
            if other_para.show_english {
                true
            } else {
                false
            }
        },
        outpath: match para.outpath { // 输出结果路径，不存在则创建，已存在则删除其中的空uuid文件夹，默认./chat-log，不需要加上`/`或`\`后缀（加上了会自动去除），保存chat记录、生成的图片、音频等
            Some(o) => get_outpath(&o),
            None => {
                if other_para.outpath.is_empty() {
                    "./chat-log".to_string()
                } else {
                    get_outpath(&other_para.outpath)
                }
            },
        },
    };
    // 输出路径不存在则创建，已存在则删除其中的空uuid文件夹
    let tmp_outpath = Path::new(&out.outpath);
    if !(tmp_outpath.exists() && tmp_outpath.is_dir()) { // 指定输出路径不存在则创建
        if let Err(err) = create_dir_all(&tmp_outpath) {
            return Err(MyError::CreateDirAllError{dir_name: out.outpath, error: err})
        }
    } else { // 遍历指定输出路径下每个文件夹，如果是空，或仅含有`prompt.txt`一个文件，则删除
        remove_no_log_folder(tmp_outpath, &out.outpath);
    }
    Ok(out)
}

/// 解析开启服务的ip地址
fn get_addr(addr: &str) -> Result<[u8;4], MyError> {
    let tmp_addr_vec: Vec<&str> = addr.split(".").collect();
    if tmp_addr_vec.len() == 4 { // 必须是“.”间隔的4位数，格式为`x.x.x.x`，例如：127.0.0.1
        let mut tmp_addr: [u8; 4] = [0; 4];
        for (i, num) in tmp_addr_vec.iter().enumerate() { // 遍历每位数，转为u8
            match num.parse::<u8>() {
                Ok(n) => tmp_addr[i] = n,
                Err(e) => return Err(MyError::ParseStringError{from: num.to_string(), to: "u8".to_string(), error: e}),
            }
        }
        Ok(tmp_addr)
    } else {
        return Err(MyError::ParaError{para: format!("-a ip address must be x.x.x.x format, not {}", addr)})
    }
}

/// 解析uuid过期时间
fn get_maxage(m: &str) -> Result<Duration, MyError> {
    match m.to_lowercase() {
        s if s.ends_with("second") => {
            let n = s.strip_suffix("second").unwrap();
            if n.is_empty() {
                Ok(Duration::SECOND)
            } else {
                match n.parse::<i64>() {
                    Ok(num) => Ok(Duration::seconds(num)),
                    Err(e) => return Err(MyError::ParaError{para: format!("-m {}: {:?}", m, e)}),
                }
            }
        },
        s if s.ends_with("minute") => {
            let n = s.strip_suffix("minute").unwrap();
            if n.is_empty() {
                Ok(Duration::MINUTE)
            } else {
                match n.parse::<i64>() {
                    Ok(num) => Ok(Duration::minutes(num)),
                    Err(e) => return Err(MyError::ParaError{para: format!("-m {}: {:?}", m, e)}),
                }
            }
        },
        s if s.ends_with("hour") => {
            let n = s.strip_suffix("hour").unwrap();
            if n.is_empty() {
                Ok(Duration::HOUR)
            } else {
                match n.parse::<i64>() {
                    Ok(num) => Ok(Duration::hours(num)),
                    Err(e) => return Err(MyError::ParaError{para: format!("-m {}: {:?}", m, e)}),
                }
            }
        },
        s if s.ends_with("day") => {
            let n = s.strip_suffix("day").unwrap();
            if n.is_empty() {
                Ok(Duration::DAY)
            } else {
                match n.parse::<i64>() {
                    Ok(num) => Ok(Duration::days(num)),
                    Err(e) => return Err(MyError::ParaError{para: format!("-m {}: {:?}", m, e)}),
                }
            }
        },
        s if s.ends_with("week") => {
            let n = s.strip_suffix("week").unwrap();
            if n.is_empty() {
                Ok(Duration::WEEK)
            } else {
                match n.parse::<i64>() {
                    Ok(num) => Ok(Duration::weeks(num)),
                    Err(e) => return Err(MyError::ParaError{para: format!("-m {}: {:?}", m, e)}),
                }
            }
        },
        _ => return Err(MyError::ParaError{para: format!("-m only support SECOND, MINUTE, HOUR, DAY, WEEK, not {}", m)}),
    }
}

/// 去除输出路径的后缀“/”或“\”
fn get_outpath(o: &str) -> String {
    if o.ends_with("/") {
        o.strip_suffix("/").unwrap().to_string()
    } else if o.ends_with("\\") {
        o.strip_suffix("\\").unwrap().to_string()
    } else {
        o.to_string()
    }
}

/// 删除输出路径下不含有chat记录的uuid文件夹
fn remove_no_log_folder(p: &Path, outpath: &str) {
    let mut rm_it: bool;
    match p.read_dir() { // 读取指定输出路径
        Ok(uuid_dirs) => {
            for i in uuid_dirs { // 遍历指定输出路径下每项
                if let Ok(entry) = i {
                    let uuid_path = entry.path();
                    if uuid_path.is_dir() { // 判断是否是文件夹
                        if let Ok(j) = uuid_path.read_dir() { // 读取该文件夹
                            /*
                            if j.next().is_none() { // 如果是空文件夹则删除
                                if let Err(e) = remove_dir(&uuid_path) {
                                    println!("{}", MyError::RemoveDirError{dir: uuid_path.to_str().unwrap().to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                                }
                            }
                            */
                            /*
                            let rm_it = match j.next() {
                                Some(f) => {
                                    if let Ok(ff) = f {
                                        if let Some(ff_str) = ff.path().file_name().unwrap().to_str() {
                                            if ff_str == "prompt.txt" { // 该uuid路径下第一项是`prompt.txt`，说明没有chat记录
                                                true
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                },
                                None => true, // 空文件夹
                            };
                            */
                            rm_it = true;
                            for file in j {
                                if let Ok(ff) = file {
                                    if let Some(ff_str) = ff.path().file_name().unwrap().to_str() {
                                        if file_name_timestamp_txt_html(ff_str) { // 是否是chat记录文件，例如：`2025-01-19_15-21-56.html`或`2025-01-19_15-21-56.log`
                                            rm_it = false;
                                            break
                                        }
                                    }
                                }
                            }
                            if rm_it { // 删除该uuid文件夹
                                if let Err(e) = remove_dir_all(&uuid_path) {
                                    println!("{}", MyError::RemoveDirError{dir: uuid_path.to_str().unwrap().to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
                                }
                            }
                        }
                    }
                }
            }
        },
        Err(e) => {
            println!("{}", MyError::ReadDirError{dir: outpath.to_string(), error: e}); // 这里不要用`{:?}`，会打印结构体而不是打印指定的错误信息
        },
    }
}

/// 检查文件名是否是`年-月-日_时-分-秒`时间戳，且是txt或html格式
/// 例如：`2025-01-19_15-21-56.html`或`2025-01-19_15-21-56.log`
fn file_name_timestamp_txt_html(name: &str) -> bool {
    let time_str: Vec<&str> = name.split('.').collect(); // 获取文件名以“.”分隔的切片
    if time_str.len() == 2 {
        match NaiveDateTime::parse_from_str(&time_str[0], "%Y-%m-%d_%H-%M-%S") {
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Model {
    pub name:        String, // 模型名称，例如："deepseek"
    pub pricing:     String, // 价格，例如："(in: 0.002/k, out: 0.008/k)"
    pub discription: String, // 模型描述信息，例如："全面升级为 DeepSeek-V3"
    pub group:       String, // 模型分组，例如："DeepSeek"，将相同组的模型相邻放置，下拉时会按照组分开
    pub is_default:  bool,   // 是否将该模型作为默认模型
    pub is_cof:      bool,   // 是否是思维链模型
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub provider: String,     // 模型提供者，例如："deepseek"、"cluade"、"openai"、"gemini"、"myself"
    pub api_key:  String,     // api-key，例如："sk-xxx"
    pub endpoint: String,     // html地址，例如："https://api.deepseek.com/v1"
    pub models:   Vec<Model>, // 所有模型
}

#[derive(Debug, Deserialize)]
struct Prompt {
    name:    String, // prompt名称
    content: String, // prompt内容
}

#[derive(Deserialize)]
struct Para {
    ip_address:        String,      // 要监听的地址，默认127.0.0.1
    port:              u16,         // 要监听的端口，默认8080
    google_engine_key: String,      // 搜索引擎的key，去google开启并免费获取，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
    google_search_key: String,      // 搜索api的key，去google开启并免费获取，每天免费100次搜索，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
    maxage:            String,      // cookie过期时间，默认1DAY，支持的单位：SECOND、MINUTE、HOUR、DAY、WEEK
    show_english:      bool,        // true展示英文界面，false展示中文界面
    outpath:           String,      // 问答结果输出路径
    model_config:      Vec<Config>, // 模型参数
    prompts:           Vec<Prompt>, // prompt
}

#[derive(Deserialize)]
struct OptherPara {
    ip_address:        String,                      // 要监听的地址，默认127.0.0.1
    port:              u16,                         // 要监听的端口，默认8080
    google_engine_key: String,                      // 搜索引擎的key，去google开启并免费获取，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
    google_search_key: String,                      // 搜索api的key，去google开启并免费获取，每天免费100次搜索，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
    maxage:            String,                      // cookie过期时间，默认1DAY，支持的单位：SECOND、MINUTE、HOUR、DAY、WEEK
    show_english:      bool,                        // true展示英文界面，false展示中文界面
    outpath:           String,                      // 问答结果输出路径
    prompt:            HashMap<usize, [String; 2]>, // key: 序号，value: [prompt名称, prompt内容]
}

/// 支持的所有api
#[derive(Clone, Debug)]
pub struct Api {
    pub config:          HashMap<String, Config>,                // key: 模型提供者，value: 参数文件中该模型提供者的所有模型
    pub models:          HashMap<usize, (String, String, bool)>, // key: 模型序号，value: (模型提供者, 模型名称, 是否支持深度思考)
    pub default:         usize,                                  // 默认模型的序号，序号与参数文件模型顺序一致
    pub pulldown_prompt: String,                                 // 给html使用的prompt下拉选项字符串，用于创建页面
    pub pulldown_model:  String,                                 // 给html使用的模型下拉选项字符串，用于创建页面
}

/// 实现Api的方法
impl Api {
    /// 初始化模型参数
    fn new(config_file: Option<String>) -> Result<(Self, OptherPara), MyError> {
        // 获取参数文件，先在当前路径下检查是否有config.txt，如果没有，再去程序所在路径下检查是否有config.txt，还没有则在当前路径下生成一个模板config.txt，供用户修改
        let config_file = match config_file {
            Some(c) => PathBuf::from(c),
            None => { // 没有指定参数文件
                let tmp_config = Path::new("config.txt");
                if !(tmp_config.exists() && tmp_config.is_file()) { // 当前路径下没有config.txt
                    let cfg = match current_exe() { // 检查程序所在路径下是否有config.txt
                        Ok(exe_path) => {
                            let cfg_path = exe_path.with_file_name("config.txt"); // 将路径的程序名替换为config.txt
                            if cfg_path.exists() && cfg_path.is_file() {
                                Some(cfg_path)
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    };
                    match cfg {
                        Some(c) => c,
                        None => { // 在当前路径下生成一个示例模板，用户基于此进行修改
                            if let Err(e) = write("config.txt", include_str!("../../config_template.txt")) {
                                return Err(MyError::WriteFileError{file: "./config.txt".to_string(), error: e})
                            }
                            return Err(MyError::ParaError{para: "The config.txt is missing, please revise the generated config.txt".to_string()})
                        },
                    }
                } else { // 当前路径下有config.txt，则使用该参数文件
                    PathBuf::from("config.txt")
                }
            },
        };
        let all_para: Para = match from_str(&read_to_string(config_file)?) {
            Ok(p) => p,
            Err(e) => return Err(MyError::ParaError{para: format!("parse config file error, {:?}", e)}),
        };
        let mut config: HashMap<String, Config> = HashMap::new();
        let mut models: HashMap<usize, (String, String, bool)> = HashMap::new();
        let mut pulldown_model: String = "".to_string(); // 给html使用的模型下拉选项字符串，用于创建页面
        let mut pulldown_model_group = "".to_string(); // 模型的分组
        let mut default: usize = 0; // 默认模型的序号，第1个模型序号是1，不是0，如果参数文件中没有指定默认模型，则将第1个模型作为默认模型
        let mut default_name = "".to_string(); // 默认模型的名称，指定了多个默认模型时打印报错用
        let mut idx: usize = 0; // 模型索引序号
        for c in all_para.model_config {
            if config.contains_key(&c.provider) {
                return Err(MyError::ParaError{para: format!("The config file must not contain duplicate providers: {}", c.provider)})
            }
            config.insert(c.provider.clone(), c.clone());
            for m in c.models {
                idx += 1;
                models.insert(idx, (c.provider.clone(), m.name.clone(), m.is_cof));
                if m.group != pulldown_model_group {
                    pulldown_model += &format!("                <option disabled>---{} {}---</option>\n", c.provider, m.group); // 显示`---模型提供者 分组---`
                    pulldown_model_group = m.group.clone();
                }
                pulldown_model += &format!(
                    "                <option value='{}'{}{}>{}{}</option>\n",
                    idx,
                    if m.is_default {
                        " selected"
                    } else {
                        ""
                    },
                    match m.discription.is_empty() {
                        true => "".to_string(),
                        false => format!(" title='{}'", m.discription),
                    },
                    m.name,
                    match m.pricing.is_empty() {
                        true => "".to_string(),
                        false => format!(" {}", m.pricing),
                    },
                );
                if m.is_default {
                    if default == 0 {
                        default = idx;
                        default_name = m.name.clone();
                    } else {
                        return Err(MyError::ParaError{para: format!("The default model permits only one, however, both {} and {} were detected.", default_name, m.name)})
                    }
                }
            }
        }
        if default == 0 { // 参数文件没有指定默认模型
            default = 1; // 使用参数文件中第一个模型作为默认模型
        }
        // 这里i要加1，即参数文件第一个prompt是1，因为内置“保持当前对话”是-1，“无prompt”是0
        let pulldown_prompt = all_para.prompts.iter().enumerate().fold("".to_string(), |acc, (i, p)| format!("{}                <option value='{}'>{}</option>\n", acc, i+1, p.name));
        let prompt: HashMap<usize, [String; 2]> = all_para.prompts.into_iter().enumerate().map(|(i, p)| (i+1, [p.name, p.content])).collect();
        Ok(
            (
                Api {
                    config,          // key: 模型提供者，value: 参数文件中该模型提供者的所有模型
                    models,          // key: 模型序号，value: (模型提供者, 模型名称, 是否支持深度思考)
                    default,         // 默认模型的序号，序号与参数文件模型顺序一致
                    pulldown_prompt, // 给html使用的prompt下拉选项字符串，用于创建页面
                    pulldown_model,  // 给html使用的模型下拉选项字符串，用于创建页面
                },
                OptherPara {
                    ip_address:        all_para.ip_address,        // 要监听的地址，默认127.0.0.1
                    port:              all_para.port,              // 要监听的端口，默认8080
                    google_engine_key: all_para.google_engine_key, // 搜索引擎的key，去google开启并免费获取，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
                    google_search_key: all_para.google_search_key, // 搜索api的key，去google开启并免费获取，每天免费100次搜索，使用google api进行搜索时要用，可以输入密码使用srx的search engine key
                    outpath:           all_para.outpath,           // 问答结果输出路径
                    maxage:            all_para.maxage,            // cookie过期时间，默认1DAY，支持的单位：SECOND、MINUTE、HOUR、DAY、WEEK
                    show_english:      all_para.show_english,      // true展示英文界面，false展示中文界面
                    prompt,                                        // key: 序号，value: (prompt名称, prompt内容)
                },
            )
        )
    }

    /// 根据模型usize序号获取模型，返回(api_key, endpoint, 模型名称, 是否支持深度思考)
    pub fn get_model_by_usize(&self, num: usize) -> Result<(String, String, String, bool), MyError> {
        match self.models.get(&num) {
            Some(value) => {
                let tmp_config = self.config.get(&value.0).unwrap();
                Ok((tmp_config.api_key.clone(), tmp_config.endpoint.clone(), value.1.clone(), value.2))
            },
            None => Err(MyError::ParaError{para: format!("no such model: {}", num)}),
        }
    }

    /// 根据指定模型字符串序号，获取模型，返回(api_key, endpoint, 模型名称, 是否支持深度思考)
    pub fn get_model_by_str(&self, n: &str) -> Result<(String, String, String, bool), MyError> {
        let num = n.parse::<usize>().map_err(|e| MyError::ParseStringError{from: n.to_string(), to: "usize".to_string(), error: e})?;
        self.get_model_by_usize(num)
    }

    /// 获取默认模型，返回(api_key, endpoint, 模型名称, 是否支持深度思考)
    pub fn get_default_model(&self) -> Result<(String, String, String, bool), MyError> {
        self.get_model_by_usize(self.default)
    }
}
