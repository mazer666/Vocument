//! Tests of the scheduling model (`ARCHITECTURE.md` §8, `ROADMAP.md` M1.5).
//!
//! The tests live in their own file so that the scheduler implementation stays
//! within the size limits of `CODE-001`.

use super::*;
use crate::config::JobBudgets;

fn budgets(max_queued: usize, promotion_ms: u64) -> JobBudgets {
    JobBudgets {
        max_concurrent_render_workers: 2,
        max_concurrent_background_ocr_workers: 1,
        yield_background_on_interaction: true,
        max_concurrent_workers: 2,
        max_queued_jobs: max_queued,
        starvation_promotion_ms: promotion_ms,
        idle_shutdown_timeout_ms: 50,
        shutdown_grace_ms: 200,
    }
}

fn noop_task() -> JobTask {
    Box::new(|_, _| JobOutcome::Completed)
}

#[test]
fn interactive_work_is_served_before_background_work() {
    let queue = JobQueue::new(budgets(16, 60_000));
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            noop_task(),
        )
        .expect("enqueue background");
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect("enqueue interactive");

    let (first, _) = queue.take_next().expect("take").expect("job");
    assert_eq!(first.kind, JobKind::RenderVisibleTile);
    assert_eq!(first.class, JobClass::Interactive);
}

#[test]
fn equal_priority_is_served_first_in_first_out() {
    let queue = JobQueue::new(budgets(16, 60_000));
    let first = queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect("enqueue");
    let second = queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect("enqueue");

    let (taken, _) = queue.take_next().expect("take").expect("job");
    assert_eq!(taken.job_id, first.job_id);
    let (taken, _) = queue.take_next().expect("take").expect("job");
    assert_eq!(taken.job_id, second.job_id);
}

#[test]
fn queue_is_bounded_and_rejects_overflow() {
    let queue = JobQueue::new(budgets(2, 60_000));
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect("enqueue 1");
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect("enqueue 2");

    let error = queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect_err("queue must be bounded");
    assert_eq!(
        error,
        JobError::QueueFull {
            queued: 2,
            limit: 2
        }
    );
}

#[test]
fn starved_background_job_is_promoted_over_newer_background_jobs() {
    let queue = JobQueue::new(budgets(16, 0));
    // The first job is enqueued with the background class, the second one
    // later. With promotion enabled the older job wins.
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::PrefetchNeighborTile,
            false,
            noop_task(),
        )
        .expect("enqueue older");
    std::thread::sleep(Duration::from_millis(5));
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderThumbnail,
            false,
            noop_task(),
        )
        .expect("enqueue newer");

    let (taken, _) = queue.take_next().expect("take").expect("job");
    assert_eq!(
        taken.kind,
        JobKind::PrefetchNeighborTile,
        "the older job must be promoted to prevent starvation"
    );
}

#[test]
fn shutdown_prevents_further_submissions_and_drops_queued_work() {
    let queue = JobQueue::new(budgets(16, 60_000));
    queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect("enqueue");
    queue.shutdown().expect("shutdown");

    assert!(queue.is_shutdown().expect("state"));
    assert_eq!(queue.queued_len().expect("len"), 0);
    let error = queue
        .enqueue(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            noop_task(),
        )
        .expect_err("must reject after shutdown");
    assert!(matches!(error, JobError::JobFailed { .. }));
}

#[test]
fn cancellation_token_reports_requested_state() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
    token.cancel();
    assert!(token.is_cancelled());
}
