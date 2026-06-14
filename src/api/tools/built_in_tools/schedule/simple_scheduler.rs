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

use crate::{
    parse_paras::PARAS,
    error::MyError,
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

/// 调度规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleType {
    // 每隔 N 秒执行一次
    Interval { secs: u64 },
    // 每天固定时间执行（24小时制）
    Daily { hour: u8, minute: u8 },
}

impl ScheduleType {
    /// 根据当前时间计算下一次触发时间
    pub fn next_run_time(&self) -> DateTime<Utc> {
        match self {
            ScheduleType::Interval { secs } => {
                Utc::now() + ChronoDuration::seconds(*secs as i64)
            }
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
            }
        }
    }
}

/// 任务结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    pub schedule: ScheduleType,
    pub tool_name: String,
    pub tool_args: String,
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
            tokio::spawn(async move {
                event!(Level::INFO, "⏰ Job '{}' fired: calling tool '{}'", job.name, job.tool_name);

                // 🔥 在这里调用你的 agent 工具系统
                //    例如: agent.call_tool(&job.tool_name, &job.tool_args).await
                match PARAS.tools.run(&job.tool_name, &job.tool_args) {
                    Ok(r) => event!(Level::INFO, "Job '{}' call tool '{}' successfull: {}", job.name, job.tool_name, r.0),
                    Err(e) => event!(Level::ERROR, "Job '{}' call tool '{}' failed: {}", job.name, job.tool_name, e),
                }

                // 重新计算下次运行时间
                if let Some(mut entry) = jobs.get_mut(&job.id) {
                    entry.next_run = Some(entry.schedule.next_run_time());
                }
            });
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
