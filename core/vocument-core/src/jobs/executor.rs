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
//!
//! # Bounded shutdown
//!
//! The shutdown signals every running job and then waits a bounded time for the
//! workers to end. A worker that is stuck inside an engine call cannot be
//! stopped from the outside; it is detached after the grace period, so that the
//! shutdown — and with it the exit of the application — never blocks forever.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::power::{PowerLimits, PowerMonitor, PowerState};
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

    /// Moves a running job back into the queue (`ROADMAP.md` M1.5, pause).
    ///
    /// A job that is interrupted by a pause is not a failed job. The state goes
    /// back to `Queued`, so that the job can be picked up again after the
    /// resume. A job that was cancelled by the user stays cancelled.
    ///
    /// Returns `true` when the job was put back into the queue.
    pub fn requeue(&self, job_id: u64) -> Result<bool, JobError> {
        let mut guard = self.lock()?;
        if guard.cancelled_while_running.remove(&job_id) {
            // The user cancelled the job; that decision is not reversed.
            return Ok(false);
        }
        let status = guard
            .statuses
            .get_mut(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        if status.descriptor.state != JobState::Running {
            return Ok(false);
        }
        status.descriptor.state = JobState::Queued;
        status.descriptor.progress_percent = 0;
        guard.tokens.remove(&job_id);
        Ok(true)
    }

    /// Returns the ids of the jobs that are currently running.
    pub fn running_job_ids(&self) -> Vec<u64> {
        let Ok(guard) = self.lock() else {
            return Vec::new();
        };
        guard
            .statuses
            .values()
            .filter(|status| status.descriptor.state == JobState::Running)
            .map(|status| status.descriptor.job_id)
            .collect()
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

    /// Signals the cancellation token of every job that is currently running.
    ///
    /// This is used when the pool is shut down. Without it, a running job would
    /// never be asked to stop, and the shutdown would block until the job
    /// finished on its own.
    pub fn cancel_all_running(&self) -> Result<usize, JobError> {
        let guard = self.lock()?;
        let tokens: Vec<CancellationToken> = guard.tokens.values().cloned().collect();
        let count = tokens.len();
        drop(guard);
        for token in tokens {
            token.cancel();
        }
        Ok(count)
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

    /// Removes a job from the tracker that never entered the queue.
    ///
    /// A job that was reserved but not published cannot run and therefore
    /// cannot reach a terminal state. It must not stay in the tracker as a
    /// job that waits for ever.
    pub fn forget(&self, job_id: u64) {
        if let Ok(mut guard) = self.lock() {
            guard.statuses.remove(&job_id);
            guard.tokens.remove(&job_id);
            guard.cancelled_while_running.remove(&job_id);
        }
    }
}

/// A worker thread together with the signal that reports its end.
struct Worker {
    join: JoinHandle<()>,
    exit: mpsc::Receiver<()>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Worker")
            .field("join", &"JoinHandle")
            .field("exit", &"Receiver")
            .finish()
    }
}

/// Bounded worker pool that executes queued jobs.
#[derive(Debug)]
pub struct JobExecutor {
    queue: Arc<JobQueue>,
    tracker: Arc<JobTracker>,
    workers: Mutex<Vec<Worker>>,
    shutdown: AtomicBool,
    /// Longest time `shutdown` waits for a worker to end before detaching it.
    shutdown_grace: Duration,
    /// The energy and thermal state of the device (`POWER-001`).
    power: Arc<PowerMonitor>,
    /// Whether the pool is currently paused (`ROADMAP.md` M1.5).
    paused: Arc<AtomicBool>,
    /// The jobs that were interrupted by a pause.
    ///
    /// A job belongs here when a pause asked it to stop. The job then knows
    /// that it may be resumed, independently of the current pause state.
    pause_interrupted: Arc<Mutex<std::collections::HashSet<u64>>>,
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
        let shutdown_grace = Duration::from_millis(budgets.shutdown_grace_ms);
        let running_jobs = Arc::new(AtomicU64::new(0));
        let power = PowerMonitor::new(worker_count);
        let executor = Arc::new(Self {
            queue: Arc::new(JobQueue::new(
                budgets,
                Arc::clone(&power),
                Arc::clone(&running_jobs),
            )),
            tracker: Arc::new(JobTracker::default()),
            workers: Mutex::new(Vec::with_capacity(worker_count)),
            shutdown: AtomicBool::new(false),
            shutdown_grace,
            power,
            paused: Arc::new(AtomicBool::new(false)),
            pause_interrupted: Arc::new(Mutex::new(std::collections::HashSet::new())),
            running_jobs,
        });

        let mut handles = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let queue = Arc::clone(&executor.queue);
            let tracker = Arc::clone(&executor.tracker);
            let running_jobs = Arc::clone(&executor.running_jobs);
            let paused = Arc::clone(&executor.paused);
            let pause_interrupted = Arc::clone(&executor.pause_interrupted);
            let (exit_tx, exit_rx) = mpsc::channel();
            match std::thread::Builder::new()
                .name(format!("vocument-job-worker-{index}"))
                .spawn(move || {
                    worker_loop(queue, tracker, running_jobs, paused, pause_interrupted);
                    // The signal is sent even when the worker panics, because the
                    // sender is dropped with the closure.
                    let _ = exit_tx.send(());
                }) {
                Ok(handle) => handles.push(Worker {
                    join: handle,
                    exit: exit_rx,
                }),
                Err(error) => {
                    // Stop the workers that were already started, so that no
                    // orphaned thread keeps running without a pool owner.
                    let _ = executor.queue.shutdown();
                    for worker in handles {
                        let _ = worker.join.join();
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
    ///
    /// The job is registered in the tracker **before** it becomes visible to
    /// the workers. The order is not a matter of taste: a worker that takes a
    /// job the tracker does not know loses every state transition of that job,
    /// because the tracker answers "unknown job" — and the job then stays
    /// `Queued` for ever, although its work has been done.
    pub fn submit(
        &self,
        session_id: String,
        document_revision: u64,
        kind: super::scheduler::JobKind,
        resumable: bool,
        task: JobTask,
    ) -> Result<JobDescriptor, JobError> {
        let descriptor = self
            .queue
            .allocate(session_id, document_revision, kind, resumable)?;
        self.tracker.register(descriptor.clone())?;
        if let Err(error) = self.queue.publish(descriptor.clone(), task) {
            // The job never entered the queue and therefore never runs. The
            // tracker must not keep a job that can never reach a terminal
            // state.
            self.tracker.forget(descriptor.job_id);
            return Err(error);
        }
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

    /// Pauses the pool (`ROADMAP.md` M1.5).
    ///
    /// The workers stop taking new jobs. The running jobs are asked to stop as
    /// well, so that the device is not busy while the application is in the
    /// background. A job that was submitted as **resumable** is not lost: it
    /// goes back into the queue and runs again after the resume.
    ///
    /// Returns the number of jobs that were interrupted.
    pub fn pause(&self) -> usize {
        self.paused.store(true, Ordering::Release);
        // The jobs that are running right now are interrupted. They are
        // marked, so that a job knows afterwards that it was the pause that
        // stopped it and not the user.
        let interrupted = self.tracker.running_job_ids();
        let _ = self.tracker.cancel_all_running();
        if let Ok(mut guard) = self.pause_interrupted.lock() {
            for job_id in &interrupted {
                guard.insert(*job_id);
            }
        }
        self.queue.notify_all_workers();
        interrupted.len()
    }

    /// Resumes a paused pool (`ROADMAP.md` M1.5).
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        self.queue.notify_all_workers();
    }

    /// Returns whether the pool is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Reports the energy and thermal state of the device (`POWER-001`).
    ///
    /// The platform shell reports the state; the pool decides what it means.
    /// A state that suspends the background work wakes the waiting workers, so
    /// that a suspended job starts as soon as the device allows it again.
    pub fn report_power_state(&self, state: PowerState) -> PowerLimits {
        let limits = self.power.report(state);
        self.queue.notify_all_workers();
        limits
    }

    /// Returns the limits that the current power state implies.
    pub fn power_limits(&self) -> PowerLimits {
        self.power.limits()
    }

    /// Returns the number of jobs currently executing.
    pub fn running_count(&self) -> u64 {
        self.running_jobs.load(Ordering::Acquire)
    }

    /// Stops the pool: queued work is dropped, running work is asked to stop.
    ///
    /// The cancellation token of every running job is signalled first, so that a
    /// cooperative job stops at its next safe point. The shutdown then joins the
    /// worker threads, so that no worker outlives the pool.
    ///
    /// The wait is bounded by `shutdown_grace_ms`. A job body that ignores its
    /// cancellation token and never returns cannot be stopped from the outside;
    /// after the grace period the worker is detached instead of blocking the
    /// shutdown forever. The detached worker keeps its own references to the
    /// queue and the tracker and ends by itself as soon as the job body returns.
    ///
    /// A job body therefore still has to observe the token, and an engine call
    /// that cannot be interrupted has to be bounded by its own timeout. The grace
    /// period is only the last resort that keeps the application able to exit.
    pub fn shutdown(&self) {
        if self.shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = self.queue.shutdown();
        // Ask the running jobs to stop before joining the workers. Without this,
        // a running job would only be noticed when it finished on its own.
        let _ = self.tracker.cancel_all_running();
        let workers = match self.workers.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(poisoned) => {
                // A poisoned lock still owns the handles: recover them instead of
                // leaking the worker threads.
                let mut guard = poisoned.into_inner();
                std::mem::take(&mut *guard)
            }
        };
        // The wait is bounded. A worker that is stuck inside an engine call
        // cannot be stopped from the outside; joining it without a limit would
        // block the shutdown — and with it the exit of the whole application —
        // for as long as the engine call lasts.
        let deadline = Instant::now() + self.shutdown_grace;
        for worker in workers {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                // The grace period is over: let the worker go. It owns its own
                // references to the queue and the tracker, so it cleans itself up
                // as soon as the job body returns.
                drop(worker.join);
                continue;
            }
            match worker.exit.recv_timeout(remaining) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                    let _ = worker.join.join();
                }
                Err(RecvTimeoutError::Timeout) => {
                    drop(worker.join);
                }
            }
        }
    }
}

