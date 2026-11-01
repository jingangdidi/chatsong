//pub mod simple_scheduler;
pub mod simple_schedule;
pub mod complex_schedule;

pub use simple_schedule::{
    SimpleScheduleTask,
    run_simple_schedule_task,
};
pub use complex_schedule::{
    ComplexScheduleTask,
    run_complex_schedule_task,
    run_tools,
};

//use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Local, TimeZone, Utc};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tracing::{event, Level};
use uuid::Uuid;
use openai_dive::v1::resources::chat::ChatMessage;

use crate::{
    parse_paras::PARAS,
    error::MyError,
    api::handlers::chat::ForComplex,
};

// 全局管道
pub static SCHEDULER_CMD_TX: Lazy<Mutex<Option<mpsc::Sender<SchedulerCmd>>>> = Lazy::new(|| Mutex::new(None));

/// 初始化调度器管道（必须在程序启动时调用一次）
pub fn start_scheduler(tick_secs: u64) {
    let scheduler = SimpleScheduler::new(tick_secs);
    let (cmd_tx, cmd_rx) = mpsc::channel(32);

    let mut data = SCHEDULER_CMD_TX.lock().unwrap();
    *data = Some(cmd_tx);

    // 启动 actor 运行调度循环与命令处理
    tokio::spawn(scheduler.run_actor(cmd_rx));
}

/// 获取全局命令发送器，向定时任务发送新命令（增加、删除、查看）
pub fn get_cmd_tx() -> Option<mpsc::Sender<SchedulerCmd>> {
    let data = SCHEDULER_CMD_TX.lock().unwrap();
    data.clone()
}

/// 定时任务的参数
#[derive(Debug, Deserialize)]
pub struct ScheduleConfig {
    //#[serde(rename = "jobType")]
    job_type: String,      // 类型
    #[serde(default)]
    hour:     Option<u8>,  // 时，指定时间执行
    #[serde(default)]
    minute:   Option<u8>,  // 分，指定时间执行
    #[serde(default)]
    secs:     Option<u64>, // 秒，间隔几秒运行一次
    #[serde(default)]
    start_hour: Option<u8>, // 开始时，daily_range 时间段的起始小时（0-23）
    #[serde(default)]
    end_hour:   Option<u8>, // 结束时，daily_range 时间段的结束小时（0-23，含）
}

/// 调度规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleType {
    // 每隔 N 秒执行一次
    Interval { secs: u64 },
    // 每天固定时间执行（24小时制）
    Daily { hour: u8, minute: u8 },
    // 每天在 [start_hour:minute, end_hour:minute] 时间段内每隔 every_secs 秒执行一次
    // 触发点对齐到窗口起点：start, start+every_secs, ...，最后一个不超过窗口终点（含）
    // 例：每天 7~18 点每小时执行 => DailyRange { start_hour: 7, end_hour: 18, minute: 0, every_secs: 3600 }
    DailyRange { start_hour: u8, end_hour: u8, minute: u8, every_secs: u64 },
}

