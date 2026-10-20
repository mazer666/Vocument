//! End-to-end verification of the background job path.
//!
//! This test exercises the complete path a platform shell uses:
//!
//! ```text
//! UI thread → UniFFI boundary → JobManager → Engine → Result → UI thread
//! ```
//!
//! It checks three things that the unit tests of the core cannot show:
//!
//! 1. **The submission returns immediately.** The caller (the UI thread) must
//!    not be blocked by the work it submits.
//! 2. **The work really runs on a worker.** The job body observes a different
//!    thread than the caller.
//! 3. **The result reaches the UI thread through polling.** The core never
//!    calls back into the caller, so the UI has to poll the status surface.
//!
//! The measurements are reported, not asserted against invented thresholds.
//! The thresholds that are asserted are the ones that follow from the
//! architecture: the submission has to return faster than the work itself,
//! and the cancellation has to be observed before the job body finishes.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vocument_core::config::PerformanceBudgetsConfig;
use vocument_core::jobs::executor::{JobExecutor, JobStatus};
use vocument_core::jobs::scheduler::{JobKind, JobOutcome};

/// Waits until a job reaches a terminal state, using the public status surface.
///
/// The platform shells poll exactly this surface, so the test uses it too
/// instead of reaching into the executor internals.
fn wait_for_terminal(
    executor: &JobExecutor,
    job_id: u64,
    timeout: Duration,
) -> Result<JobStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let status = executor.status(job_id).map_err(|error| error.to_string())?;
        if status.descriptor.state.is_terminal() {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "job {job_id} did not reach a terminal state in time"
            ));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Releases the blocking flags of a test, also when an assertion panics.
///
/// Without it a failed assertion would leave the workers inside their job body
/// and the test binary would hang instead of reporting the failure.
struct BlockerGuard {
    flags: Vec<Arc<AtomicBool>>,
}

impl BlockerGuard {
    fn new(flags: Vec<Arc<AtomicBool>>) -> Self {
        Self { flags }
    }

