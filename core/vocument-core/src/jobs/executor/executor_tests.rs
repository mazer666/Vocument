//! Tests of the bounded worker pool (`ARCHITECTURE.md` §8, `ROADMAP.md` M1.5).
//!
//! The tests live in their own file so that the executor implementation stays
//! within the size limits of `CODE-001`.

use super::*;
use crate::jobs::scheduler::JobKind;
use std::sync::Arc;

/// Releases the blocker of a worker even when the test fails.
///
/// Without this guard a failing assertion would panic while a worker is still
/// blocked. The panic unwinds, the executor is dropped, and its `shutdown`
/// joins the blocked worker: the test run would hang instead of reporting the
/// failure. A test that hangs is far worse than a test that fails, because it
/// burns the whole CI timeout without naming the defect.
struct BlockerGuard {
    flag: Arc<std::sync::atomic::AtomicBool>,
}

impl BlockerGuard {
    fn new(flag: &Arc<std::sync::atomic::AtomicBool>) -> Self {
        Self {
            flag: Arc::clone(flag),
        }
    }

    fn release(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for BlockerGuard {
    fn drop(&mut self) {
        self.release();
    }
}

fn test_budgets(workers: usize, queued: usize) -> JobBudgets {
    JobBudgets {
        max_concurrent_render_workers: workers,
        max_concurrent_background_ocr_workers: 1,
        yield_background_on_interaction: true,
        max_concurrent_workers: workers,
        max_queued_jobs: queued,
        starvation_promotion_ms: 50,
        idle_shutdown_timeout_ms: 20,
        shutdown_grace_ms: 200,
    }
}

#[test]
fn submitted_job_runs_on_a_worker_and_completes() {
    let executor = JobExecutor::start(test_budgets(2, 16)).expect("executor must start");
    let descriptor = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(|_, _| JobOutcome::Completed),
        )
        .expect("submit");

    let status = wait_for_terminal(&executor.tracker, descriptor.job_id, Duration::from_secs(5))
        .expect("job must finish");
    assert_eq!(status.descriptor.state, JobState::Completed);
    assert_eq!(status.descriptor.progress_percent, 100);
    executor.shutdown();
}

#[test]
fn panicking_job_body_is_isolated_and_marked_failed() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let descriptor = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(|_, _| panic!("engine blew up")),
        )
        .expect("submit");

    let status = wait_for_terminal(&executor.tracker, descriptor.job_id, Duration::from_secs(5))
        .expect("job must finish");
    assert_eq!(status.descriptor.state, JobState::Failed);
    assert!(status.failure.is_some());

    // The worker survives the panic and can run further jobs.
    let second = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(|_, _| JobOutcome::Completed),
        )
        .expect("submit");
    let status = wait_for_terminal(&executor.tracker, second.job_id, Duration::from_secs(5))
        .expect("second job must finish");
    assert_eq!(status.descriptor.state, JobState::Completed);
    executor.shutdown();
}

#[test]
fn job_can_cancel_itself_cooperatively() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let descriptor = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RunOcr,
            false,
            Arc::new(|_, token| {
                token.cancel();
                JobOutcome::Cancelled
            }),
        )
        .expect("submit");

    let status = wait_for_terminal(&executor.tracker, descriptor.job_id, Duration::from_secs(5))
        .expect("job must finish");
    assert_eq!(status.descriptor.state, JobState::Cancelled);
    executor.shutdown();
}