impl ScheduleType {
    /// 根据当前时间计算下一次触发时间
    pub fn next_run_time(&self) -> DateTime<Utc> {
        match self {
            ScheduleType::Interval { secs } => {
                Utc::now() + ChronoDuration::seconds(*secs as i64)
            },
            ScheduleType::Daily { hour, minute } => {
                let now = Local::now();
                let today_target = now
                    .date_naive()
                    .and_hms_opt(*hour as u32, *minute as u32, 0)
                    .unwrap();
                let local_dt = Local
                    .from_local_datetime(&today_target)
                    .single()
                    .unwrap();
                if local_dt > now {
                    local_dt.with_timezone(&Utc)
                } else {
                    (local_dt + ChronoDuration::days(1)).with_timezone(&Utc)
                }
            },
            ScheduleType::DailyRange { start_hour, end_hour, minute, every_secs } => {
                let now = Local::now();
                let now_ts = now.timestamp();
                let today = now.date_naive();
                // 依次检查今天、明天的窗口，返回第一个晚于 now 的触发点
                for day_offset in 0..2 {
                    let date = today + ChronoDuration::days(day_offset);
                    let start_local = Local
                        .from_local_datetime(&date.and_hms_opt(*start_hour as u32, *minute as u32, 0).unwrap())
                        .single()
                        .unwrap();
                    let end_local = Local
                        .from_local_datetime(&date.and_hms_opt(*end_hour as u32, *minute as u32, 0).unwrap())
                        .single()
                        .unwrap();
                    let start_ts = start_local.timestamp();
                    let end_ts = end_local.timestamp();
                    if now_ts < start_ts {
                        // 今天的窗口还没开始，第一个触发点就是窗口起点
                        return start_local.with_timezone(&Utc);
                    }
                    if now_ts <= end_ts {
                        // 在窗口内，找对齐到窗口起点的下一个触发点：start + k * every_secs
                        let every = *every_secs as i64;
                        let elapsed = now_ts - start_ts;
                        let next_ts = start_ts + (elapsed / every + 1) * every;
                        if next_ts <= end_ts {
                            return Utc.timestamp_opt(next_ts, 0).single().unwrap();
                        }
                        // 今天的触发点已用完，继续看明天
                    }
                }
                // 兜底（理论不可达）：明天窗口起点
                let tomorrow = today + ChronoDuration::days(1);
                Local
                    .from_local_datetime(&tomorrow.and_hms_opt(*start_hour as u32, *minute as u32, 0).unwrap())
                    .single()
                    .unwrap()
                    .with_timezone(&Utc)
            },
        }
    }
}

// 任务类型，简单或复杂，简单任务只调用一个tool，复杂任务通过agent调用skill、多个tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimpleComplex {
    Simple{
        tool_name: String,
        tool_id: String,
        tool_args: String,
    },
    Complex{
        prompt: String,
        model: ForComplex,
        remember_context: bool, // 是否记住之前执行的上下文，默认false每次独立
        history: Vec<ChatMessage>,
    },
}

impl SimpleComplex {
    /// 简单任务
    pub fn new_simple(tool_name: String, tool_id: String, tool_args: String) -> Self {
        Self::Simple{tool_name, tool_id, tool_args}
    }

    /// 复杂任务
    pub fn new_complex(prompt: String, model: ForComplex, remember_context: bool, history: Vec<ChatMessage>) -> Self {
        Self::Complex{prompt, model, remember_context, history}
    }
}

/// 任务结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    pub schedule: ScheduleType,
    pub task: SimpleComplex,
    pub enabled: bool,
    pub next_run: Option<DateTime<Utc>>,
}

/// 调度器 Actor
pub struct SimpleScheduler {
    jobs: Arc<DashMap<String, ScheduledJob>>,
    tick_interval_secs: u64,
}

/// 调度器命令
pub enum SchedulerCmd {
    AddJob {
        job: ScheduledJob,
        reply: oneshot::Sender<Result<String, MyError>>,
    },
    RemoveJob {
        id: String,
        reply: oneshot::Sender<Result<(), MyError>>,
    },
    ListJobs {
        reply: oneshot::Sender<Vec<ScheduledJob>>,
    },
}

impl SimpleScheduler {
    pub fn new(tick_interval_secs: u64) -> Self {
        Self {
            jobs: Arc::new(DashMap::new()),
            tick_interval_secs,
        }
    }

    /// 同步内部操作，供 actor 使用
    fn add_job(&self, mut job: ScheduledJob) -> Result<String, MyError> {
        if job.id.is_empty() {
            job.id = Uuid::new_v4().to_string();
        }
        if self.jobs.iter().any(|e| e.value().name == job.name) {
            return Err(MyError::OtherError{info: format!("schedule job '{}' already exist", job.name)});
        }
        job.next_run = Some(job.schedule.next_run_time());
        let id = job.id.clone();
        self.jobs.insert(id.clone(), job);
        Ok(id)
    }

