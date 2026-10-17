use neo_daemon::config;
use neo_daemon::http::{self, Request};
use neo_daemon::labels::{self, Entry};
use neo_daemon::shared::Shared;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-labels-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn entry(label: &str, sources: &[&str]) -> Entry {
    Entry { label: label.into(), sources: sources.iter().map(|s| s.to_string()).collect(), mapping: "NEO 4".into() }
}

#[test]
fn sources_include_buttons_and_hat_directions_only() {
    for good in ["s1", "q31", "hat-up", "hat-down-left"] {
        assert!(labels::valid_source(good), "{good}");
    }
    for bad in ["s0", "hat-", "hat-sideways", "hat", "x"] {
        assert!(!labels::valid_source(bad), "{bad}");
    }
}

#[test]
fn parse_validates_every_field() {
    let ok = serde_json::to_string(&vec![entry("5", &["s12"]), entry("Hat up", &["hat-up", "s1"])]).unwrap();
    assert_eq!(labels::parse(&ok).unwrap().len(), 2);
    let bad = |entries: Vec<Entry>| labels::parse(&serde_json::to_string(&entries).unwrap()).unwrap_err();
    assert!(bad(vec![entry("  ", &["s1"])]).contains("1 to 60"));
    assert!(bad(vec![entry(&"x".repeat(61), &["s1"])]).contains("1 to 60"));
    assert!(bad(vec![entry("a", &[])]).contains("detected"));
    assert!(bad(vec![entry("a", &["s1"; 9])]).contains("detected"));
    assert!(bad(vec![entry("a", &["zz"])]).contains("detected"));
    let mut long = entry("a", &["s1"]);
    long.mapping = "m".repeat(201);
    assert!(bad(vec![long]).contains("too long"));
    assert!(bad(vec![entry("a", &["s1"]); 201]).contains("At most"));
    assert!(labels::parse("{").unwrap_err().starts_with("Invalid recordings"));
    assert!(labels::parse(r#"[{"label":"a","sources":["s1"],"mapping":"","extra":1}]"#).is_err());
}

#[test]
fn load_and_save_round_trip_and_failures() {
    let dir = temp("store");
    let path = dir.join("label-map.json");
    assert!(labels::load(&path).unwrap().is_empty());
    labels::save(&path, &[entry("5", &["s12"])]).unwrap();
    assert_eq!(labels::load(&path).unwrap(), [entry("5", &["s12"])]);
    assert!(!dir.join("label-map.tmp").exists());
    assert!(labels::save(&dir.join("missing").join("x.json"), &[]).unwrap_err().contains("could not be saved"));
    assert_eq!(labels::load(&dir).unwrap_err(), "The recordings could not be read.");
}

#[test]
fn api_reads_and_replaces_recordings_next_to_the_profile() {
    let dir = temp("api");
    let shared = Shared::new(dir.join("config.json"), config::defaults());
    assert_eq!(shared.labels_path(), dir.join("label-map.json"));
    let call = |method: &str, body: &str| {
        let request = Request { method: method.into(), path: "/api/labels".into(), host: "127.0.0.1:1".into(), token: "t".into(), body: body.into() };
        let response = http::handle(&request, &shared, 1, "t");
        (response.status, serde_json::from_str::<serde_json::Value>(&response.body).unwrap())
    };
    assert_eq!(call("GET", ""), (200, serde_json::json!({"labels": []})));
    let body = serde_json::to_string(&vec![entry("5", &["s12"])]).unwrap();
    assert_eq!(call("PUT", &body), (200, serde_json::json!({"saved": 1})));
    assert_eq!(call("GET", "").1["labels"][0]["label"], "5");
    let (status, error) = call("PUT", "[]x");
    assert_eq!(status, 400);
    assert!(error["error"].as_str().unwrap().starts_with("Invalid recordings"));
    std::fs::write(dir.join("label-map.json"), "nope").unwrap();
    assert_eq!(call("GET", "").0, 400);
}
