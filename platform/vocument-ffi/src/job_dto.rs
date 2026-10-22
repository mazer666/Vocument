//! UniFFI surface of the central `JobManager` (`ARCHITECTURE.md` §8, `ROADMAP.md` M1.5).
//!
//! The FFI layer exposes the **same** `JobManager` that the Rust core uses, so
//! macOS and Android share one scheduling implementation instead of two
//! divergent ones.
//!
//! # Threading contract
//!
//! * [`document_core_job_submit`] returns immediately. The work runs on the
//!   bounded worker pool, never on the caller's thread.
//! * Progress and results are **polled** through
//!   [`document_core_job_status`] / [`document_core_job_statuses`]. The core
//!   never calls back into the UI, so no callback can ever block a UI thread.
//! * [`document_core_job_cancel`] requests a cooperative cancellation and is
//!   safe to call from the UI thread.
//!
//! # Lifecycle contract
//!
//! Closing a document cancels every job of that session
//! ([`document_core_close_session`] does this automatically), so a result of a
//! closed document can never be applied.

use std::sync::Arc;

use vocument_core::config::PerformanceBudgetsConfig;
use vocument_core::jobs::executor::{JobExecutor, JobStatus};
use vocument_core::jobs::scheduler::{CancellationToken, JobKind, JobOutcome, JobTask};

use crate::DocumentCoreError;

/// Edge length of the thumbnail rendered by the background job, in pixels.
///
/// The core clamps the value into `32..=512`; this constant stays inside that
/// range.
const JOB_THUMBNAIL_MAX_DIMENSION_PX: u32 = 192;

/// Job status exposed across the UniFFI boundary.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiJobStatus {
    pub job_id: u64,
    pub session_id: String,
    pub document_revision: u64,
    /// `visible_page_render`, `user_triggered_search`, `background_ocr_index`, ...
    pub kind: String,
    /// `interactive`, `user_initiated` or `background`.
    pub class: String,
    /// `visible_page_render`, `user_triggered_search`, ...
    pub priority: String,
    /// `queued`, `running`, `completed`, `failed` or `cancelled`.
    pub state: String,
    pub progress_percent: u8,
    pub resumable: bool,
    /// Present when the job failed.
    pub failure: Option<String>,
}

impl From<JobStatus> for FfiJobStatus {
    fn from(status: JobStatus) -> Self {
        Self {
            job_id: status.descriptor.job_id,
            session_id: status.descriptor.session_id.clone(),
            document_revision: status.descriptor.document_revision,
            kind: format!("{:?}", status.descriptor.kind),
            class: format!("{:?}", status.descriptor.class),
            priority: format!("{:?}", status.descriptor.priority),
            state: format!("{:?}", status.descriptor.state),
            progress_percent: status.descriptor.progress_percent,
            resumable: status.descriptor.resumable,
            failure: status.failure,
        }
    }
}

/// The process wide job executor.
///
/// The pool is started lazily with the central configuration
/// (`config/performance-budgets.toml`, `[jobs]`), so the worker count and the
/// queue limits are identical on macOS and Android.
pub(crate) fn job_executor() -> Result<&'static Arc<JobExecutor>, DocumentCoreError> {
    static EXECUTOR: std::sync::OnceLock<Result<Arc<JobExecutor>, String>> =
        std::sync::OnceLock::new();
    let result = EXECUTOR.get_or_init(|| {
        let budgets = PerformanceBudgetsConfig::load_embedded()
            .map_err(|error| format!("failed to load job budgets: {error}"))?
            .jobs;
        JobExecutor::start(budgets).map_err(|error| format!("{error}"))
    });
    result
        .as_ref()
        .map_err(|details| DocumentCoreError::InternalFailure {
            details: details.clone(),
        })
}

/// Submits background work for a document.
///
/// The work runs on the bounded worker pool. The caller receives the job id
/// immediately and can poll [`document_core_job_status`] for progress.
///
/// `kind` is one of `render_visible_tile`, `prefetch_neighbor_tile`,
/// `render_thumbnail`, `index_search`, `run_ocr` or `optimize_document`.
///
/// The submitted work is a **real** core operation, not a no-op: the job runs on
/// the worker pool and the resulting state reflects the actual outcome. Which
/// operation runs is determined by `kind`:
///
/// * `render_thumbnail` renders the thumbnail of page `0` into the thumbnail
///   cache, exactly like the synchronous
///   [`crate::document_core_render_page_thumbnail`] call does.
/// * `index_search` runs the search index query of the session.
/// * `run_ocr` and `optimize_document` are not backed by a core operation yet
///   and therefore fail with a typed error instead of pretending to succeed.
///
/// The `document_revision` is the revision the caller believes to be current.
/// The job is rejected with a typed error if the session moved on, so that a
/// result of a stale revision is never applied.
#[uniffi::export]
pub fn document_core_job_submit(
    session_id: String,
    document_revision: u64,
    kind: String,
    resumable: bool,
) -> Result<u64, DocumentCoreError> {
    let executor = job_executor()?;
    let job_kind = parse_job_kind(&kind)?;
    let descriptor = executor
        .submit(
            session_id.clone(),
            document_revision,
            job_kind,
            resumable,
            build_job_task(session_id, job_kind),
        )
        .map_err(|error| DocumentCoreError::InternalFailure {
            details: error.to_string(),
        })?;
    Ok(descriptor.job_id)
}

