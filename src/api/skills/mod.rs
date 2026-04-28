use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::MyError;

/// skills
pub struct Skills {
    pub skill_manager: SkillManager,
    pub available:     Vec<SkillAvailability>, // all available skills
    pub unavailable:   Vec<SkillAvailability>, // all unavailable skills
    pub html:          String,                 // html dropdown string
}

impl Skills {
    /// get skill full content
    pub fn get_skill_full_content(&self, skill_name: &str) -> Result<String, MyError> {
        for skill in &self.available {
            if skill.meta.name == skill_name {
                let mut result = format!("# Skill: {}\n\n", skill.meta.name);
                result.push_str(&format!("Description: {}\n", skill.meta.description));
                result.push_str(&format!("Skill directory: {}\n", skill.meta.dir_path.display()));
                result.push_str(&format!("Source: {}\n", skill.meta.source));
                if let Some(version) = &skill.meta.version {
                    result.push_str(&format!("Version: {}\n", version));
                }
                if let Some(updated_at) = &skill.meta.updated_at {
                    result.push_str(&format!("Updated at: {}\n", updated_at));
                }
                if !skill.meta.platforms.is_empty() {
                    result.push_str(&format!("Platforms: {}\n", skill.meta.platforms.join(", ")));
                }
                if !skill.meta.deps.is_empty() {
                    result.push_str(&format!("Dependencies: {}\n", skill.meta.deps.join(", ")));
                }
                result.push_str("\n## Instructions\n\n");
                result.push_str(&skill.body);
                return Ok(result)
            }
        }
        Err(MyError::OtherError{info: format!("no such skill: `{}`", skill_name)})
    }

    /// 获取可用skills中指定name的skill的name和description，用于prompt
    pub fn get_single_available_skill_prompt(&self, index: usize) -> String {
        if self.available.len() > index {
            format!("<available_skills>\n- {}: {}\n</available_skills>", &self.available[index].meta.name, &self.available[index].meta.description)
        } else {
            "<available_skills>\n</available_skills>".to_string()
        }
    }

    /// 获取所有可用skills的name和description，用于prompt
    pub fn get_all_available_skills_prompt(&self) -> String {
        let mut prompt = String::from("<available_skills>\n");
        for skill in &self.available {
            //prompt.push_str(&format!("- {}\n", skill.meta.name)); // 节省token可省略description
            prompt.push_str(&format!("- {}: {}\n", skill.meta.name, skill.meta.description));
        }
        prompt.push_str("</available_skills>");
        prompt
    }

    pub fn get_group_available_skills_prompt(&self, group: String) -> String {
        let mut prompt = String::from("<available_skills>\n");
        let group_string = format!("skill_group_{}", group);
        for skill in &self.available {
            if skill.group == group_string {
                //prompt.push_str(&format!("- {}\n", skill.meta.name)); // 节省token可省略description
                prompt.push_str(&format!("- {}: {}\n", skill.meta.name, skill.meta.description));
            }
        }
        prompt.push_str("</available_skills>");
        prompt
    }
}

/// 下拉选择的skills
pub enum SelectedSkills {
    All,
    Group(String),
    Single(usize),
}

/// 解析的SKILL.md，不包含body部分
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub name:        String,
    pub description: String,
    pub dir_path:    PathBuf,
    pub platforms:   Vec<String>,
    pub deps:        Vec<String>,
    pub source:      String,
    pub version:     Option<String>,
    pub updated_at:  Option<String>,
    pub env_file:    Option<String>,
}

/// skills是否可用，以及不可用的原因
#[derive(Debug, Clone)]
pub struct SkillAvailability {
    pub meta:      SkillMetadata,
    pub body:      String,
    pub available: bool,
    pub reason:    Option<String>,
    pub group:     String,
}

/// 依赖
#[derive(Debug, Deserialize, Default)]
struct SkillCompatibility {
    #[serde(default)]
    os: Vec<String>, // 支持的操作系统
    #[serde(default)]
    deps: Vec<String>, // 需要的依赖软件
}

