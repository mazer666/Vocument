//! Job scheduling primitives for the central `JobManager` (`ARCHITECTURE.md` §8, `JOB-001`).
//!
//! This module contains the **platform independent** scheduling model: job classes,
//! the typed state machine, the bounded queue and the priority/fairness rules. The
//! actual execution of job work happens in [`crate::jobs::executor`], which owns
//! the bounded worker pool.
//!
//! # Scheduling rules
//!
//! * A job belongs to exactly one [`JobClass`]. The class determines the base
//!   priority: interactive work (visible page rendering) is served before
//!   user-triggered work (OCR, import, export), which is served before background
//!   work (preprocessing, caching).
//! * Within one class, jobs are ordered by submission order (FIFO), so equal
//!   priority never depends on timing.
//! * **Fairness / anti-starvation:** a job that has been queued for longer than
//!   `starvation_promotion_ms` is promoted by one priority level. This guarantees
//!   that background work makes progress even while interactive work keeps
//!   arriving, without ever letting a background job jump ahead of an interactive
//!   one.
//! * The queue is bounded. Submitting beyond `max_queued_jobs` fails with a typed
//!   error instead of growing without limit.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::config::JobBudgets;

/// Canonical job priority levels (`ARCHITECTURE.md` §8).
/// Lower numeric discriminant means higher scheduling priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobPriority {
    VisiblePageRender = 1,
    ActiveSelectionDependency = 2,
    UserTriggeredSearch = 3,
    NearbyPagePrefetch = 4,
    Thumbnails = 5,
    BackgroundOcrIndex = 6,
    BatchOptimization = 7,
}

impl JobPriority {
    /// Returns the next lower priority level, used for starvation promotion.
    ///
    /// The highest priority is never demoted further, because promoting an
    /// interactive job would not help fairness.
    pub fn demoted(self) -> Self {
        match self {
            Self::VisiblePageRender => Self::VisiblePageRender,
            Self::ActiveSelectionDependency => Self::VisiblePageRender,
            Self::UserTriggeredSearch => Self::ActiveSelectionDependency,
            Self::NearbyPagePrefetch => Self::UserTriggeredSearch,
            Self::Thumbnails => Self::NearbyPagePrefetch,
            Self::BackgroundOcrIndex => Self::Thumbnails,
            Self::BatchOptimization => Self::BackgroundOcrIndex,
        }
    }

    /// Returns the numeric priority value (lower means more urgent).
    pub fn value(self) -> u8 {
        self as u8
    }
}

/// Job class, derived from the job kind (`ARCHITECTURE.md` §8.2).
///
/// The class is the unit the user actually perceives: an interactive action, a
/// user-triggered action, or background work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobClass {
    /// Work the user is waiting for right now (visible page rendering).
    Interactive,
    /// Work the user explicitly started (OCR, import, export).
    UserInitiated,
    /// Background work (preprocessing, caching, prefetching).
    Background,
}

impl JobClass {
    /// Returns the base priority of this class.
    pub fn base_priority(self) -> JobPriority {
        match self {
            Self::Interactive => JobPriority::VisiblePageRender,
            Self::UserInitiated => JobPriority::UserTriggeredSearch,
            Self::Background => JobPriority::BackgroundOcrIndex,
        }
    }
}

/// Kind of scheduled job (`ARCHITECTURE.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobKind {
    RenderVisibleTile,
    PrefetchNeighborTile,
    RenderThumbnail,
    IndexSearch,
    RunOcr,
    OptimizeDocument,
}

impl JobKind {
    /// Returns the class this job kind belongs to.
    pub fn class(self) -> JobClass {
        match self {
            Self::RenderVisibleTile | Self::IndexSearch => JobClass::Interactive,
            Self::RunOcr | Self::OptimizeDocument => JobClass::UserInitiated,
            Self::PrefetchNeighborTile | Self::RenderThumbnail => JobClass::Background,
        }
    }
}