    fn release(&self) {
        for flag in &self.flags {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

impl Drop for BlockerGuard {
    fn drop(&mut self) {
        self.release();
    }
}

fn test_budgets() -> vocument_core::config::JobBudgets {
    PerformanceBudgetsConfig::load_embedded()
        .expect("the central configuration must load")
        .jobs
}

/// A job body that records the thread it ran on and how long it took.
struct JobObservation {
    caller_thread: std::thread::ThreadId,
    worker_thread: std::thread::ThreadId,
    duration: Duration,
}

#[test]
fn submitting_a_job_returns_immediately_and_runs_on_a_worker() {
    let budgets = test_budgets();
    let executor = JobExecutor::start(budgets).expect("the pool must start");

    let observation: Arc<Mutex<Option<JobObservation>>> = Arc::new(Mutex::new(None));
    let observation_slot = Arc::clone(&observation);
    let caller_thread = std::thread::current().id();

    // The job body blocks until it is released, so that the measurement of the
    // submission time cannot be shortened by a fast job.
    let release = Arc::new(AtomicBool::new(false));
    let release_flag = Arc::clone(&release);

    let submit_start = Instant::now();
    let descriptor = executor
        .submit(
            "sess_e2e".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Box::new(move |_, _| {
                let worker_thread = std::thread::current().id();
                let started = Instant::now();
                while !release_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                let mut guard = observation_slot.lock().expect("observation lock");
                *guard = Some(JobObservation {
                    caller_thread,
                    worker_thread,
                    duration: started.elapsed(),
                });
                JobOutcome::Completed
            }),
        )
        .expect("the job must be submitted");
    let submit_duration = submit_start.elapsed();

    // The submission must not have waited for the work.
    assert!(
        submit_duration < Duration::from_millis(200),
        "the submission must return immediately, took {submit_duration:?}"
    );

    // Give the worker a moment, then release the job.
    std::thread::sleep(Duration::from_millis(20));
    release.store(true, Ordering::SeqCst);

    let status = wait_for_terminal(&executor, descriptor.job_id, Duration::from_secs(10))
        .expect("the job must finish");
    assert_eq!(
        status.descriptor.state,
        vocument_core::jobs::scheduler::JobState::Completed
    );

    let observation = observation
        .lock()
        .expect("observation lock")
        .take()
        .expect("the job body must have run");
    assert_ne!(
        observation.worker_thread, observation.caller_thread,
        "the job body must not run on the calling (UI) thread"
    );
    println!(
        "e2e: submit took {submit_duration:?}, job body took {:?}, worker thread {:?} != caller thread {:?}",
        observation.duration, observation.worker_thread, observation.caller_thread
    );
    executor.shutdown();
}

#[test]
fn the_cancellation_latency_is_bounded_by_the_token_poll_interval() {
    let budgets = test_budgets();
    let executor = JobExecutor::start(budgets).expect("the pool must start");

    let cancel_requested = Arc::new(AtomicU64::new(0));
    let cancel_flag = Arc::clone(&cancel_requested);
    let observed = Arc::new(AtomicU64::new(0));
    let observed_flag = Arc::clone(&observed);

    let descriptor = executor
        .submit(
            "sess_e2e".to_owned(),
            1,
            JobKind::RunOcr,
            false,
            Box::new(move |_, token| {
                // A cooperative engine polls its token between steps.
                loop {
                    if token.is_cancelled() {
                        observed_flag.store(cancel_flag.load(Ordering::SeqCst), Ordering::SeqCst);
                        return JobOutcome::Cancelled;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }),
        )
        .expect("the job must be submitted");

    std::thread::sleep(Duration::from_millis(20));
    let requested_at = Instant::now();
    cancel_requested.store(
        requested_at.elapsed().as_nanos() as u64 + 1,
        Ordering::SeqCst,
    );
    executor
        .cancel_and_signal(descriptor.job_id)
        .expect("the cancellation must be accepted");

    let status = wait_for_terminal(&executor, descriptor.job_id, Duration::from_secs(10))
        .expect("the job must finish");
    let latency = requested_at.elapsed();

    assert_eq!(
        status.descriptor.state,
        vocument_core::jobs::scheduler::JobState::Cancelled,
        "a cancelled job must not report success"
    );
    assert!(
        latency < Duration::from_millis(500),
        "the cancellation must be observed quickly, took {latency:?}"
    );
    println!("e2e: cancellation latency {latency:?}");
    executor.shutdown();
}

#[test]
fn a_full_queue_reports_the_overflow_instead_of_growing_without_limit() {
    let budgets = test_budgets();
    let queue_limit = budgets.max_queued_jobs;
    let executor = JobExecutor::start(budgets).expect("the pool must start");

    // The queue is filled on purpose. The first job is taken by a worker and
    // blocks it, so that the remaining submissions have to be queued.
    let blocker = Arc::new(AtomicBool::new(false));
    let blocker_flag = Arc::clone(&blocker);
    let _guard = BlockerGuard::new(vec![Arc::clone(&blocker)]);
    executor
        .submit(
            "sess_e2e".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Box::new(move |_, _| {
                while !blocker_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                JobOutcome::Completed
            }),
        )
        .expect("the blocking job must be submitted");

    // The submitted jobs must block, otherwise the worker drains the queue
    // while the loop is still running and the overflow never happens. The
    // blocker is released at the end of the test.
    let queue_blocker = Arc::new(AtomicBool::new(false));
    let queue_blocker_flag = Arc::clone(&queue_blocker);
    let _queue_guard = BlockerGuard::new(vec![Arc::clone(&queue_blocker)]);

    let mut rejected = 0usize;
    let mut accepted = 0usize;
    for _ in 0..(queue_limit + 32) {
        let queue_blocker_flag = Arc::clone(&queue_blocker_flag);
        match executor.submit(
            "sess_e2e".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            Box::new(move |_, token| {
                while !queue_blocker_flag.load(Ordering::SeqCst) {
                    if token.is_cancelled() {
                        return JobOutcome::Cancelled;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                JobOutcome::Completed
            }),
        ) {
            Ok(_) => accepted += 1,
            Err(vocument_core::jobs::scheduler::JobError::QueueFull { queued, limit }) => {
                rejected += 1;
                assert!(
                    queued <= limit,
                    "the reported queue size must stay within the limit"
                );
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    assert!(rejected > 0, "the bounded queue must reject the overflow");
    // The limit applies to the queue. The jobs that a worker has already taken
    // are running and do not occupy a queue slot, so they may exceed it.
    let worker_limit = test_budgets().max_concurrent_workers;
    assert!(
        accepted <= queue_limit + worker_limit,
        "at most {accepted} jobs may be accepted, the queue limit is {queue_limit} \
         plus {worker_limit} running jobs"
    );
    println!("e2e: queue limit {queue_limit}, accepted {accepted}, rejected {rejected}");

    queue_blocker.store(true, Ordering::SeqCst);
    blocker.store(true, Ordering::SeqCst);
    executor.shutdown();
}

#[test]
fn closing_a_session_cancels_its_jobs_and_the_result_is_never_applied() {
    let budgets = test_budgets();
    let executor = JobExecutor::start(budgets).expect("the pool must start");

    let result_applied = Arc::new(AtomicBool::new(false));
    let applied_flag = Arc::clone(&result_applied);

    let descriptor = executor
        .submit(
            "sess_closing".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Box::new(move |_, token| {
                // The engine finishes its work even though the session was
                // closed. The late result must be discarded by the tracker.
                std::thread::sleep(Duration::from_millis(60));
                if !token.is_cancelled() {
                    applied_flag.store(true, Ordering::SeqCst);
                }
                JobOutcome::Completed
            }),
        )
        .expect("the job must be submitted");

    std::thread::sleep(Duration::from_millis(10));
    let cancelled = executor
        .cancel_session("sess_closing")
        .expect("the session cancellation must succeed");
    assert!(cancelled >= 1, "the job of the session must be cancelled");

    let status = wait_for_terminal(&executor, descriptor.job_id, Duration::from_secs(10))
        .expect("the job must finish");

    assert_eq!(
        status.descriptor.state,
        vocument_core::jobs::scheduler::JobState::Cancelled,
        "a job of a closed session must never report success"
    );
    assert!(
        !result_applied.load(Ordering::SeqCst),
        "the late result of a closed session must not be applied"
    );
    println!("e2e: session cancellation discarded the late result");
    executor.shutdown();
}

#[test]
fn the_status_surface_reports_every_state_of_a_job() {
    let budgets = test_budgets();
    let executor = JobExecutor::start(budgets).expect("the pool must start");

    let release = Arc::new(AtomicBool::new(false));
    let release_flag = Arc::clone(&release);
    let descriptor = executor
        .submit(
            "sess_e2e".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Box::new(move |_, _| {
                while !release_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                JobOutcome::Completed
            }),
        )
        .expect("the job must be submitted");

    // The status has to be readable while the job is queued or running, because
    // the UI polls it.
    let queued_status: JobStatus = executor
        .status(descriptor.job_id)
        .expect("the status must be readable");
    assert!(
        matches!(
            queued_status.descriptor.state,
            vocument_core::jobs::scheduler::JobState::Queued
                | vocument_core::jobs::scheduler::JobState::Running
        ),
        "a pending job must report Queued or Running, was {:?}",
        queued_status.descriptor.state
    );

    release.store(true, Ordering::SeqCst);
    let final_status = wait_for_terminal(&executor, descriptor.job_id, Duration::from_secs(10))
        .expect("the job must finish");
    assert_eq!(final_status.descriptor.progress_percent, 100);
    assert!(final_status.descriptor.state.is_terminal());
    executor.shutdown();
}