/// 解析SKILL.md
#[derive(Debug, Deserialize, Default)]
struct SkillFrontmatter {
    name: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    platforms: Vec<String>,
    #[serde(default)]
    deps: Vec<String>,
    #[serde(default)]
    compatibility: SkillCompatibility,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    env_file: Option<String>,
}

/// skills路径，该路径下包含所有skills
pub struct SkillManager {
    skills_dir: PathBuf,
}

impl SkillManager {
    /// 根据指定skills路径创建
    pub fn from_skills_dir(skills_dir: PathBuf) -> Self {
        SkillManager{skills_dir}
    }

    /// 检查当前操作系统是否满足指定skill，以及依赖的软件是否都满足
    fn skill_is_available(&self, skill: &SkillMetadata) -> Result<(), MyError> {
        // 检查操作系统是否支持
        if !platform_allowed(&skill.platforms) {
            return Err(MyError::OtherError{info: format!(
                "Skill '{}' is not available on this platform (current: {}, supported: {}).",
                skill.name,
                current_platform(),
                skill.platforms.join(", ")
            )})
        }

        // 检查依赖的程序是否满足
        let missing = missing_deps(&skill.deps);
        if !missing.is_empty() {
            return Err(MyError::OtherError{info: format!(
                "Skill '{}' is missing required dependencies: {}",
                skill.name,
                missing.join(", ")
            )})
        }

        Ok(())
    }

    /// 检查当前每个skill是否可用，以及不可用的原因
    /*
    fn discover_skill_statuses(&self) -> Vec<SkillAvailability> {
        let mut statuses = Vec::new();
        let entries = match std::fs::read_dir(&self.skills_dir) {
            Ok(e) => e,
            Err(_) => return statuses,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue
            }
            if let Ok(content) = std::fs::read_to_string(&skill_md) {
                if let Some((meta, body)) = parse_skill_md(&content, &path) {
                    match self.skill_is_available(&meta) {
                        Ok(()) => statuses.push(SkillAvailability {
                            meta,
                            body,
                            available: true,
                            reason: None,
                        }),
                        Err(MyError::OtherError{info: reason}) => statuses.push(SkillAvailability {
                            meta,
                            body,
                            available: false,
                            reason: Some(reason),
                        }),
                        _ => unreachable!(),
                    };
                }
            }
        }
        //statuses.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
        statuses.sort_by_key(|s| s.meta.name.to_ascii_lowercase());
        statuses
    }
    */

