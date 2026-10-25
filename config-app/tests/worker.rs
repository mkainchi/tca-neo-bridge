//! The background worker: requests, live polling while visible, and when the daemon counts as gone.
mod common;
use common::{config_reply, live_reply, wait_until, FakeApi};
use neo_config::api::ApiError;
use neo_config::worker::{Fatal, Reply, Request, Worker, MISSES};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const SLOW: Duration = Duration::from_secs(3600);

fn start(fake: &FakeApi, period: Duration) -> (Worker, Arc<AtomicUsize>) {
    let woken = Arc::new(AtomicUsize::new(0));
    let counter = woken.clone();
    (Worker::start(Box::new(fake.clone()), period, Box::new(move || { counter.fetch_add(1, Ordering::SeqCst); })), woken)
}

fn next(worker: &Worker) -> Reply {
    worker.replies.recv_timeout(Duration::from_secs(10)).unwrap()
}

#[test]
fn load_reads_the_profile_and_the_saved_profile_names() {
    let fake = FakeApi::idle();
    fake.reply("/api/profiles", Ok(json!({"profiles": ["Attack"], "active": "Attack"}).to_string()));
    let (worker, woken) = start(&fake, SLOW);
    worker.send(Request::Load);
    let Reply::Loaded(config, profiles) = next(&worker) else { panic!("not loaded") };
    assert_eq!((config.status.as_str(), profiles.active.as_deref()), ("Idle", Some("Attack")));
    assert_eq!(woken.load(Ordering::SeqCst), 1);
    assert_eq!(fake.calls(), [("GET".into(), "/api/config".into()), ("GET".into(), "/api/profiles".into())]);
}

#[test]
fn load_failures_are_reported_and_a_stale_token_is_fatal() {
    let fake = FakeApi::idle();
    let (worker, _) = start(&fake, SLOW);
    fake.reply("/api/profiles", Err(ApiError::Rejected("Busy.".into())));
    worker.send(Request::Load);
    assert_eq!(next(&worker), Reply::Failed("Busy.".into()));
    fake.reply("/api/config", Err(ApiError::Unreachable));
    worker.send(Request::Load);
    assert_eq!(next(&worker), Reply::Failed("The bridge is not answering.".into()));
    fake.reply("/api/config", Ok("{\"config\": 1}".into()));
    worker.send(Request::Load);
    assert_eq!(next(&worker), Reply::Failed("The daemon sent a reply this app does not understand.".into()));
    fake.reply("/api/config", Err(ApiError::Forbidden));
    worker.send(Request::Load);
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
    // The thread has ended: later requests go nowhere.
    worker.send(Request::Load);
    drop(worker);
    assert_eq!(fake.count("/api/config"), 4);
}

#[test]
fn live_input_is_polled_while_visible() {
    let fake = FakeApi::idle();
    fake.reply("/api/live", Ok(live_reply(json!({"status": "Emulating", "monitor": common::monitor()}))));
    let (worker, _) = start(&fake, Duration::from_millis(5));
    let Reply::Live(live) = next(&worker) else { panic!("no live reply") };
    assert_eq!(live.status, "Emulating");
    assert!(live.monitor.is_some());
    // Hidden: no polls.
    worker.set_visible(false);
    while worker.replies.try_recv().is_ok() {}
    let before = fake.count("/api/live");
    std::thread::sleep(Duration::from_millis(50));
    assert!(fake.count("/api/live") <= before + 1);
    worker.set_visible(true);
    wait_until(|| fake.count("/api/live") > before + 2);
}

#[test]
fn a_few_missed_polls_are_forgiven_but_a_gone_daemon_is_fatal() {
    let fake = FakeApi::idle();
    {
        let mut script = fake.0.lock().unwrap();
        for _ in 1..MISSES {
            script.live.push_back(Err(ApiError::Unreachable));
        }
    }
    let (worker, _) = start(&fake, Duration::from_millis(1));
    assert!(matches!(next(&worker), Reply::Live(_)));
    fake.reply("/api/live", Err(ApiError::Rejected("?".into())));
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Lost));
    // The forgiven misses, at least one success, then MISSES failures in a row.
    assert!(fake.count("/api/live") >= MISSES as usize * 2);
}

#[test]
fn a_stale_token_on_a_poll_is_fatal() {
    let fake = FakeApi::idle();
    fake.reply("/api/live", Err(ApiError::Forbidden));
    let (worker, _) = start(&fake, Duration::from_millis(1));
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
}

#[test]
fn config_reply_fixture_parses() {
    assert!(serde_json::from_str::<neo_common::api::ConfigReply>(&config_reply(json!({"status": "Emulating"}))).is_ok());
}
