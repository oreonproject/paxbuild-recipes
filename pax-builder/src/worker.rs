use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildJob {
    pub id: String,
    pub package_name: String,
    pub package_dir: PathBuf,
    pub architecture: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub worker_id: String,
    pub hostname: String,
    pub architecture: String,
    pub available_workers: usize,
    pub jobs_queued: usize,
    pub jobs_running: usize,
    pub jobs_completed: usize,
    pub jobs_failed: usize,
    pub status: String,
}

pub struct Worker {
    pub worker_id: String,
    pub hostname: String,
    pub architecture: String,
    pub server_url: String,
    pub worker_pool_size: usize,
    pub status_tx: mpsc::Sender<WorkerStatus>,
}

impl Worker {
    pub fn new(
        server_url: String,
        worker_pool_size: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let worker_id = Self::generate_worker_id();
        let hostname =
            std::env::var("HOSTNAME").unwrap_or_else(|_| format!("worker-{}", &worker_id[..8]));
        let architecture = format!("{:?}", std::env::consts::ARCH);

        let (status_tx, _status_rx) = mpsc::channel(100);

        Ok(Self {
            worker_id,
            hostname,
            architecture,
            server_url,
            worker_pool_size,
            status_tx,
        })
    }

    fn generate_worker_id() -> String {
        use std::time::SystemTime;
        format!(
            "{:x}",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Worker {} starting...", self.worker_id);
        println!("Server URL: {}", self.server_url);
        println!("Worker pool size: {}", self.worker_pool_size);

        Self::send_register(self).await?;

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            interval.tick().await;
            self.check_for_jobs().await?;
        }
    }

    async fn send_register(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "worker_id": self.worker_id,
            "hostname": self.hostname,
            "architecture": self.architecture,
            "available_workers": self.worker_pool_size,
        });

        client
            .post(format!("{}/api/workers/register", self.server_url))
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }

    async fn check_for_jobs(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/workers/next-job?worker_id={}",
            self.server_url, self.worker_id
        );

        let response = client.get(&url).send().await?;

        if response.status().is_success() {
            if let Ok(job) = response.json::<BuildJob>().await {
                Self::execute_job(self, job).await?;
            }
        }

        Ok(())
    }

    async fn execute_job(&self, job: BuildJob) -> Result<(), Box<dyn std::error::Error>> {
        println!("Executing job: {} for {}", job.id, job.package_name);

        let yaml_path = job.package_dir.join("pax.yaml");
        if !yaml_path.exists() {
            return Err(format!("pax.yaml not found for {}", job.package_name).into());
        }

        let output = tokio::process::Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("pax-builder")
            .arg("--")
            .arg("build")
            .arg(&yaml_path)
            .arg("--target")
            .arg(&job.architecture)
            .arg("--verbose")
            .output()
            .await?;

        let success = output.status.success();
        Self::report_job_result(&self.server_url, &job.id, success).await?;

        Ok(())
    }

    async fn report_job_result(
        server_url: &str,
        job_id: &str,
        success: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "job_id": job_id,
            "success": success,
        });

        client
            .post(format!("{}/api/workers/job-result", server_url))
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }

    pub fn get_status(&self) -> WorkerStatus {
        WorkerStatus {
            worker_id: self.worker_id.clone(),
            hostname: self.hostname.clone(),
            architecture: self.architecture.clone(),
            available_workers: self.worker_pool_size,
            jobs_queued: 0,
            jobs_running: 0,
            jobs_completed: 0,
            jobs_failed: 0,
            status: "running".to_string(),
        }
    }
}

pub struct BuildQueue {
    jobs: Vec<BuildJob>,
}

impl BuildQueue {
    pub fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    pub fn add_job(&mut self, job: BuildJob) {
        self.jobs.push(job);
    }

    pub fn next_job(&mut self) -> Option<BuildJob> {
        self.jobs.pop()
    }

    pub fn get_queue_length(&self) -> usize {
        self.jobs.len()
    }
}

impl Default for BuildQueue {
    fn default() -> Self {
        Self::new()
    }
}