#[test]
fn closing_a_session_cancels_its_running_jobs() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let descriptor = executor
        .submit(
            "sess_close".to_owned(),
            1,
            JobKind::RunOcr,
            false,
            Arc::new(|_, token| {
                // Long running work that checks the cancellation flag.
                for _ in 0..2000 {
                    if token.is_cancelled() {
                        return JobOutcome::Cancelled;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    // Give the worker a moment to actually start the job.
    std::thread::sleep(Duration::from_millis(20));
    let cancelled = executor
        .cancel_session("sess_close")
        .expect("cancel session");
    assert!(cancelled >= 1, "the running job must be cancelled");

    let status = wait_for_terminal(&executor.tracker, descriptor.job_id, Duration::from_secs(5))
        .expect("job must finish");
    assert_eq!(
        status.descriptor.state,
        JobState::Cancelled,
        "a job of a closed session must never report success"
    );
    executor.shutdown();
}

#[test]
fn queue_limit_is_enforced_under_concurrent_submission() {
    let executor = JobExecutor::start(test_budgets(1, 4)).expect("executor must start");
    let mut rejected = 0usize;
    let mut accepted = 0usize;
    for _ in 0..32 {
        match executor.submit(
            "sess_1".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            Arc::new(|_, _| {
                std::thread::sleep(Duration::from_millis(20));
                JobOutcome::Completed
            }),
        ) {
            Ok(_) => accepted += 1,
            Err(JobError::QueueFull { .. }) => rejected += 1,
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
    assert!(rejected > 0, "the bounded queue must reject overflow");
    assert!(accepted <= 5, "accepted jobs must stay within the bound");
    executor.shutdown();
}

#[test]
fn worker_count_is_bounded_and_jobs_run_concurrently() {
    let executor = JobExecutor::start(test_budgets(3, 32)).expect("executor must start");
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut ids = Vec::new();
    for _ in 0..3 {
        let counter = Arc::clone(&counter);
        let peak = Arc::clone(&peak);
        ids.push(
            executor
                .submit(
                    "sess_1".to_owned(),
                    1,
                    JobKind::RenderVisibleTile,
                    false,
                    Arc::new(move |_, _| {
                        let current = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                        peak.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                        JobOutcome::Completed
                    }),
                )
                .expect("submit"),
        );
    }

    for id in ids {
        wait_for_terminal(&executor.tracker, id.job_id, Duration::from_secs(5))
            .expect("job must finish");
    }
    let observed_peak = peak.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        observed_peak <= 3,
        "no more jobs may run than there are workers, observed {observed_peak}"
    );
    executor.shutdown();
}

#[test]
fn running_count_reflects_the_jobs_that_execute() {
    let executor = JobExecutor::start(test_budgets(2, 16)).expect("executor must start");
    assert_eq!(
        executor.running_count(),
        0,
        "an idle pool must not report running jobs"
    );

    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release_guard = BlockerGuard::new(&release);
    let mut ids = Vec::new();
    for _ in 0..2 {
        let release = Arc::clone(&release);
        ids.push(
            executor
                .submit(
                    "sess_1".to_owned(),
                    1,
                    JobKind::RenderVisibleTile,
                    false,
                    Arc::new(move |_, token| {
                        while !release.load(std::sync::atomic::Ordering::SeqCst) {
                            if token.is_cancelled() {
                                return JobOutcome::Cancelled;
                            }
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        JobOutcome::Completed
                    }),
                )
                .expect("submit"),
        );
    }

    // Wait until both jobs are actually executing.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while executor.running_count() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        executor.running_count(),
        2,
        "both jobs must be counted as running while they execute"
    );

    release_guard.release();
    for id in ids {
        wait_for_terminal(&executor.tracker, id.job_id, Duration::from_secs(5))
            .expect("job must finish");
    }
    assert_eq!(
        executor.running_count(),
        0,
        "finished jobs must not be counted as running anymore"
    );
    executor.shutdown();
}

#[test]
fn a_job_cancelled_before_it_starts_never_executes_its_body() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    // Block the single worker so that the next job stays queued.
    let blocker = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let blocker_flag = Arc::clone(&blocker);
    let blocker_guard = BlockerGuard::new(&blocker);
    let blocking_job = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(move |_, _| {
                while !blocker_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executed_flag = Arc::clone(&executed);
    let queued = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            Arc::new(move |_, _| {
                executed_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    executor.cancel_and_signal(queued.job_id).expect("cancel");
    blocker_guard.release();

    let status = wait_for_terminal(&executor.tracker, queued.job_id, Duration::from_secs(5))
        .expect("job must finish");
    assert_eq!(
        status.descriptor.state,
        JobState::Cancelled,
        "a job cancelled while queued must end as cancelled"
    );
    assert!(
        !executed.load(std::sync::atomic::Ordering::SeqCst),
        "the body of a job cancelled while queued must never run"
    );

    wait_for_terminal(
        &executor.tracker,
        blocking_job.job_id,
        Duration::from_secs(5),
    )
    .expect("blocking job must finish");
    executor.shutdown();
}

#[test]
fn a_cancelled_job_never_reports_success_even_if_the_body_completes() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started_flag = Arc::clone(&started);
    let descriptor = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(move |_, _| {
                started_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                // The engine cannot be interrupted: it finishes its work.
                std::thread::sleep(Duration::from_millis(80));
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !started.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    executor
        .cancel_and_signal(descriptor.job_id)
        .expect("cancel");

    let status = wait_for_terminal(&executor.tracker, descriptor.job_id, Duration::from_secs(5))
        .expect("job must finish");
    assert_eq!(
        status.descriptor.state,
        JobState::Cancelled,
        "a late result of a cancelled job must be discarded"
    );
    executor.shutdown();
}

#[test]
fn starting_and_stopping_the_pool_repeatedly_keeps_it_usable() {
    // The pool is started and stopped several times in one process, which
    // exercises the lifecycle handling of the worker handles.
    for round in 0..3 {
        let executor = JobExecutor::start(test_budgets(2, 8)).expect("executor must start");
        let descriptor = executor
            .submit(
                format!("sess_{round}"),
                1,
                JobKind::RenderVisibleTile,
                false,
                Arc::new(|_, _| JobOutcome::Completed),
            )
            .expect("submit");
        let status =
            wait_for_terminal(&executor.tracker, descriptor.job_id, Duration::from_secs(5))
                .expect("job must finish");
        assert_eq!(status.descriptor.state, JobState::Completed);
        executor.shutdown();
        // A second shutdown must be a no-op and must not panic.
        executor.shutdown();
    }
}
#[test]
fn shutting_the_pool_down_asks_the_running_jobs_to_stop() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let observed_cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed_flag = Arc::clone(&observed_cancellation);
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started_flag = Arc::clone(&started);
    // The job body stops when the token is signalled. The force flag is only
    // used to release the job if the shutdown failed, so that the test can
    // report the defect instead of blocking the whole test run.
    let force_release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let force_release_flag = Arc::clone(&force_release);

    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(move |_, token| {
                started_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                while !token.is_cancelled()
                    && !force_release_flag.load(std::sync::atomic::Ordering::SeqCst)
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                observed_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                JobOutcome::Cancelled
            }),
        )
        .expect("submit");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !started.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        started.load(std::sync::atomic::Ordering::SeqCst),
        "the job must have started"
    );

    // The shutdown runs in a separate thread, so that a shutdown that does not
    // ask the job to stop is reported as a failure instead of blocking the
    // whole test run.
    let shutdown_finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown_finished);
    let executor_for_shutdown = Arc::clone(&executor);
    let shutdown_thread = std::thread::spawn(move || {
        executor_for_shutdown.shutdown();
        shutdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !shutdown_finished.load(std::sync::atomic::Ordering::SeqCst)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    let shutdown_returned = shutdown_finished.load(std::sync::atomic::Ordering::SeqCst);

    // Release the job in any case, so that no thread is left behind.
    force_release.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = shutdown_thread.join();

    assert!(
        shutdown_returned,
        "the shutdown must return: it has to ask the running job to stop"
    );
    assert!(
        observed_cancellation.load(std::sync::atomic::Ordering::SeqCst),
        "the shutdown must ask the running job to stop"
    );
}

#[test]
fn a_job_that_ignores_the_cancellation_does_not_block_the_shutdown_forever() {
    // A job body that neither observes the token nor returns models an engine
    // call that cannot be interrupted. The shutdown must not wait for it
    // forever, because the application has to be able to exit.
    let mut budgets = test_budgets(1, 16);
    budgets.shutdown_grace_ms = 200;
    let executor = JobExecutor::start(budgets).expect("executor must start");

    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started_flag = Arc::clone(&started);
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release_flag = Arc::clone(&release);
    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(move |_, _| {
                started_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                // Deliberately ignores the cancellation token.
                while !release_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !started.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        started.load(std::sync::atomic::Ordering::SeqCst),
        "the job must have started"
    );

    let begin = std::time::Instant::now();
    executor.shutdown();
    let elapsed = begin.elapsed();
    // The job is released afterwards, so that no thread is left behind.
    release.store(true, std::sync::atomic::Ordering::SeqCst);

    assert!(
        elapsed < Duration::from_secs(5),
        "the shutdown must be bounded by the grace period, it took {elapsed:?}"
    );
}

#[test]
fn a_paused_pool_does_not_start_new_work_and_resumes_it_afterwards() {
    let executor = JobExecutor::start(test_budgets(2, 16)).expect("executor must start");
    let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ran_flag = Arc::clone(&ran);
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release_flag = Arc::clone(&release);
    let blocker_guard = BlockerGuard::new(&release);

    executor.pause();
    assert!(executor.is_paused(), "the pool must report the pause");

    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderThumbnail,
            false,
            Arc::new(move |_, _| {
                ran_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                while !release_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    std::thread::sleep(Duration::from_millis(150));
    assert!(
        !ran.load(std::sync::atomic::Ordering::SeqCst),
        "a paused pool must not start new work"
    );

    executor.resume();
    assert!(!executor.is_paused(), "the pool must report the resume");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !ran.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        ran.load(std::sync::atomic::Ordering::SeqCst),
        "the job must run after the resume"
    );

    drop(blocker_guard);
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    executor.shutdown();
}

#[test]
fn a_resumable_job_that_is_interrupted_by_a_pause_runs_again() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let attempts = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let attempts_flag = Arc::clone(&attempts);
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release_flag = Arc::clone(&release);
    let blocker_guard = BlockerGuard::new(&release);

    let descriptor = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RunOcr,
            true,
            Arc::new(move |_, token| {
                let attempt = attempts_flag.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if attempt == 1 {
                    // The first attempt is interrupted by the pause.
                    while !token.is_cancelled()
                        && !release_flag.load(std::sync::atomic::Ordering::SeqCst)
                    {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    return JobOutcome::Cancelled;
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");
    assert!(
        descriptor.resumable,
        "the job must be submitted as resumable"
    );

    // Wait until the job runs, then pause the pool: the job is interrupted.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while attempts.load(std::sync::atomic::Ordering::SeqCst) == 0
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the job must have started"
    );
    executor.pause();

    // The interrupted job goes back into the queue and runs again.
    executor.resume();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while attempts.load(std::sync::atomic::Ordering::SeqCst) < 2
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "the interrupted job must run again after the resume"
    );

    drop(blocker_guard);
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    executor.shutdown();
}

#[test]
fn a_job_that_was_cancelled_by_the_user_is_not_revived_by_a_pause() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let attempts = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let attempts_flag = Arc::clone(&attempts);
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let release_flag = Arc::clone(&release);
    let blocker_guard = BlockerGuard::new(&release);

    let descriptor = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RunOcr,
            true,
            Arc::new(move |_, token| {
                attempts_flag.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                while !token.is_cancelled()
                    && !release_flag.load(std::sync::atomic::Ordering::SeqCst)
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Cancelled
            }),
        )
        .expect("submit");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while attempts.load(std::sync::atomic::Ordering::SeqCst) == 0
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    // The user cancels the job and pauses the pool afterwards.
    executor
        .cancel_and_signal(descriptor.job_id)
        .expect("cancel");
    executor.pause();
    executor.resume();

    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        attempts.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "a job that the user cancelled must not be revived by a pause"
    );

    drop(blocker_guard);
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    executor.shutdown();
}

#[test]
fn the_queue_reports_the_number_of_waiting_jobs() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    let blocker = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let blocker_flag = Arc::clone(&blocker);
    let blocker_guard = BlockerGuard::new(&blocker);
    // The flag is set as soon as the worker picked the blocking job up, so
    // that the measurement does not depend on a timing guess.
    let blocker_running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let blocker_running_flag = Arc::clone(&blocker_running);
    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(move |_, _| {
                blocker_running_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                while !blocker_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    for _ in 0..3 {
        executor
            .submit(
                "sess_1".to_owned(),
                1,
                JobKind::PrefetchNeighborTile,
                false,
                Arc::new(|_, _| JobOutcome::Completed),
            )
            .expect("submit");
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !blocker_running.load(std::sync::atomic::Ordering::SeqCst)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        blocker_running.load(std::sync::atomic::Ordering::SeqCst),
        "the blocking job must have been picked up by the worker"
    );
    assert_eq!(
        executor.queued_len().expect("queue length"),
        3,
        "the queue must report the waiting jobs"
    );
    blocker_guard.release();
    executor.shutdown();
}