/// Lifecycle state of a tracked job (`ARCHITECTURE.md` §8.3).
///
/// The state machine is strictly enforced: a job moves
/// `Queued -> Running -> Completed | Failed | Cancelled`. A job that is cancelled
/// while it is still queued never reaches `Running`; a job that is cancelled
/// while running ends as `Cancelled` and never as `Completed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    /// Returns true when the job reached a final state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        };
        formatter.write_str(label)
    }
}

/// Immutable metadata snapshot for a scheduled job (`ARCHITECTURE.md` §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDescriptor {
    pub job_id: u64,
    pub session_id: String,
    pub document_revision: u64,
    pub kind: JobKind,
    pub class: JobClass,
    pub priority: JobPriority,
    pub state: JobState,
    pub progress_percent: u8,
    pub resumable: bool,
    /// Monotonic submission sequence, used for FIFO ordering within a priority.
    pub sequence: u64,
}

/// Cooperative cancellation flag shared between the scheduler and the worker.
///
/// The flag is checked by the job body at safe points. Engines that cannot be
/// interrupted safely (MuPDF, Tesseract) are allowed to run to completion; the
/// result is then discarded instead of being applied (see
/// `ARCHITECTURE.md` §8.4).
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicU64>,
}

impl CancellationToken {
    /// Creates a fresh, non-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(1, Ordering::Release);
    }

    /// Returns true when cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) != 0
    }
}

/// Error type for job scheduling and state transitions.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobError {
    #[error("job '{job_id}' not found")]
    JobNotFound { job_id: u64 },
    #[error("job manager lock poisoned")]
    LockPoisoned,
    #[error("job queue is full: {queued} queued jobs, limit {limit}")]
    QueueFull { queued: usize, limit: usize },
    #[error("illegal job state transition for job '{job_id}': {from} -> {to}")]
    IllegalTransition {
        job_id: u64,
        from: JobState,
        to: JobState,
    },
    #[error("job '{job_id}' failed: {details}")]
    JobFailed { job_id: u64, details: String },
    #[error("job worker could not be started: {details}")]
    WorkerSpawnFailed { details: String },
}

/// Outcome of a completed job body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    /// The job finished successfully.
    Completed,
    /// The job failed with the given description.
    Failed(String),
    /// The job observed the cancellation token and stopped cooperatively.
    Cancelled,
}

/// A unit of work scheduled on the `JobManager`.
///
/// The closure receives the job id and a [`CancellationToken`]. It must return a
/// [`JobOutcome`] and must not panic; a panic inside the closure is caught and
/// converted into a `Failed` job, so that one faulty job cannot take down a
/// worker thread.
pub type JobTask = Box<dyn FnOnce(u64, &CancellationToken) -> JobOutcome + Send + 'static>;

/// Internal queue entry.
///
/// The entry holds a job closure, so it deliberately does not implement `Debug`.
struct QueueEntry {
    descriptor: JobDescriptor,
    task: JobTask,
    enqueued_at: Instant,
}

/// Shared, bounded job queue with priority ordering and anti-starvation.
pub struct JobQueue {
    inner: Mutex<QueueInner>,
    signal: Condvar,
    budgets: JobBudgets,
}

impl std::fmt::Debug for JobQueue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let queued = self.queued_len().unwrap_or(0);
        formatter
            .debug_struct("JobQueue")
            .field("queued", &queued)
            .field("max_queued_jobs", &self.budgets.max_queued_jobs)
            .finish()
    }
}

struct QueueInner {
    entries: VecDeque<QueueEntry>,
    next_sequence: u64,
    shutdown: bool,
}