impl Drop for JobExecutor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Worker loop: pull a job, execute it, publish the terminal state.
fn worker_loop(
    queue: Arc<JobQueue>,
    tracker: Arc<JobTracker>,
    running_jobs: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    pause_interrupted: Arc<Mutex<std::collections::HashSet<u64>>>,
) {
    loop {
        // The worker waits for work that may run under the current power
        // limits. Only then it reserves a slot, so that a suspended job does
        // not occupy a slot.
        let next = loop {
            // A paused pool does not start new work. The wait is woken by the
            // resume, so it does not spin.
            if !queue.wait_until_resumed(&paused).unwrap_or(false) {
                return;
            }
            if !queue.wait_for_work().unwrap_or(false) {
                // The pool was shut down.
                return;
            }
            // The pause may have been requested while the worker was waiting.
            if paused.load(Ordering::Acquire) {
                continue;
            }
            // The slot is reserved under the lock of the queue, so that two
            // workers can never both pass the limit.
            //
            // A worker that finds no slot **waits** for one — it does not
            // leave the pool. A worker that left would shrink the pool with
            // every race for the last slot, and a job that waits in the queue
            // would never run: the pool would be empty while the queue is
            // full. (The pool is shut down in another way — the flag of the
            // queue ✗ — and then the wait returns false ✗.)
            while !queue.reserve_slot().unwrap_or(false) {
                if !queue.await_free_slot().unwrap_or(false) {
                    return;
                }
            }
            match queue.take_next_ready() {
                Ok(Some(job)) => break job,
                Ok(None) => {
                    // Another worker was faster. The reserved slot is released
                    // again and the worker starts over.
                    queue.release_slot();
                }
                Err(_) => {
                    queue.release_slot();
                    return;
                }
            }
        };
        let (descriptor, task) = next;

        if tracker
            .is_cancelled_before_start(descriptor.job_id)
            .unwrap_or(false)
        {
            // The job was cancelled while it was still queued.
            let _ = tracker.finish(descriptor.job_id, JobState::Cancelled, None);
            // The slot was reserved for this job and the job never runs: the
            // slot goes back to the pool. A slot that is not given back takes
            // a worker away from the pool for ever — the pool would serve its
            // queue with fewer and fewer workers, and a job that waits in the
            // queue would never run.
            queue.release_slot();
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

        let outcome = catch_unwind(AssertUnwindSafe(|| task(job_id, &token)));
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(_) => JobOutcome::Failed("job body panicked".to_owned()),
        };

        // The counter is decremented before the terminal state is published, so
        // that a job is never reported as "not running" while it still runs.
        running_jobs.fetch_sub(1, Ordering::AcqRel);

        // A job that was interrupted by a pause is put back into the queue
        // when it may be resumed. A job that the user cancelled stays
        // cancelled.
        let interrupted_by_pause = pause_interrupted
            .lock()
            .map(|mut guard| guard.remove(&job_id))
            .unwrap_or(false);
        if outcome == JobOutcome::Cancelled
            && descriptor.resumable
            && interrupted_by_pause
            && tracker.requeue(job_id).unwrap_or(false)
        {
            let _ = queue.requeue(descriptor.clone(), task);
            continue;
        }

        let (state, failure) = match outcome {
            JobOutcome::Completed => (JobState::Completed, None),
            JobOutcome::Failed(details) => (JobState::Failed, Some(details)),
            JobOutcome::Cancelled => (JobState::Cancelled, None),
        };
        let _ = tracker.finish(job_id, state, failure);
        tracker.detach_token(job_id);
        // A slot became free: a worker that waits for the power limits may
        // continue now.
        queue.notify_all_workers();
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
#[cfg(test)]
mod pool_regression_tests;