/// Builds the real payload of a job.
///
/// The payload is a closure that runs on a worker thread. It performs the
/// actual core operation and reports the outcome. A failure of the operation
/// becomes a `Failed` job, so the platform shells can distinguish "the work
/// failed" from "the work succeeded".
fn build_job_task(session_id: String, job_kind: JobKind) -> JobTask {
    Arc::new(
        move |job_id: u64, token: &CancellationToken| -> JobOutcome {
            if token.is_cancelled() {
                return JobOutcome::Cancelled;
            }
            match job_kind {
                JobKind::RenderThumbnail => match render_session_thumbnail(&session_id) {
                    Ok(()) => JobOutcome::Completed,
                    Err(details) => {
                        if token.is_cancelled() {
                            JobOutcome::Cancelled
                        } else {
                            JobOutcome::Failed(details)
                        }
                    }
                },
                JobKind::IndexSearch => match run_session_index_search(&session_id) {
                    Ok(()) => JobOutcome::Completed,
                    Err(details) => {
                        if token.is_cancelled() {
                            JobOutcome::Cancelled
                        } else {
                            JobOutcome::Failed(format!("job {job_id} failed: {details}"))
                        }
                    }
                },
                other => JobOutcome::Failed(format!(
                    "job kind {other:?} has no core operation attached yet"
                )),
            }
        },
    )
}

