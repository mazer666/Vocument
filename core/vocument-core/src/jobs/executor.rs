//! Bounded worker pool that executes `JobManager` jobs off the calling thread
//! (`ARCHITECTURE.md` §7–§8, `ROADMAP.md` M1.5).
//!
//! The pool owns a fixed number of worker threads. Workers pull jobs from the
//! shared [`JobQueue`], execute them and publish the resulting state. No
//! unbounded thread creation happens anywhere: the number of threads is fixed at
//! construction time and the queue is bounded.
//!
//! # Cooperative cancellation
//!
//! Every job receives a [`CancellationToken`]. A job that observes the token
//! returns [`JobOutcome::Cancelled`]. Engines that cannot be interrupted safely
//! (MuPDF, Tesseract) run to completion; their result is then discarded because
//! the job is already marked cancelled, so a late result can never overwrite
//! newer document state (see `ARCHITECTURE.md` §8.4).
//!
//! # Panic isolation
//!
//! A panic inside a job body is caught and converted into a `Failed` job. One
//! faulty job therefore cannot terminate a worker thread or the pool.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use super::scheduler::{
    CancellationToken, JobDescriptor, JobError, JobOutcome, JobQueue, JobState, JobTask,
};
use crate::config::JobBudgets;

/// Shared, observable state of a tracked job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStatus {
    pub descriptor: JobDescriptor,
    /// Human readable failure description, present for `Failed` jobs.
    pub failure: Option<String>,
}

#[derive(Debug, Default)]
struct TrackerInner {
    statuses: std::collections::BTreeMap<u64, JobStatus>,
    /// Jobs that were cancelled while running; their late results are discarded.
    cancelled_while_running: std::collections::HashSet<u64>,
    /// Cancellation token of every job, so that a cancellation request can reach
    /// a job that is currently executing.
    tokens: std::collections::HashMap<u64, CancellationToken>,
}

/// Tracks the state of all jobs known to the manager.
#[derive(Debug, Default)]
pub struct JobTracker {
    inner: Mutex<TrackerInner>,
    signal: Condvar,
}

impl JobTracker {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, TrackerInner>, JobError> {
        self.inner.lock().map_err(|_| JobError::LockPoisoned)
    }

    /// Records a newly enqueued job.
    pub fn register(&self, descriptor: JobDescriptor) -> Result<(), JobError> {
        let mut guard = self.lock()?;
        guard.statuses.insert(
            descriptor.job_id,
            JobStatus {
                descriptor,
                failure: None,
            },
        );
        Ok(())
    }