impl JobQueue {
    /// Creates a queue with the given capacity limits.
    pub fn new(budgets: JobBudgets) -> Self {
        Self {
            inner: Mutex::new(QueueInner {
                entries: VecDeque::new(),
                next_sequence: 0,
                shutdown: false,
            }),
            signal: Condvar::new(),
            budgets,
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, QueueInner>, JobError> {
        self.inner.lock().map_err(|_| JobError::LockPoisoned)
    }

    /// Enqueues a job, applying the priority and fairness rules.
    pub fn enqueue(
        &self,
        session_id: String,
        document_revision: u64,
        kind: JobKind,
        resumable: bool,
        task: JobTask,
    ) -> Result<JobDescriptor, JobError> {
        let mut guard = self.lock()?;
        if guard.shutdown {
            return Err(JobError::JobFailed {
                job_id: 0,
                details: "job manager is shut down".to_owned(),
            });
        }
        if guard.entries.len() >= self.budgets.max_queued_jobs {
            return Err(JobError::QueueFull {
                queued: guard.entries.len(),
                limit: self.budgets.max_queued_jobs,
            });
        }

        let sequence = guard.next_sequence;
        guard.next_sequence += 1;

        let class = kind.class();
        let descriptor = JobDescriptor {
            job_id: sequence + 1,
            session_id,
            document_revision,
            kind,
            class,
            priority: class.base_priority(),
            state: JobState::Queued,
            progress_percent: 0,
            resumable,
            sequence,
        };

        guard.entries.push_back(QueueEntry {
            descriptor: descriptor.clone(),
            task,
            enqueued_at: Instant::now(),
        });
        drop(guard);
        self.signal.notify_one();
        Ok(descriptor)
    }

    /// Returns the next job to execute, applying priority and anti-starvation.
    ///
    /// Returns `None` when the queue is empty or the manager was shut down.
    pub fn take_next(&self) -> Result<Option<(JobDescriptor, JobTask)>, JobError> {
        let mut guard = self.lock()?;
        loop {
            if guard.shutdown {
                return Ok(None);
            }
            if guard.entries.is_empty() {
                let timeout = Duration::from_millis(self.budgets.idle_shutdown_timeout_ms);
                let (new_guard, _) = self
                    .signal
                    .wait_timeout(guard, timeout)
                    .map_err(|_| JobError::LockPoisoned)?;
                guard = new_guard;
                if guard.entries.is_empty() {
                    return Ok(None);
                }
                continue;
            }

            // Apply anti-starvation: a job that waited long enough is promoted by
            // one level before the ordering is computed.
            let now = Instant::now();
            let promotion = Duration::from_millis(self.budgets.starvation_promotion_ms);
            for entry in guard.entries.iter_mut() {
                if now.duration_since(entry.enqueued_at) >= promotion {
                    let promoted = entry.descriptor.priority.demoted();
                    entry.descriptor.priority = promoted;
                }
            }

            let selected = guard
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| {
                    (entry.descriptor.priority.value(), entry.descriptor.sequence)
                })
                .map(|(index, _)| index);

            if let Some(index) = selected {
                // The index was derived from the very same `entries` deque under
                // the same lock, so the removal cannot fail. It is handled as an
                // error nevertheless, so that a future change of the container
                // cannot turn into a panic inside a worker thread.
                let Some(entry) = guard.entries.remove(index) else {
                    return Err(JobError::LockPoisoned);
                };
                return Ok(Some((entry.descriptor, entry.task)));
            }
        }
    }

    /// Returns the number of currently queued jobs.
    pub fn queued_len(&self) -> Result<usize, JobError> {
        Ok(self.lock()?.entries.len())
    }

    /// Signals all waiting workers and prevents further submissions.
    pub fn shutdown(&self) -> Result<(), JobError> {
        let mut guard = self.lock()?;
        guard.shutdown = true;
        // Cancel queued work: the tasks are dropped, their cancellation token is
        // never handed out, so they can never run.
        guard.entries.clear();
        drop(guard);
        self.signal.notify_all();
        Ok(())
    }

    /// Returns true when the queue was shut down.
    pub fn is_shutdown(&self) -> Result<bool, JobError> {
        Ok(self.lock()?.shutdown)
    }
}

#[cfg(test)]
mod scheduler_tests;