    /// 检查当前每个skill是否可用，以及不可用的原因
    /// 如果`skills_dir`路径下一级文件夹含有`SKILL.md`，则归为`Other`分组
    /// 如果`skills_dir`路径下一级文件夹不含有`SKILL.md`，继续判断该一级文件夹下的文件夹（二级文件夹）中是否含有`SKILL.md`，含有则该skill的分组设为一级文件夹的名称
    /// 这样其实就是把skill做链分组，方便直接按照分组选择
    fn discover_skill_statuses(&self) -> Vec<SkillAvailability> {
        let mut statuses = Vec::new();
        let entries = match std::fs::read_dir(&self.skills_dir) {
            Ok(e) => e,
            Err(_) => return statuses,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue
            }
            let (skill_mds, path_vec, group) = {
                let md_path = path.join("SKILL.md");
                if md_path.exists() {
                    (vec![md_path], vec![path], "Other-skills".to_string())
                } else { // 该路径下没有SKILL.md，则从该路径中寻找含有SKILL.md的文件夹，即该路径内包含链一组skills
                    let group = path.file_name().unwrap().to_str().unwrap().to_string();
                    let entries2 = match std::fs::read_dir(&path) {
                        Ok(e) => e,
                        Err(_) => return statuses,
                    };
                    let mut mds = Vec::new();
                    let mut path2_vec = Vec::new();
                    for entry2 in entries2.flatten() {
                        let path2 = entry2.path();
                        if !path2.is_dir() {
                            continue
                        }
                        let skill_md = path2.join("SKILL.md");
                        if !skill_md.exists() {
                            continue
                        }
                        mds.push(skill_md);
                        path2_vec.push(path2);
                    }
                    (mds, path2_vec, group)
                }
            };
            for (md, path) in skill_mds.into_iter().zip(path_vec) {
                if let Ok(content) = std::fs::read_to_string(&md) {
                    if let Some((meta, body)) = parse_skill_md(&content, &path) {
                        match self.skill_is_available(&meta) {
                            Ok(()) => statuses.push(SkillAvailability {
                                meta,
                                body,
                                available: true,
                                reason: None,
                                group: group.clone(),
                            }),
                            Err(MyError::OtherError{info: reason}) => statuses.push(SkillAvailability {
                                meta,
                                body,
                                available: false,
                                reason: Some(reason),
                                group: group.clone(),
                            }),
                            _ => unreachable!(),
                        };
                    }
                }
            }
        }
        // 先根据group排序，再根据meta.name排序
        statuses.sort_by(|a, b| {
            // 先比较 group
            match a.group.cmp(&b.group) {
                std::cmp::Ordering::Equal => {
                    // 如果 group 相同，再比较 meta.name
                    a.meta.name.to_ascii_lowercase().cmp(&b.meta.name.to_ascii_lowercase())
                }
                other => other,
            }
        });
        statuses
    }

    /// 获取所有可用和不可用的skills
    pub fn discover_skills(&self) -> (Vec<SkillAvailability>, Vec<SkillAvailability>) {
        let statuses = self.discover_skill_statuses();
        statuses.into_iter().partition(|s| s.available)
    }

    /// 将所有可用skills整理为字符串，用于插入到prompt中
    pub fn build_skills_catalog(&self, skills: &Vec<SkillAvailability>) -> String {
        if skills.is_empty() {
            return String::new()
        }

        // 整理最终skills字符串
        let mut catalog = String::from("<available_skills>\n");
        for skill in skills {
            //catalog.push_str(&format!("- {}\n", skill.meta.name)); // 节省token可省略description
            catalog.push_str(&format!("- {}: {}\n", skill.meta.name, skill.meta.description));
        }
        catalog.push_str("</available_skills>");
        catalog
    }

    /// html页面下拉选项
    pub fn html_dropdown(&self, available: &Vec<SkillAvailability>, unavailable: &Vec<SkillAvailability>, english: bool) -> String {
        let mut options: Vec<String> = Vec::with_capacity(3 + available.len() + unavailable.len());
        let mut group = "".to_string();
        // available
        if !available.is_empty() {
            if english {
                options.push("<option value='not_select_any_skills' selected>⚪ not using any skills</option>".to_string());
                options.push("                <option value='select_all_available_skills'>🔴 select all available skills</option>".to_string());
                options.push("                <optgroup label='available skills'>".to_string());
            } else {
                options.push("<option value='not_select_any_skills' selected>⚪ 不使用任何skill</option>".to_string());
                options.push("                <option value='select_all_available_skills'>🔴 选择所有可用skills</option>".to_string());
                options.push("                <optgroup label='可用skills'>".to_string());
            }
            if available.iter().all(|s| s.group == "Other-skills") { // 没有分组
                for (i, skill) in available.iter().enumerate() {
                    options.push(format!("                    <option value='available-skill-{}' title=\"{}\">{}</option>", i, skill.meta.description.replace("\"", "&quot;"), skill.meta.name.replace("\"", "&quot;")));
                }
            } else { // 有分组
                for (i, skill) in available.iter().enumerate() {
                    if group != skill.group {
                        group = skill.group.clone();
                        options.push(format!("                    <option disabled>--{}--</option>", group));
                        if english {
                            options.push(format!("                    <option value='skill_group_{}'>🟢 select all {}</option>", group, group));
                        } else {
                            options.push(format!("                    <option value='skill_group_{}'>🟢 选择所有{}</option>", group, group));
                        }
                    }
                    options.push(format!("                    <option value='available-skill-{}' title=\"{}\">{}</option>", i, skill.meta.description.replace("\"", "&quot;"), skill.meta.name.replace("\"", "&quot;")));
                }
            }
            options.push("                </optgroup>".to_string());
        }
        // unavailable
        if !unavailable.is_empty() {
            if english {
                options.push("                <optgroup label='unavailable skills'>".to_string());
            } else {
                options.push("                <optgroup label='不可用skills'>".to_string());
            }
            for (i, skill) in unavailable.iter().enumerate() {
                options.push(format!("                    <option disabled value='unavailable-skill-{}' title=\"{}\">{}</option>", i, skill.meta.description.replace("\"", "&quot;"), skill.meta.name.replace("\"", "&quot;")));
            }
            options.push("                </optgroup>".to_string());
        }
        options.join("\n")
    }

    /// 返回方便用户阅读的所有可用的skills字符串
    pub fn list_skills_formatted(&self, skills: &Vec<SkillAvailability>) -> String {
        if skills.is_empty() {
            return "No skills available".to_string()
        }
        let mut output = format!("Available skills ({}):\n\n", skills.len());
        for skill in skills {
            output.push_str(&format!(
                "• {} — {} [{}]\n",
                skill.meta.name, skill.meta.description, skill.meta.source
            ));
        }
        output
    }

    /// 返回方便用户阅读的所有skills字符串（包括不可用的skills）
    pub fn list_skills_formatted_all(&self, available: &Vec<SkillAvailability>, unavailable: &Vec<SkillAvailability>) -> String {
        if available.is_empty() && unavailable.is_empty() {
            return "No skills found in skills directory.".to_string()
        }
        let mut output = String::new();
        output.push_str(&format!("Available skills ({}):\n\n", available.len()));
        for skill in available {
            output.push_str(&format!(
                "• {} — {} [{}]\n",
                skill.meta.name, skill.meta.description, skill.meta.source
            ));
        }
        output.push('\n');
        output.push_str(&format!("Unavailable skills ({}):\n\n", unavailable.len()));
        for skill in unavailable {
            output.push_str(&format!(
                "• {} — {}\n",
                skill.meta.name,
                skill
                    .reason
                    .as_deref()
                    .unwrap_or("unavailable for unknown reason")
            ));
        }
        output
    }
}

