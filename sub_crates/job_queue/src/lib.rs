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
                do_cancel: false,
            })),
        }
    }

    pub fn add_job<F>(&self, job: F) -> bool
    where
        F: FnOnce(&Mutex<JobStatus>) + Send + 'static,
    {
        let mut job_status = self.job_status.lock().unwrap();
        if job_status.do_cancel {
            // Don't allow adding jobs when in the middle of canceling.
            return false;
        }

        // Add the job.
        let local_job_status = Arc::clone(&self.job_status);
        job_status.jobs.push_back(self.runner.execute(move || {
            let job_status = local_job_status;

            // Actually run the job.
            job(&job_status);

            // Cleanup.
            let mut job_status = job_status.lock().unwrap();
            job_status.jobs.pop_front(); // The job we just finished.
            if job_status.do_cancel {
                for job in job_status.jobs.drain(..) {
                    job.cancel();
                }
                job_status.do_cancel = false;
            }
            job_status.clear_progress();
        }));

        true
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
            job_status.do_cancel = true;
        }
    }
}

pub struct JobStatus {
    jobs: VecDeque<JobHandle>,
    job_progress: Option<(String, f32)>,
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
}

#[cfg(test)]
mod tests {}