    /// 移除指定任务
    fn remove_job(&self, id: &str) -> Result<(), MyError> {
        self.jobs
            .remove(id)
            .ok_or_else(|| MyError::OtherError{info: format!("schedule job '{id}' not exist")})?;
        Ok(())
    }

    /// 查看当前所有任务
    fn list_jobs(&self) -> Vec<ScheduledJob> {
        self.jobs.iter().map(|e| e.value().clone()).collect()
    }

    /// 扫描并触发到期任务（内部异步方法）
    async fn fire_due_jobs(&self) {
        let now = Utc::now();
        let due: Vec<ScheduledJob> = self
            .jobs
            .iter()
            .filter(|e| e.value().enabled && e.value().next_run.is_some_and(|t| t <= now))
            .map(|e| e.value().clone())
            .collect();

        for job in due {
            // 防止重复触发
            if let Some(mut entry) = self.jobs.get_mut(&job.id) {
                entry.next_run = None;
            } else {
                continue;
            }

            let jobs = self.jobs.clone();
            match job.task.clone() {
                SimpleComplex::Simple{tool_name, tool_id, tool_args} => {
                    tokio::spawn(async move {
                        event!(Level::INFO, "Job '{}' fired: calling tool '{}'", job.name, tool_name);
                        // 🔥 在这里调用你的 agent 工具系统
                        //    例如: agent.call_tool(&job.tool_name, &job.tool_args).await
                        match PARAS.tools.run(&tool_id, &tool_args) {
                            Ok(r) => event!(Level::INFO, "Job '{}' call tool '{}' successfull: {}", job.name, tool_name, r.0),
                            Err(e) => event!(Level::ERROR, "Job '{}' call tool '{}' failed: {}", job.name, tool_name, e),
                        }
                    });
                },
                SimpleComplex::Complex{prompt, model, remember_context, history} => {
                    match run_tools(
                        &model.uuid,
                        prompt,
                        model.model,
                        model.api_key,
                        model.endpoint,
                        model.reasoning,
                        model.effort,
                        model.tools,
                        model.skills,
                        history,
                    ).await {
                        Ok((result, logs)) => {
                            event!(Level::INFO, "{} Job '{}' successfull:\n{}", model.uuid, job.name, result);
                            if remember_context {
                                if let Some(mut entry) = jobs.get_mut(&job.id) {
                                    if let SimpleComplex::Complex{ref mut history, ..} = entry.task {
                                        *history = logs;
                                    }
                                }
                            }
                        },
                        Err(e) => event!(Level::ERROR, "{} Job '{}' failed: {}", model.uuid, job.name, e),
                    }
                },
            }
            // 重新计算下次运行时间
            if let Some(mut entry) = jobs.get_mut(&job.id) {
                entry.next_run = Some(entry.schedule.next_run_time());
            }
        }
    }

    /// 启动 actor：同时处理命令和定时扫描
    async fn run_actor(self, mut cmd_rx: mpsc::Receiver<SchedulerCmd>) {
        let mut ticker = tokio::time::interval(Duration::from_secs(self.tick_interval_secs));

        loop {
            tokio::select! {
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        SchedulerCmd::AddJob { job, reply } => {
                            let res = self.add_job(job);
                            let _ = reply.send(res);
                        }
                        SchedulerCmd::RemoveJob { id, reply } => {
                            let res = self.remove_job(&id);
                            let _ = reply.send(res);
                        }
                        SchedulerCmd::ListJobs { reply } => {
                            let jobs = self.list_jobs();
                            let _ = reply.send(jobs);
                        }
                    }
                }
                _ = ticker.tick() => {
                    self.fire_due_jobs().await;
                }
            }
        }
    }
}
