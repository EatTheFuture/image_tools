use std::collections::VecDeque;

use scheduled_thread_pool::{JobHandle, ScheduledThreadPool};
use shared_data::Shared;

/// A job queue that uses a single thread to process jobs.
pub struct JobQueue {
    runner: ScheduledThreadPool,
    job_status: Shared<JobStatus>,
}

impl JobQueue {
    pub fn new() -> JobQueue {
        JobQueue {
            runner: ScheduledThreadPool::new(1),
            job_status: Shared::new(JobStatus {
                jobs: VecDeque::new(),
                job_progress: None,
                log: VecDeque::new(),
                do_cancel: false,
                update_fn: None,
            }),
        }
    }

    /// Set a function to be run whenever there's an update from a job.
    ///
    /// Updates are:
    /// - A job starts.
    /// - A job updates its progress.
    /// - A job logs an error, warning, or note.
    /// - A job finishes.
    /// - Cancelation is requested.
    pub fn set_update_fn<F: Fn() + Send + 'static>(&mut self, cleanup_function: F) {
        self.job_status.lock_mut().update_fn = Some(Box::new(cleanup_function));
    }

    pub fn add_job<F>(&self, name: &str, job: F)
    where
        F: FnOnce(&Shared<JobStatus>) + Send + 'static,
    {
        let job_name1 = name.to_string();
        let job_name2 = name.to_string();
        let mut job_status = self.job_status.lock_mut();

        // Add the job.
        let local_job_status = self.job_status.clone_ref();
        job_status.jobs.push_back((
            self.runner.execute(move || {
                let job_status = local_job_status;

                if let Some(update_fn) = &job_status.lock().update_fn {
                    update_fn();
                }

                // Actually run the job.
                // TODO: this use of `AssertUndwindSafe` is a workaround
                // for the way `egui::Context` works, because we pass it
                // around frequently.
                if let Err(_) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&job_status)))
                {
                    job_status
                        .lock_mut()
                        .log_error(format!("ERROR: job \"{}\" panicked!", job_name1));
                }

                // Cleanup.
                let mut job_status = job_status.lock_mut();
                job_status.jobs.pop_front(); // This job.
                job_status.do_cancel = false;
                job_status.clear_progress();
                if let Some(update_fn) = &job_status.update_fn {
                    update_fn();
                }
            }),
            job_name2,
        ));
    }

    pub fn progress(&self) -> Option<(String, f32)> {
        self.job_status.lock().job_progress.clone()
    }

    pub fn job_count(&self) -> usize {
        self.job_status.lock().jobs.len()
    }

    pub fn cancel_all_jobs(&self) {
        let mut job_status = self.job_status.lock_mut();
        if !job_status.jobs.is_empty() {
            // Cancel all not-currently-running jobs.
            while job_status.jobs.len() > 1 {
                job_status.jobs.pop_back().unwrap().0.cancel()
            }

            // Mark currently running job for cancelation.
            job_status.do_cancel = true;
        }
        if let Some(update_fn) = &job_status.update_fn {
            update_fn();
        }
    }

    /// Cancel all jobs that aren't currently running.
    pub fn cancel_pending_jobs(&self) {
        let mut job_status = self.job_status.lock_mut();

        // Cancel all not-currently-running jobs.
        while job_status.jobs.len() > 1 {
            job_status.jobs.pop_back().unwrap().0.cancel();
        }
        if let Some(update_fn) = &job_status.update_fn {
            update_fn();
        }
    }

    pub fn cancel_jobs_with_name(&self, name: &str) {
        let mut job_status = self.job_status.lock_mut();
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
        if let Some(update_fn) = &job_status.update_fn {
            update_fn();
        }
    }

    /// Cancel all jobs that aren't currently running that match the given name.
    pub fn cancel_pending_jobs_with_name(&self, name: &str) {
        let mut job_status = self.job_status.lock_mut();
        if job_status.jobs.len() > 1 {
            // Cancel all not-currently-running jobs with name.
            for i in 0..(job_status.jobs.len() - 1) {
                let job_i = job_status.jobs.len() - 1 - i;
                if job_status.jobs[job_i].1 == name {
                    job_status.jobs.remove(job_i).unwrap().0.cancel();
                }
            }
        }
        if let Some(update_fn) = &job_status.update_fn {
            update_fn();
        }
    }

    pub fn is_canceling(&self) -> bool {
        self.job_status.lock().do_cancel
    }

    pub fn log_count(&self) -> usize {
        self.job_status.lock().log.len()
    }

    /// Index zero is the most recent error.
    pub fn get_log(&self, index: usize) -> Option<(String, LogLevel)> {
        self.job_status.lock().log.get(index).map(|l| l.clone())
    }

    pub fn clear_log(&self) {
        self.job_status.lock_mut().log.clear()
    }

    /// Convenience function for logging errors outside of a job.
    pub fn log_error(&self, message: String) {
        self.job_status.lock_mut().log_error(message);
    }

    /// Convenience function for logging warnings outside of a job.
    pub fn log_warning(&self, message: String) {
        self.job_status.lock_mut().log_warning(message);
    }

    /// Convenience function for logging notes outside of a job.
    pub fn log_note(&self, message: String) {
        self.job_status.lock_mut().log_note(message);
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
    update_fn: Option<Box<dyn Fn() + Send + 'static>>,
}

impl JobStatus {
    pub fn is_canceled(&self) -> bool {
        self.do_cancel
    }

    pub fn set_progress(&mut self, text: String, ratio: f32) {
        self.job_progress = Some((text, ratio));
        if let Some(update_fn) = &self.update_fn {
            update_fn();
        }
    }

    pub fn clear_progress(&mut self) {
        self.job_progress = None;
        if let Some(update_fn) = &self.update_fn {
            update_fn();
        }
    }

    pub fn log_error(&mut self, message: String) {
        self.log.push_front((message, LogLevel::Error));
        if let Some(update_fn) = &self.update_fn {
            update_fn();
        }
    }

    pub fn log_warning(&mut self, message: String) {
        self.log.push_front((message, LogLevel::Warning));
        if let Some(update_fn) = &self.update_fn {
            update_fn();
        }
    }

    pub fn log_note(&mut self, message: String) {
        self.log.push_front((message, LogLevel::Note));
        if let Some(update_fn) = &self.update_fn {
            update_fn();
        }
    }
}
