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
    let Reply::Loaded(loaded) = next(&worker) else { panic!("not loaded") };
    assert_eq!((loaded.config.status.as_str(), loaded.profiles.unwrap().active.as_deref(), loaded.labels), ("Idle", Some("Attack"), Ok(vec![])));
    // The window is woken right after the reply is queued, so the wake can land a moment after the reply is read.
    wait_until(|| woken.load(Ordering::SeqCst) == 1);
    assert_eq!(fake.calls(), [("GET".into(), "/api/config".into()), ("GET".into(), "/api/profiles".into()), ("GET".into(), "/api/labels".into())]);
}

#[test]
fn load_failures_are_reported_and_a_stale_token_is_fatal() {
    let fake = FakeApi::idle();
    let (worker, _) = start(&fake, SLOW);
    fake.reply("/api/config", Err(ApiError::Rejected("Busy.".into())));
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

fn entry() -> neo_common::labels::Entry {
    neo_common::labels::Entry { label: "Sidestick button 1".into(), sources: vec!["s1".into()], mapping: "NEO 1 (Gun) \u{2192} Nothing".into() }
}

#[test]
fn load_reports_the_log_and_profiles_on_their_own() {
    let fake = FakeApi::idle();
    fake.reply("/api/labels", Err(ApiError::Rejected("Unreadable.".into())));
    fake.reply("/api/profiles", Err(ApiError::Unreachable));
    let (worker, _) = start(&fake, SLOW);
    worker.send(Request::Load);
    let Reply::Loaded(loaded) = next(&worker) else { panic!("not loaded") };
    assert_eq!((loaded.profiles, loaded.labels), (Err("The bridge is not answering.".into()), Err("Unreadable.".into())));
    fake.reply("/api/labels", Err(ApiError::Forbidden));
    worker.send(Request::Load);
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
}

#[test]
fn a_stale_token_on_the_profile_list_or_any_save_is_fatal() {
    for (path, request) in [("/api/profiles", Request::Load), ("PUT /api/config", Request::Record { config: Box::new(neo_daemon::config::defaults()), labels: vec![] }), ("PUT /api/labels", Request::Forget(vec![]))] {
        let fake = FakeApi::idle();
        fake.reply(path, Err(ApiError::Forbidden));
        let (worker, _) = start(&fake, SLOW);
        worker.send(request);
        assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
    }
}

#[test]
fn recording_saves_the_profile_then_the_log() {
    let fake = FakeApi::idle();
    let (worker, _) = start(&fake, SLOW);
    let config = Box::new(neo_daemon::config::defaults());
    fake.reply("PUT /api/config", Ok(json!({"saved": true, "running": true}).to_string()));
    worker.send(Request::Record { config: config.clone(), labels: vec![entry()] });
    assert_eq!(next(&worker), Reply::Recorded { config: config.clone(), running: true, log: Ok(vec![entry()]) });
    let bodies = fake.bodies();
    assert_eq!((bodies[0].0.as_str(), bodies[1].0.as_str()), ("/api/config", "/api/labels"));
    assert_eq!(serde_json::from_str::<neo_common::config::Config>(&bodies[0].1).unwrap(), *config);
    assert_eq!(serde_json::from_str::<Vec<neo_common::labels::Entry>>(&bodies[1].1).unwrap(), [entry()]);
    // The log fails on its own; the profile is still saved.
    fake.reply("PUT /api/labels", Err(ApiError::Rejected("Disk full.".into())));
    worker.send(Request::Record { config: config.clone(), labels: vec![] });
    assert_eq!(next(&worker), Reply::Recorded { config: config.clone(), running: true, log: Err("Disk full.".into()) });
    // The profile is refused: nothing is logged.
    fake.reply("PUT /api/config", Err(ApiError::Rejected("Invalid profile.".into())));
    worker.send(Request::Record { config: config.clone(), labels: vec![] });
    assert_eq!(next(&worker), Reply::NotSaved("Invalid profile.".into()));
    assert_eq!(fake.count("/api/labels"), 2);
    fake.reply("PUT /api/config", Ok(json!({"saved": true, "running": false}).to_string()));
    fake.reply("PUT /api/labels", Err(ApiError::Forbidden));
    worker.send(Request::Record { config, labels: vec![] });
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
}

#[test]
fn the_hat_mode_and_log_removals_are_saved() {
    let fake = FakeApi::idle();
    let (worker, _) = start(&fake, SLOW);
    let config = Box::new(neo_daemon::config::defaults());
    worker.send(Request::SetHat(config.clone()));
    assert_eq!(next(&worker), Reply::HatSet { config: config.clone(), running: false });
    worker.send(Request::Forget(vec![entry()]));
    assert_eq!(next(&worker), Reply::Forgot(Ok(vec![entry()])));
    fake.reply("PUT /api/labels", Err(ApiError::Unreachable));
    worker.send(Request::Forget(vec![]));
    assert_eq!(next(&worker), Reply::Forgot(Err("The bridge is not answering.".into())));
    fake.reply("PUT /api/config", Ok("{}".into()));
    worker.send(Request::SetHat(config));
    assert_eq!(next(&worker), Reply::NotSaved("The daemon sent a reply this app does not understand.".into()));
    fake.reply("PUT /api/config", Err(ApiError::Forbidden));
    worker.send(Request::SetHat(Box::new(neo_daemon::config::defaults())));
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
}
#[test]
fn settings_save_and_preview_through_the_daemon() {
    let fake = FakeApi::idle();
    fake.reply("PUT /api/preview", Ok(json!({"preview": true, "running": true}).to_string()));
    fake.reply("DELETE /api/preview", Ok(json!({"preview": false}).to_string()));
    let (worker, _) = start(&fake, SLOW);
    let config = Box::new(neo_daemon::config::defaults());
    worker.send(Request::Save(config.clone()));
    assert_eq!(next(&worker), Reply::Saved { config: config.clone(), running: false });
    worker.send(Request::Preview(config.clone()));
    assert_eq!(next(&worker), Reply::Previewed);
    worker.send(Request::EndPreview);
    assert_eq!(next(&worker), Reply::PreviewEnded);
    assert_eq!(fake.calls()[1..], [("PUT".into(), "/api/preview".into()), ("DELETE".into(), "/api/preview".into())]);
    fake.reply("PUT /api/config", Err(ApiError::Rejected("Invalid profile.".into())));
    fake.reply("PUT /api/preview", Err(ApiError::Rejected("Not emulating.".into())));
    fake.reply("DELETE /api/preview", Err(ApiError::Unreachable));
    worker.send(Request::Save(config.clone()));
    assert_eq!(next(&worker), Reply::SaveFailed("Invalid profile.".into()));
    worker.send(Request::Preview(config.clone()));
    assert_eq!(next(&worker), Reply::PreviewFailed("Not emulating.".into()));
    worker.send(Request::EndPreview);
    assert_eq!(next(&worker), Reply::PreviewEndFailed("The bridge is not answering.".into()));
    for (path, request) in [("PUT /api/config", Request::Save(config.clone())), ("PUT /api/preview", Request::Preview(config)), ("DELETE /api/preview", Request::EndPreview)] {
        let fake = FakeApi::idle();
        fake.reply(path, Err(ApiError::Forbidden));
        let (worker, _) = start(&fake, SLOW);
        worker.send(request);
        assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
    }
}
#[test]
fn profile_actions_report_what_happened() {
    let fake = FakeApi::idle();
    let (worker, _) = start(&fake, SLOW);
    let defaults = neo_daemon::config::defaults();
    let done = |running: bool| Ok(json!({"profiles": ["Cruise", "Dog fight"], "active": "Dog fight", "config": defaults, "running": running}).to_string());
    let listed = neo_common::api::Profiles { profiles: vec!["Cruise".into(), "Dog fight".into()], active: Some("Dog fight".into()) };
    fake.reply("POST /api/profiles/Dog%20fight/activate", done(true));
    worker.send(Request::Activate("Dog fight".into()));
    assert_eq!(next(&worker), Reply::Adopted { config: Box::new(defaults.clone()), profiles: Some(listed.clone()), text: "Switched to Dog fight. Applied to the running emulation.".into() });
    fake.reply("POST /api/profiles/Dog%20fight/activate", done(false));
    worker.send(Request::Activate("Dog fight".into()));
    assert!(matches!(next(&worker), Reply::Adopted { text, .. } if text == "Switched to Dog fight."));
    fake.reply("PUT /api/profiles/Dog%20fight", done(false));
    worker.send(Request::SaveAs("Dog fight".into()));
    assert_eq!(next(&worker), Reply::Listed { profiles: listed.clone(), text: "Saved as Dog fight.".into() });
    fake.reply("DELETE /api/profiles/Dog%20fight", done(false));
    worker.send(Request::DeleteProfile("Dog fight".into()));
    assert_eq!(next(&worker), Reply::Listed { profiles: listed, text: "Deleted Dog fight. The current settings are kept.".into() });
    assert_eq!(fake.bodies().last(), Some(&("/api/profiles/Dog%20fight".to_string(), "{}".to_string())));
    // Unknown names fail the same way the daemon says.
    worker.send(Request::Activate("x".into()));
    assert_eq!(next(&worker), Reply::ProfileFailed("Not switched: The bridge is not answering.".into()));
    worker.send(Request::SaveAs("x".into()));
    assert_eq!(next(&worker), Reply::ProfileFailed("Not saved: The bridge is not answering.".into()));
    worker.send(Request::DeleteProfile("x".into()));
    assert_eq!(next(&worker), Reply::ProfileFailed("Not deleted: The bridge is not answering.".into()));
    // A stale token ends the window whichever action meets it.
    for (key, request) in [("POST /api/profiles/x/activate", Request::Activate("x".into())), ("PUT /api/profiles/x", Request::SaveAs("x".into())), ("DELETE /api/profiles/x", Request::DeleteProfile("x".into()))] {
        let fake = FakeApi::idle();
        fake.reply(key, Err(ApiError::Forbidden));
        let (worker, _) = start(&fake, SLOW);
        worker.send(request);
        assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
    }
}

#[test]
fn an_import_becomes_the_saved_settings() {
    let fake = FakeApi::idle();
    let (worker, _) = start(&fake, SLOW);
    let defaults = neo_daemon::config::defaults();
    let profile = json!({"version": 1});
    worker.send(Request::Import { profile: profile.clone(), name: "old.json".into() });
    assert_eq!(next(&worker), Reply::Adopted { config: Box::new(defaults), profiles: None, text: "Imported old.json. Save it under a name to keep it as a profile.".into() });
    assert_eq!(fake.calls()[fake.calls().len() - 2..], [("PUT".into(), "/api/config".into()), ("GET".into(), "/api/config".into())]);
    fake.reply("PUT /api/config", Ok(json!({"saved": true, "running": true}).to_string()));
    worker.send(Request::Import { profile: profile.clone(), name: "old.json".into() });
    assert!(matches!(next(&worker), Reply::Adopted { text, .. } if text == "Imported old.json. Applied to the running emulation. Save it under a name to keep it as a profile."));
    fake.reply("/api/config", Err(ApiError::Rejected("Busy.".into())));
    worker.send(Request::Import { profile: profile.clone(), name: "old.json".into() });
    assert_eq!(next(&worker), Reply::ProfileFailed("Not imported: Busy.".into()));
    fake.reply("PUT /api/config", Err(ApiError::Rejected("Invalid profile: version.".into())));
    worker.send(Request::Import { profile: profile.clone(), name: "old.json".into() });
    assert_eq!(next(&worker), Reply::ProfileFailed("Not imported: Invalid profile: version.".into()));
    let (stale, _) = start(&fake, SLOW);
    fake.reply("PUT /api/config", Err(ApiError::Forbidden));
    stale.send(Request::Import { profile: profile.clone(), name: "old.json".into() });
    assert_eq!(next(&stale), Reply::Fatal(Fatal::Forbidden));
    fake.reply("PUT /api/config", Ok(json!({"saved": true, "running": false}).to_string()));
    fake.reply("/api/config", Err(ApiError::Forbidden));
    worker.send(Request::Import { profile, name: "old.json".into() });
    assert_eq!(next(&worker), Reply::Fatal(Fatal::Forbidden));
}

#[test]
fn profile_names_are_percent_encoded() {
    assert_eq!(neo_config::worker::encode("Dog fight_2-b/é"), "Dog%20fight_2-b%2F%C3%A9");
}