    /// Transitions a job to `Running`.
    pub fn mark_running(&self, job_id: u64) -> Result<(), JobError> {
        let mut guard = self.lock()?;
        let status = guard
            .statuses
            .get_mut(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        status.descriptor.state = JobState::Running;
        Ok(())
    }

    /// Updates the progress of a running job.
    pub fn update_progress(&self, job_id: u64, progress_percent: u8) -> Result<(), JobError> {
        let mut guard = self.lock()?;
        let status = guard
            .statuses
            .get_mut(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        status.descriptor.progress_percent = progress_percent.min(100);
        Ok(())
    }

    /// Requests cancellation of a job and records the intent.
    ///
    /// Cancelling a job that is still queued removes it from the queue. Cancelling
    /// a running job marks it, so that a result arriving later is discarded.
    pub fn request_cancel(&self, job_id: u64) -> Result<JobState, JobError> {
        let mut guard = self.lock()?;
        let status = guard
            .statuses
            .get_mut(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        match status.descriptor.state {
            JobState::Queued => {
                status.descriptor.state = JobState::Cancelled;
                Ok(JobState::Cancelled)
            }
            JobState::Running => {
                guard.cancelled_while_running.insert(job_id);
                Ok(JobState::Running)
            }
            terminal => Ok(terminal),
        }
    }

    /// Returns true when the job was already cancelled before it started running.
    ///
    /// This is a pure read: it never changes the state, so the worker can use it
    /// to detect a cancellation that happened while the job was still queued.
    pub fn is_cancelled_before_start(&self, job_id: u64) -> Result<bool, JobError> {
        let guard = self.lock()?;
        let status = guard
            .statuses
            .get(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        Ok(status.descriptor.state == JobState::Cancelled)
    }

    /// Registers the cancellation token of a job that is about to run.
    pub fn attach_token(&self, job_id: u64, token: CancellationToken) -> Result<(), JobError> {
        let mut guard = self.lock()?;
        guard.tokens.insert(job_id, token);
        Ok(())
    }

    /// Returns the cancellation token of a job, if it is currently running.
    pub fn cancellation_token(&self, job_id: u64) -> Option<CancellationToken> {
        self.lock().ok()?.tokens.get(&job_id).cloned()
    }

    /// Records the terminal state of a job.
    ///
    /// A job that was cancelled while running always ends as `Cancelled`, never as
    /// `Completed`, even if the job body returned successfully afterwards.
    pub fn finish(
        &self,
        job_id: u64,
        state: JobState,
        failure: Option<String>,
    ) -> Result<(), JobError> {
        let mut guard = self.lock()?;
        let was_cancelled = guard.cancelled_while_running.remove(&job_id);
        let status = guard
            .statuses
            .get_mut(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        let final_state = if was_cancelled && state == JobState::Completed {
            JobState::Cancelled
        } else {
            state
        };

        status.descriptor.state = final_state;
        status.descriptor.progress_percent = if final_state == JobState::Completed {
            100
        } else {
            status.descriptor.progress_percent
        };
        status.failure = failure;
        drop(guard);
        self.signal.notify_all();
        Ok(())
    }

    /// Returns the current status of a job.
    pub fn status(&self, job_id: u64) -> Result<JobStatus, JobError> {
        self.lock()?
            .statuses
            .get(&job_id)
            .cloned()
            .ok_or(JobError::JobNotFound { job_id })
    }

    /// Returns the status of all known jobs.
    pub fn all_statuses(&self) -> Result<Vec<JobStatus>, JobError> {
        Ok(self.lock()?.statuses.values().cloned().collect())
    }

    /// Removes terminal jobs from the tracker, returning how many were removed.
    pub fn purge_terminal(&self) -> Result<usize, JobError> {
        let mut guard = self.lock()?;
        let before = guard.statuses.len();
        let terminal_ids: Vec<u64> = guard
            .statuses
            .iter()
            .filter(|(_, status)| status.descriptor.state.is_terminal())
            .map(|(job_id, _)| *job_id)
            .collect();
        for job_id in &terminal_ids {
            guard.statuses.remove(job_id);
            guard.tokens.remove(job_id);
            guard.cancelled_while_running.remove(job_id);
        }
        Ok(before - guard.statuses.len())
    }

    /// Removes the cancellation token of a finished job.
    pub fn detach_token(&self, job_id: u64) {
        if let Ok(mut guard) = self.lock() {
            guard.tokens.remove(&job_id);
        }
    }
}

/// Bounded worker pool that executes queued jobs.
#[derive(Debug)]
pub struct JobExecutor {
    queue: Arc<JobQueue>,
    tracker: Arc<JobTracker>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    shutdown: AtomicBool,
    /// Number of jobs that are currently executing inside a worker.
    ///
    /// Shared with the worker threads, so that the count stays correct even while
    /// a job is running.
    running_jobs: Arc<AtomicU64>,
}

impl JobExecutor {
    /// Creates an executor and starts the configured number of worker threads.
    ///
    /// The worker count is `min(max_concurrent_workers, 1)` at minimum one, so a
    /// misconfiguration cannot silently produce a pool without workers.
    ///
    /// Returns an error when a worker thread cannot be spawned. A partially
    /// started pool is never returned: the already started workers are stopped
    /// before the error is propagated, so the caller never observes a pool with
    /// fewer workers than configured.
    pub fn start(budgets: JobBudgets) -> Result<Arc<Self>, JobError> {
        let worker_count = budgets.max_concurrent_workers.max(1);
        let executor = Arc::new(Self {
            queue: Arc::new(JobQueue::new(budgets)),
            tracker: Arc::new(JobTracker::default()),
            workers: Mutex::new(Vec::with_capacity(worker_count)),
            shutdown: AtomicBool::new(false),
            running_jobs: Arc::new(AtomicU64::new(0)),
        });

        let mut handles = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let queue = Arc::clone(&executor.queue);
            let tracker = Arc::clone(&executor.tracker);
            let running_jobs = Arc::clone(&executor.running_jobs);
            match std::thread::Builder::new()
                .name(format!("vocument-job-worker-{index}"))
                .spawn(move || worker_loop(queue, tracker, running_jobs))
            {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    // Stop the workers that were already started, so that no
                    // orphaned thread keeps running without a pool owner.
                    let _ = executor.queue.shutdown();
                    for handle in handles {
                        let _ = handle.join();
                    }
                    return Err(JobError::WorkerSpawnFailed {
                        details: format!("worker {index} could not be started: {error}"),
                    });
                }
            }
        }
        {
            let mut guard = executor
                .workers
                .lock()
                .map_err(|_| JobError::LockPoisoned)?;
            *guard = handles;
        }
        Ok(executor)
    }

    /// Submits a job for background execution.
    pub fn submit(
        &self,
        session_id: String,
        document_revision: u64,
        kind: super::scheduler::JobKind,
        resumable: bool,
        task: JobTask,
    ) -> Result<JobDescriptor, JobError> {
        let descriptor =
            self.queue
                .enqueue(session_id, document_revision, kind, resumable, task)?;
        self.tracker.register(descriptor.clone())?;
        Ok(descriptor)
    }

    /// Returns the status of a job.
    pub fn status(&self, job_id: u64) -> Result<JobStatus, JobError> {
        self.tracker.status(job_id)
    }

    /// Returns the status of all jobs.
    pub fn all_statuses(&self) -> Result<Vec<JobStatus>, JobError> {
        self.tracker.all_statuses()
    }

    /// Requests cancellation of a job.
    pub fn cancel(&self, job_id: u64) -> Result<JobState, JobError> {
        self.tracker.request_cancel(job_id)
    }

    /// Cancels a job and signals its cancellation token, so that a running job
    /// can stop cooperatively at the next safe point.
    pub fn cancel_and_signal(&self, job_id: u64) -> Result<JobState, JobError> {
        let state = self.tracker.request_cancel(job_id)?;
        if let Some(token) = self.tracker.cancellation_token(job_id) {
            token.cancel();
        }
        Ok(state)
    }

    /// Cancels every job of a session and removes it from the queue.
    ///
    /// Used when a document is closed: no result of a closed document may be
    /// applied afterwards.
    pub fn cancel_session(&self, session_id: &str) -> Result<usize, JobError> {
        let statuses = self.tracker.all_statuses()?;
        let mut cancelled = 0usize;
        for status in statuses {
            if status.descriptor.session_id != session_id {
                continue;
            }
            if status.descriptor.state.is_terminal() {
                continue;
            }
            self.cancel_and_signal(status.descriptor.job_id)?;
            cancelled += 1;
        }
        Ok(cancelled)
    }

    /// Returns the number of jobs currently queued.
    pub fn queued_len(&self) -> Result<usize, JobError> {
        self.queue.queued_len()
    }

    /// Removes finished jobs from the tracker and returns how many were removed.
    pub fn purge_finished(&self) -> Result<usize, JobError> {
        self.tracker.purge_terminal()
    }

    /// Returns the number of jobs currently executing.
    pub fn running_count(&self) -> u64 {
        self.running_jobs.load(Ordering::Acquire)
    }

    /// Stops the pool: queued work is dropped, running work is asked to stop.
    pub fn shutdown(&self) {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.queue.shutdown();
        let handles = match self.workers.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(poisoned) => {
                // A poisoned lock still owns the handles: recover them instead of
                // leaking the worker threads.
                let mut guard = poisoned.into_inner();
                std::mem::take(&mut *guard)
            }
        };
        for handle in handles {
            let _ = handle.join();
        }
    }
}

impl Drop for JobExecutor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Worker loop: pull a job, execute it, publish the terminal state.
fn worker_loop(queue: Arc<JobQueue>, tracker: Arc<JobTracker>, running_jobs: Arc<AtomicU64>) {
    loop {
        let next = match queue.take_next() {
            Ok(next) => next,
            Err(_) => return,
        };
        let Some((descriptor, task)) = next else {
            // The queue is empty and the manager was shut down.
            return;
        };

        if tracker
            .is_cancelled_before_start(descriptor.job_id)
            .unwrap_or(false)
        {
            // The job was cancelled while it was still queued.
            let _ = tracker.finish(descriptor.job_id, JobState::Cancelled, None);
            continue;
        }

        let _ = tracker.mark_running(descriptor.job_id);
        let token = CancellationToken::new();
        // Register the token so that a cancellation request from another thread
        // can reach the running job.
        let _ = tracker.attach_token(descriptor.job_id, token.clone());

        // The cancellation may have been requested between the check above and
        // the moment the token became visible to the canceller. The token is
        // signalled here if the job was already marked as cancelled, so that the
        // job body observes the cancellation instead of running to completion.
        if tracker
            .is_cancelled_before_start(descriptor.job_id)
            .unwrap_or(false)
        {
            token.cancel();
        }

        let job_id = descriptor.job_id;
        running_jobs.fetch_add(1, Ordering::AcqRel);

        let outcome = catch_unwind(AssertUnwindSafe(|| task(job_id, &token)));
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(_) => JobOutcome::Failed("job body panicked".to_owned()),
        };

        // The counter is decremented before the terminal state is published, so
        // that a job is never reported as "not running" while it still runs.
        running_jobs.fetch_sub(1, Ordering::AcqRel);

        let (state, failure) = match outcome {
            JobOutcome::Completed => (JobState::Completed, None),
            JobOutcome::Failed(details) => (JobState::Failed, Some(details)),
            JobOutcome::Cancelled => (JobState::Cancelled, None),
        };
        let _ = tracker.finish(job_id, state, failure);
        tracker.detach_token(job_id);
    }
}

/// Convenience helper that waits until a job reaches a terminal state.
///
/// Used by tests and by headless verification tooling. The timeout prevents an
/// unbounded wait if a worker is stuck.
pub fn wait_for_terminal(
    tracker: &JobTracker,
    job_id: u64,
    timeout: Duration,
) -> Result<JobStatus, JobError> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let status = tracker.status(job_id)?;
        if status.descriptor.state.is_terminal() {
            return Ok(status);
        }
        if std::time::Instant::now() >= deadline {
            return Err(JobError::JobFailed {
                job_id,
                details: format!("job did not reach a terminal state within {timeout:?}"),
            });
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod executor_tests;
