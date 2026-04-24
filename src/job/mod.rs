use std::path::PathBuf;
use std::sync::Arc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::solver::{self, SolveStatus};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub label: String,
    pub result: Option<String>,
    pub duration_ms: Option<u64>,
}

pub struct JobManager {
    jobs: Arc<DashMap<String, Job>>,
    handles: Arc<Mutex<Vec<(String, JoinHandle<()>)>>>,
    z3_bin: PathBuf,
}

impl JobManager {
    pub fn new(z3_bin: PathBuf) -> Self {
        Self {
            jobs: Arc::new(DashMap::new()),
            handles: Arc::new(Mutex::new(Vec::new())),
            z3_bin,
        }
    }

    #[allow(dead_code)]
    pub fn z3_bin(&self) -> &PathBuf { &self.z3_bin }

    pub async fn submit_with_bin(&self, label: String, smt_input: String, timeout_secs: u64, z3_bin: PathBuf) -> String {
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();

        let job = Job {
            id: id.clone(),
            status: JobStatus::Running,
            label,
            result: None,
            duration_ms: None,
        };
        self.jobs.insert(id.clone(), job);

        let jobs = self.jobs.clone();
        let jid = id.clone();

        let handle = tokio::spawn(async move {
            let result = solver::solve(&z3_bin, &smt_input, timeout_secs).await;

            if let Some(mut j) = jobs.get_mut(&jid) {
                j.status = match &result.status {
                    SolveStatus::Timeout | SolveStatus::Error(_) => JobStatus::Error,
                    _ => JobStatus::Done,
                };
                j.result = Some(result.to_compact());
                j.duration_ms = Some(result.duration_ms);
            }
        });

        let mut handles = self.handles.lock().await;
        handles.push((id.clone(), handle));
        id
    }

    pub fn status(&self, job_id: &str) -> Option<Job> {
        self.jobs.get(job_id).map(|j| j.clone())
    }

    pub fn result(&self, job_id: &str) -> Option<String> {
        self.jobs.get(job_id).and_then(|j| j.result.clone())
    }

    pub async fn cancel(&self, job_id: &str) -> bool {
        if let Some(mut j) = self.jobs.get_mut(job_id) {
            if j.status == JobStatus::Running || j.status == JobStatus::Pending {
                j.status = JobStatus::Cancelled;
                let mut handles = self.handles.lock().await;
                handles.retain(|(id, h)| {
                    if id == job_id { h.abort(); false } else { true }
                });
                return true;
            }
        }
        false
    }

    #[allow(dead_code)]
    pub fn list(&self) -> Vec<String> {
        self.jobs.iter().map(|j| format!("{} {:?} {}", j.id, j.status, j.label)).collect()
    }
}