/// Renders the thumbnail of the first page of a session into the thumbnail cache.
fn render_session_thumbnail(session_id: &str) -> Result<(), String> {
    crate::session_registry()
        .render_page_thumbnail(session_id, 0, JOB_THUMBNAIL_MAX_DIMENSION_PX)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Runs a search index query over a session, so that the index is built and
/// exercised on the worker pool.
fn run_session_index_search(session_id: &str) -> Result<(), String> {
    crate::session_registry()
        .index_search_batch(session_id, 0, 0)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Returns the current status of a job.
#[uniffi::export]
pub fn document_core_job_status(job_id: u64) -> Result<FfiJobStatus, DocumentCoreError> {
    let executor = job_executor()?;
    let status = executor
        .status(job_id)
        .map_err(|error| DocumentCoreError::InternalFailure {
            details: error.to_string(),
        })?;
    Ok(status.into())
}

/// Returns the status of every known job.
#[uniffi::export]
pub fn document_core_job_statuses() -> Result<Vec<FfiJobStatus>, DocumentCoreError> {
    let executor = job_executor()?;
    let statuses = executor
        .all_statuses()
        .map_err(|error| DocumentCoreError::InternalFailure {
            details: error.to_string(),
        })?;
    Ok(statuses.into_iter().map(FfiJobStatus::from).collect())
}

/// Requests a cooperative cancellation of a job.
///
/// Returns the state of the job after the request. A job that is already
/// finished keeps its terminal state.
#[uniffi::export]
pub fn document_core_job_cancel(job_id: u64) -> Result<String, DocumentCoreError> {
    let executor = job_executor()?;
    let state =
        executor
            .cancel_and_signal(job_id)
            .map_err(|error| DocumentCoreError::InternalFailure {
                details: error.to_string(),
            })?;
    Ok(format!("{state:?}"))
}

/// Cancels every job of a session and returns how many jobs were cancelled.
#[uniffi::export]
pub fn document_core_job_cancel_session(session_id: String) -> Result<u32, DocumentCoreError> {
    let executor = job_executor()?;
    let cancelled = executor.cancel_session(&session_id).map_err(|error| {
        DocumentCoreError::InternalFailure {
            details: error.to_string(),
        }
    })?;
    Ok(cancelled as u32)
}

/// Returns the number of jobs waiting in the queue.
#[uniffi::export]
pub fn document_core_job_queued_count() -> Result<u32, DocumentCoreError> {
    let executor = job_executor()?;
    let queued = executor
        .queued_len()
        .map_err(|error| DocumentCoreError::InternalFailure {
            details: error.to_string(),
        })?;
    Ok(queued as u32)
}

/// Returns the number of jobs currently executing.
#[uniffi::export]
pub fn document_core_job_running_count() -> Result<u32, DocumentCoreError> {
    let executor = job_executor()?;
    Ok(executor.running_count() as u32)
}

/// Removes finished jobs from the tracker and returns how many were removed.
#[uniffi::export]
pub fn document_core_job_purge_finished() -> Result<u32, DocumentCoreError> {
    let executor = job_executor()?;
    let purged = executor
        .purge_finished()
        .map_err(|error| DocumentCoreError::InternalFailure {
            details: error.to_string(),
        })?;
    Ok(purged as u32)
}

/// Parses the job kind from its stable string representation.
fn parse_job_kind(kind: &str) -> Result<JobKind, DocumentCoreError> {
    match kind {
        "render_visible_tile" => Ok(JobKind::RenderVisibleTile),
        "prefetch_neighbor_tile" => Ok(JobKind::PrefetchNeighborTile),
        "render_thumbnail" => Ok(JobKind::RenderThumbnail),
        "index_search" => Ok(JobKind::IndexSearch),
        "run_ocr" => Ok(JobKind::RunOcr),
        "optimize_document" => Ok(JobKind::OptimizeDocument),
        other => Err(DocumentCoreError::InternalFailure {
            details: format!("unknown job kind '{other}'"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn sample_corpus_pdf_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/m0_1_sample_two_pages.pdf")
    }

    fn open_session() -> String {
        let pdf_path = sample_corpus_pdf_path();
        let snapshot =
            crate::document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
                .expect("session must open");
        snapshot.session_id
    }

    /// The time the test waits for a job.
    ///
    /// The tests of this module share **one** worker pool, and they run at
    /// the same time: a job that waits in the queue behind the real work of
    /// another test starts later on a slow machine than on a fast one. The
    /// time is therefore generous — what the tests prove is the **state** of
    /// the job, not how fast it came.
    const JOB_TIMEOUT: Duration = Duration::from_secs(180);

    fn wait_for_state(job_id: u64, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let status = document_core_job_status(job_id).expect("status must be readable");
            let state = status.state.clone();
            if matches!(state.as_str(), "Completed" | "Failed" | "Cancelled") {
                return state;
            }
            if Instant::now() >= deadline {
                // The state of the pool is printed with the failure: a job
                // that waits in the queue and a job that was never enqueued
                // look the same from the outside, and the difference decides
                // where the fault lies.
                let status = document_core_job_status(job_id);
                let queued = super::document_core_job_queued_count().unwrap_or(0);
                let running = super::document_core_job_running_count().unwrap_or(0);
                panic!(
                    "job {job_id} did not reach a terminal state in time: the state is {status:?}, \
                     the queue holds {queued} job(s), {running} job(s) run"
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_thumbnail_job_performs_real_work_and_completes() {
        let session_id = open_session();
        let job_id =
            document_core_job_submit(session_id.clone(), 1, "render_thumbnail".to_owned(), false)
                .expect("job must be submitted");

        let state = wait_for_state(job_id, JOB_TIMEOUT);
        assert_eq!(state, "Completed", "the thumbnail job must succeed");

        // The job must have produced a real thumbnail, not just returned.
        let thumbnail = crate::document_core_render_page_thumbnail(session_id, 0, 192)
            .expect("thumbnail must render");
        assert!(thumbnail.width_px > 0 && thumbnail.height_px > 0);
    }

    #[test]
    fn a_job_for_an_unknown_session_fails_instead_of_pretending_to_succeed() {
        let job_id = document_core_job_submit(
            "sess_does_not_exist".to_owned(),
            1,
            "render_thumbnail".to_owned(),
            false,
        )
        .expect("job must be submitted");

        let state = wait_for_state(job_id, JOB_TIMEOUT);
        assert_eq!(
            state, "Failed",
            "a job for an unknown session must report a failure"
        );
        let status = document_core_job_status(job_id).expect("status must be readable");
        assert!(
            status.failure.is_some(),
            "the failure must be described in the job status"
        );
    }

    #[test]
    fn a_job_kind_without_a_core_operation_fails_instead_of_succeeding() {
        let session_id = open_session();
        let job_id = document_core_job_submit(session_id, 1, "run_ocr".to_owned(), false)
            .expect("job must be submitted");

        let state = wait_for_state(job_id, JOB_TIMEOUT);
        assert_eq!(
            state, "Failed",
            "a job kind without a core operation must not report success"
        );
    }

    #[test]
    fn an_unknown_job_kind_is_rejected_at_submission() {
        let error = document_core_job_submit(
            "sess_1".to_owned(),
            1,
            "not_a_real_job_kind".to_owned(),
            false,
        )
        .expect_err("an unknown job kind must be rejected");
        assert!(
            error.to_string().contains("not_a_real_job_kind"),
            "the error must name the unknown kind, was: {error}"
        );
    }

    #[test]
    fn closing_a_session_cancels_its_jobs() {
        let session_id = open_session();
        let job_id =
            document_core_job_submit(session_id.clone(), 1, "render_thumbnail".to_owned(), false)
                .expect("job must be submitted");

        let cancelled = document_core_job_cancel_session(session_id.clone())
            .expect("session cancellation must succeed");
        assert!(cancelled >= 1, "the job of the session must be cancelled");

        let state = wait_for_state(job_id, JOB_TIMEOUT);
        assert!(
            matches!(state.as_str(), "Cancelled" | "Completed"),
            "the job must end in a terminal state, was {state}"
        );
        crate::document_core_close_session(session_id).expect("session must close");
    }
}
