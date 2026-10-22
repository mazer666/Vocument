//! Regression tests of the worker pool (`ARCHITECTURE.md` §8, `RISK-060`).
//!
//! The tests live in their own file so that the executor implementation stays
//! within the size limits of `CODE-001`.
//!
//! Every test in this file was written **against the defect**: it fails on
//! the code of the defect and passes on the code of the fix. The proof of
//! that is in `docs/audit/ci-gate-findings.md` §4.5.

use super::*;
use crate::jobs::scheduler::JobKind;
use std::sync::Arc;

use super::executor_tests::{BlockerGuard, test_budgets};

#[test]
fn a_job_cancelled_while_it_is_queued_does_not_take_a_worker_slot_away_from_the_pool() {
    let executor = JobExecutor::start(test_budgets(1, 16)).expect("executor must start");
    // The single worker is busy, so the next job waits in the queue.
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

    let queued = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            Arc::new(|_, _| JobOutcome::Completed),
        )
        .expect("submit");

    // The job is cancelled while it waits. The worker takes it out of the queue
    // afterwards and finds it cancelled: it must give the slot back, because
    // the slot belongs to the pool and not to the job.
    executor.cancel_and_signal(queued.job_id).expect("cancel");
    blocker_guard.release();

    wait_for_terminal(
        &executor.tracker,
        blocking_job.job_id,
        Duration::from_secs(5),
    )
    .expect("blocking job must finish");
    wait_for_terminal(&executor.tracker, queued.job_id, Duration::from_secs(5))
        .expect("cancelled job must finish");

    // The pool must still be able to run a job. A lost slot shrinks the pool
    // with every cancelled job, and the queue would wait forever.
    //
    // The job has the same class as the cancelled one, so that the queue
    // serves it after the cancelled job: the order inside a class is the
    // order of the submission.
    let afterwards = executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            Arc::new(|_, _| JobOutcome::Completed),
        )
        .expect("submit");
    let status = wait_for_terminal(&executor.tracker, afterwards.job_id, Duration::from_secs(5))
        .expect("the pool must still run jobs after a job was cancelled while queued");
    assert_eq!(status.descriptor.state, JobState::Completed);
    executor.shutdown();
}

#[test]
fn every_submitted_job_reaches_a_terminal_state_under_parallel_submission() {
    // The job manager of the application is used by several threads at the
    // same time. Every job that was submitted has to reach a terminal state,
    // and it has to do so quickly: the body of these jobs is trivial.
    //
    // Without the fix, a worker was able to take a job before the tracker
    // knew it: the state transitions of that job were lost, and it stayed
    // `Queued` for ever. Three of five runs failed.
    let executor = JobExecutor::start(test_budgets(4, 256)).expect("executor must start");
    const THREADS: usize = 4;
    const JOBS_PER_THREAD: usize = 50;

    let mut handles = Vec::with_capacity(THREADS);
    for _ in 0..THREADS {
        let executor = Arc::clone(&executor);
        handles.push(std::thread::spawn(move || {
            let mut ids = Vec::with_capacity(JOBS_PER_THREAD);
            for _ in 0..JOBS_PER_THREAD {
                let descriptor = executor
                    .submit(
                        "sess_1".to_owned(),
                        1,
                        JobKind::RenderVisibleTile,
                        false,
                        Arc::new(|_, _| JobOutcome::Completed),
                    )
                    .expect("submit");
                ids.push(descriptor.job_id);
            }
            ids
        }));
    }

    let mut job_ids = Vec::with_capacity(THREADS * JOBS_PER_THREAD);
    for handle in handles {
        job_ids.extend(handle.join().expect("the submitting thread must not panic"));
    }

    for job_id in job_ids {
        let status = wait_for_terminal(&executor.tracker, job_id, Duration::from_secs(30))
            .expect("every submitted job must reach a terminal state");
        assert_eq!(status.descriptor.state, JobState::Completed);
    }
    executor.shutdown();
}