/// 获取当前操作系统
fn current_platform() -> String {
    if cfg!(target_os = "macos") {
        "darwin".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "unknown".to_string()
    }
}

/// 转小写，macos和osx都转为darwin
fn normalize_platform(value: &str) -> String {
    let v = value.trim().to_ascii_lowercase();
    match v.as_str() {
        "macos" | "osx" => "darwin".to_string(),
        _ => v,
    }
}

/// 检查当前操作系统是否支持skill所需的操作系统
fn platform_allowed(platforms: &[String]) -> bool {
    if platforms.is_empty() {
        true
    } else {
        let current = current_platform(); // 获取当前操作系统
        platforms.iter().any(|p| {
            let p = normalize_platform(p);
            p == "all" || p == "*" || p == current
        })
    }
}

/// 检查指定程序是否存在
pub fn command_exists(command: &str) -> bool {
    if command.trim().is_empty() {
        return true
    }

    // 从`PATH`环境变量获取可用的程序
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::env::split_paths(&path_var);

    #[cfg(target_os = "windows")]
    let candidates: Vec<String> = {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        let ext_list: Vec<String> = exts
            .split(';')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let lower = command.to_ascii_lowercase();
        if ext_list.iter().any(|ext| lower.ends_with(ext)) {
            vec![command.to_string()]
        } else {
            let mut c = vec![command.to_string()];
            for ext in ext_list {
                c.push(format!("{command}{ext}"));
            }
            c
        }
    };

    #[cfg(not(target_os = "windows"))]
    let candidates: Vec<String> = vec![command.to_string()];

    for base in paths {
        for candidate in &candidates {
            let full = base.join(candidate);
            if let (true, Ok(meta)) = (full.is_file(), full.metadata()) {
                if meta.len() > 0 { // 除了检查文件是否存在，还要保证文件不为空
                    return true
                }
            }
        }
    }

    false
}

