//! Central `JobManager` for prioritizing, scheduling, cancelling, and pausing/resuming work
//! (`ARCHITECTURE.md` §8, `JOB-001`, `ROADMAP.md` M1.5).
//!
//! The module is split into three parts:
//!
//! * [`scheduler`] — the platform independent scheduling model: job classes, the
//!   typed state machine, the bounded queue and the priority/fairness rules.
//! * [`executor`] — the bounded worker pool that actually runs the jobs off the
//!   calling thread.
//! * [`legacy`] — the original bookkeeping-only `JobManager` kept for API
//!   compatibility with existing call sites.
//!
//! # Priority order (`ARCHITECTURE.md` §8)
//! 1. `VisiblePageRender`
//! 2. `ActiveSelectionDependency`
//! 3. `UserTriggeredSearch`
//! 4. `NearbyPagePrefetch`
//! 5. `Thumbnails`
//! 6. `BackgroundOcrIndex`
//! 7. `BatchOptimization`
//!
//! # Worker model
//!
//! Work submitted through [`JobExecutor::submit`] runs on a bounded pool of
//! worker threads. The number of workers and the queue length come from the
//! central configuration (`config/performance-budgets.toml`, `[jobs]`), so the
//! same limits apply on macOS and on Android. No unbounded thread creation
//! happens anywhere.

pub mod executor;
pub mod legacy;
pub mod scheduler;

pub use executor::{JobExecutor, JobStatus, JobTracker, wait_for_terminal};
pub use legacy::{JobDescriptor, JobError, JobKind, JobManager, JobPriority, JobState};
pub use scheduler::{CancellationToken, JobClass, JobOutcome, JobQueue, JobTask};
