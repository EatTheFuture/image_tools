use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use scheduled_thread_pool::{JobHandle, ScheduledThreadPool};

/// A job queue that uses a single thread to process jobs.
pub struct JobQueue {
    runner: ScheduledThreadPool,
    job_status: Arc<Mutex<JobStatus>>,
}

impl JobQueue {
    pub fn new() -> JobQueue {
        JobQueue {
            runner: ScheduledThreadPool::new(1),
            job_status: Arc::new(Mutex::new(JobStatus {
                jobs: VecDeque::new(),
                job_progress: None,
                log: VecDeque::new(),
                do_cancel: false,
            })),
        }
    }

    pub fn add_job<F>(&self, name: &str, job: F)
    where
        F: FnOnce(&Mutex<JobStatus>) + Send + std::panic::UnwindSafe + 'static,
    {
        let job_name1 = name.to_string();
        let job_name2 = name.to_string();
        let mut job_status = self.job_status.lock().unwrap();

        // Add the job.
        let local_job_status = Arc::clone(&self.job_status);
        job_status.jobs.push_back((
            self.runner.execute(move || {
                let job_status = local_job_status;

                // Actually run the job.
                if let Err(_) = std::panic::catch_unwind(|| job(&job_status)) {
                    job_status
                        .lock()
                        .unwrap()
                        .log_error(format!("ERROR: job \"{}\" panicked!", job_name1));
                }

                // Cleanup.
                let mut job_status = job_status.lock().unwrap();
                job_status.jobs.pop_front(); // This job.
                job_status.do_cancel = false;
                job_status.clear_progress();
            }),
            job_name2,
        ));
    }

    pub fn progress(&self) -> Option<(String, f32)> {
        self.job_status.lock().unwrap().job_progress.clone()
    }

    pub fn job_count(&self) -> usize {
        self.job_status.lock().unwrap().jobs.len()
    }

    pub fn cancel_all_jobs(&self) {
        let mut job_status = self.job_status.lock().unwrap();
        if !job_status.jobs.is_empty() {
            // Cancel all not-currently-running jobs.
            while job_status.jobs.len() > 1 {
                job_status.jobs.pop_back().unwrap().0.cancel()
            }

            // Mark currently running job for cancelation.
            job_status.do_cancel = true;
        }
    }

    /// Cancel all jobs that aren't currently running.
    pub fn cancel_pending_jobs(&self) {
        let mut job_status = self.job_status.lock().unwrap();

        // Cancel all not-currently-running jobs.
        while job_status.jobs.len() > 1 {
            job_status.jobs.pop_back().unwrap().0.cancel();
        }
    }

    pub fn cancel_jobs_with_name(&self, name: &str) {
        let mut job_status = self.job_status.lock().unwrap();
        if !job_status.jobs.is_empty() {
            // Cancel all not-currently-running jobs with name.
            for i in 0..(job_status.jobs.len() - 1) {
                let job_i = job_status.jobs.len() - 1 - i;
                if job_status.jobs[job_i].1 == name {
                    job_status.jobs.remove(job_i).unwrap().0.cancel();
                }
            }

            // Mark currently running job for cancelation if its name matches.
            if job_status.jobs[0].1 == name {
                job_status.do_cancel = true;
            }
        }
    }

    /// Cancel all jobs that aren't currently running that match the given name.
    pub fn cancel_pending_jobs_with_name(&self, name: &str) {
        let mut job_status = self.job_status.lock().unwrap();
        if job_status.jobs.len() > 1 {
            // Cancel all not-currently-running jobs with name.
            for i in 0..(job_status.jobs.len() - 1) {
                let job_i = job_status.jobs.len() - 1 - i;
                if job_status.jobs[job_i].1 == name {
                    job_status.jobs.remove(job_i).unwrap().0.cancel();
                }
            }
        }
    }

    pub fn is_canceling(&self) -> bool {
        self.job_status.lock().unwrap().do_cancel
    }

    pub fn log_count(&self) -> usize {
        self.job_status.lock().unwrap().log.len()
    }

    /// Index zero is the most recent error.
    pub fn get_log(&self, index: usize) -> (String, LogLevel) {
        self.job_status.lock().unwrap().log[index].clone()
    }

    pub fn clear_log(&self) {
        self.job_status.lock().unwrap().log.clear()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LogLevel {
    Error,
    Warning,
    Note,
}

pub struct JobStatus {
    jobs: VecDeque<(JobHandle, String)>, // (handle, name)
    job_progress: Option<(String, f32)>,
    log: VecDeque<(String, LogLevel)>,
    do_cancel: bool,
}

impl JobStatus {
    pub fn is_canceled(&self) -> bool {
        self.do_cancel
    }

    pub fn set_progress(&mut self, text: String, ratio: f32) {
        self.job_progress = Some((text, ratio));
    }

    pub fn clear_progress(&mut self) {
        self.job_progress = None;
    }

    pub fn log_error(&mut self, message: String) {
        self.log.push_front((message, LogLevel::Error));
    }

    pub fn log_warning(&mut self, message: String) {
        self.log.push_front((message, LogLevel::Warning));
    }

    pub fn log_note(&mut self, message: String) {
        self.log.push_front((message, LogLevel::Note));
    }
}