/// 获取缺失的程序依赖
fn missing_deps(deps: &[String]) -> Vec<String> {
    deps.iter()
        .filter(|dep| !command_exists(dep))
        .cloned()
        .collect()
}

/// 将单行skill（`--- name: x description: y --- body`）转为标准的多行YAML格式
fn normalize_single_line_frontmatter(content: &str) -> Option<String> {
    if !content.starts_with("--- ") {
        return None
    }
    let after_open = &content[4..]; // skip "--- "
    let close_idx = after_open.find(" ---")?;
    let yaml_part = after_open[..close_idx].trim();
    if yaml_part.is_empty() {
        return None
    }
    let body = after_open[close_idx + 4..].trim_start();

    // Insert newlines before known frontmatter keys so serde_yaml can parse them
    let known_keys: &[&str] = &[
        "name:",
        "description:",
        "license:",
        "platforms:",
        "deps:",
        "compatibility:",
        "source:",
        "version:",
        "updated_at:",
    ];
    let mut yaml = yaml_part.to_string();
    for key in known_keys {
        yaml = yaml.replacen(&format!(" {key}"), &format!("\n{key}"), 1);
    }

    Some(format!("---\n{yaml}\n---\n{body}"))
}

/// 解析SKILL.md
fn parse_skill_md(content: &str, dir_path: &Path) -> Option<(SkillMetadata, String)> {
    let trimmed = content.trim_start_matches('\u{feff}'); // 去除起始的连续字符`\u{feff}`，这个字符是`ZERO WIDTH NO-BREAK SPACE`，称为`Byte order mark`或`BOM`

    // 将但行skill转为标准的多行YAML格式
    let normalized;
    let input = if !trimmed.starts_with("---\n") && !trimmed.starts_with("---\r\n") {
        normalized = normalize_single_line_frontmatter(trimmed)?;
        &normalized
    } else {
        trimmed
    };

    let mut lines = input.lines();
    let _ = lines.next()?; // 跳过第一行`---`

    let mut yaml_block = String::new();
    let mut consumed = 0usize; // 已经读取的内容长度
    for line in lines {
        consumed += line.len() + 1; // 该行长度加上换行符
        if line.trim() == "---" || line.trim() == "..." {
            break // 结束name和description部分
        }
        yaml_block.push_str(line);
        yaml_block.push('\n');
    }

    if yaml_block.trim().is_empty() {
        return None
    }

    // 解析skill
    let fm: SkillFrontmatter = serde_yaml::from_str(&yaml_block).ok()?;
    let name = fm.name?.trim().to_string();
    if name.is_empty() {
        return None
    }

    // 获取支持的操作系统
    let mut platforms: Vec<String> = fm
        .platforms
        .into_iter()
        .chain(fm.compatibility.os)
        .map(|p| normalize_platform(&p))
        .filter(|p| !p.is_empty())
        .collect();
    platforms.sort();
    platforms.dedup();

    // 获取所需的依赖
    let mut deps: Vec<String> = fm
        .deps
        .into_iter()
        .chain(fm.compatibility.deps)
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .collect();
    deps.sort();
    deps.dedup();

    // 计算bosy之前内容的长度
    let header_len = if let Some(idx) = input.find("\n---\n") {
        idx + 5
    } else if let Some(idx) = input.find("\n...\n") {
        idx + 5
    } else {
        // fallback to consumed length from line-by-line scan
        4 + consumed // 如果没有找到description下一行的`---`或`...`，则用前面已读取内容长度加上文件起始的`---\n`4个字符
    };

    // 截取body部分
    let body = input
        .get(header_len..)
        .unwrap_or_default()
        .trim()
        .to_string();

    Some((
        SkillMetadata {
            name,
            description: fm.description,
            dir_path: dir_path.to_path_buf(),
            platforms,
            deps,
            source: fm
                .source
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "local".to_string()),
            version: fm
                .version
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            updated_at: fm
                .updated_at
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            env_file: fm
                .env_file
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        },
        body,
    ))
}
