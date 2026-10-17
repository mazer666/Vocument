//! Central `JobManager` for prioritizing, scheduling, cancelling, and pausing/resuming work (`ARCHITECTURE.md` §8, `JOB-001`).
//!
//! Priority order (`ARCHITECTURE.md` §8):
//! 1. `VisiblePageRender`
//! 2. `ActiveSelectionDependency`
//! 3. `UserTriggeredSearch`
//! 4. `NearbyPagePrefetch`
//! 5. `Thumbnails`
//! 6. `BackgroundOcrIndex`
//! 7. `BatchOptimization`

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

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

/// Kind of scheduled job (`ARCHITECTURE.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    RenderVisibleTile,
    PrefetchNeighborTile,
    RenderThumbnail,
    IndexSearch,
    RunOcr,
    OptimizeDocument,
}

/// Lifecycle state of a tracked job (`ARCHITECTURE.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

/// Immutable metadata snapshot for a scheduled job (`ARCHITECTURE.md` §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDescriptor {
    pub job_id: u64,
    pub session_id: String,
    pub document_revision: u64,
    pub kind: JobKind,
    pub priority: JobPriority,
    pub state: JobState,
    pub progress_percent: u8,
    pub resumable: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobError {
    #[error("job '{job_id}' not found")]
    JobNotFound { job_id: u64 },
    #[error("job manager lock poisoned")]
    LockPoisoned,
}

/// Central job scheduler and tracker (`ARCHITECTURE.md` §8, `JOB-001`).
#[derive(Debug)]
pub struct JobManager {
    next_job_id: AtomicU64,
    yield_background_on_interaction: bool,
    jobs: Mutex<BTreeMap<u64, JobDescriptor>>,
}

impl JobManager {
    pub fn new(yield_background_on_interaction: bool) -> Self {
        Self {
            next_job_id: AtomicU64::new(1),
            yield_background_on_interaction,
            jobs: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn yield_background_on_interaction(&self) -> bool {
        self.yield_background_on_interaction
    }

    /// Registers a new job in `Queued` state and returns its descriptor.
    pub fn enqueue_job(
        &self,
        session_id: impl Into<String>,
        document_revision: u64,
        kind: JobKind,
        priority: JobPriority,
        resumable: bool,
    ) -> Result<JobDescriptor, JobError> {
        let job_id = self.next_job_id.fetch_add(1, Ordering::Relaxed);
        let descriptor = JobDescriptor {
            job_id,
            session_id: session_id.into(),
            document_revision,
            kind,
            priority,
            state: JobState::Queued,
            progress_percent: 0,
            resumable,
        };
        let mut guard = self.jobs.lock().map_err(|_| JobError::LockPoisoned)?;
        guard.insert(job_id, descriptor.clone());
        Ok(descriptor)
    }

    /// Transitions a job to `Running`.
    pub fn mark_running(&self, job_id: u64) -> Result<JobDescriptor, JobError> {
        self.update_state(job_id, JobState::Running, 10)
    }

    /// Transitions a job to `Completed` (`100%`).
    pub fn mark_completed(&self, job_id: u64) -> Result<JobDescriptor, JobError> {
        self.update_state(job_id, JobState::Completed, 100)
    }

    /// Explicitly cancels a queued or running job (`ARCHITECTURE.md` §5, §8).
    pub fn cancel_job(&self, job_id: u64) -> Result<JobDescriptor, JobError> {
        self.update_state(job_id, JobState::Cancelled, 0)
    }

    /// Cancels all queued prefetch jobs for a session that are no longer needed after a viewport jump.
    pub fn cancel_queued_prefetch_for_session(&self, session_id: &str) -> Result<usize, JobError> {
        let mut guard = self.jobs.lock().map_err(|_| JobError::LockPoisoned)?;
        let mut count = 0;
        for job in guard.values_mut() {
            if job.session_id == session_id
                && job.priority == JobPriority::NearbyPagePrefetch
                && job.state == JobState::Queued
            {
                job.state = JobState::Cancelled;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Returns queued jobs for a session ordered strictly by priority (highest priority first, then FIFO `job_id`).
    pub fn pending_jobs_ordered(&self, session_id: &str) -> Result<Vec<JobDescriptor>, JobError> {
        let guard = self.jobs.lock().map_err(|_| JobError::LockPoisoned)?;
        let mut pending: Vec<JobDescriptor> = guard
            .values()
            .filter(|j| j.session_id == session_id && j.state == JobState::Queued)
            .cloned()
            .collect();
        pending.sort_by_key(|j| (j.priority, j.job_id));
        Ok(pending)
    }

    fn update_state(
        &self,
        job_id: u64,
        new_state: JobState,
        progress_percent: u8,
    ) -> Result<JobDescriptor, JobError> {
        let mut guard = self.jobs.lock().map_err(|_| JobError::LockPoisoned)?;
        let job = guard
            .get_mut(&job_id)
            .ok_or(JobError::JobNotFound { job_id })?;
        job.state = new_state;
        if new_state != JobState::Cancelled {
            job.progress_percent = progress_percent;
        }
        Ok(job.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_visible_render_ahead_of_prefetch_and_supports_cancellation() {
        let manager = JobManager::new(true);
        let prefetch = manager
            .enqueue_job(
                "sess_1",
                1,
                JobKind::PrefetchNeighborTile,
                JobPriority::NearbyPagePrefetch,
                false,
            )
            .expect("enqueue prefetch");
        let visible = manager
            .enqueue_job(
                "sess_1",
                1,
                JobKind::RenderVisibleTile,
                JobPriority::VisiblePageRender,
                false,
            )
            .expect("enqueue visible");

        let ordered = manager.pending_jobs_ordered("sess_1").expect("ordered");
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].job_id, visible.job_id);
        assert_eq!(ordered[1].job_id, prefetch.job_id);

        let cancelled = manager
            .cancel_job(prefetch.job_id)
            .expect("cancel prefetch");
        assert_eq!(cancelled.state, JobState::Cancelled);

        let remaining = manager.pending_jobs_ordered("sess_1").expect("remaining");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].job_id, visible.job_id);
    }
}
