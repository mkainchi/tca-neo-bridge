use neo_daemon::config;
use neo_daemon::driver::Notice;
use neo_daemon::profiles;
use neo_daemon::shared::Shared;
use std::path::PathBuf;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-profiles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn names_are_plain_and_cannot_leave_the_folder() {
    for good in ["Dogfight", "Ground attack", "a", "x_1-2", &"n".repeat(40)] {
        assert!(profiles::valid(good), "{good}");
    }
    for bad in ["", " lead", "trail ", "../up", "a/b", "a\\b", "c:", "dot.json", "é", &"n".repeat(41)] {
        assert!(!profiles::valid(bad), "{bad}");
    }
}

#[test]
fn save_list_load_delete_and_cycle() {
    let dir = temp("crud").join("profiles");
    assert!(profiles::list(&dir).is_empty()); // no folder yet
    assert_eq!(profiles::next(&dir, None), None);
    let mut dogfight = config::defaults();
    dogfight.axes.roll.curve = 0.3;
    profiles::save(&dir, "dogfight", &dogfight).unwrap();
    profiles::save(&dir, "Attack", &config::defaults()).unwrap();
    profiles::save(&dir, "Cruise", &config::defaults()).unwrap();
    std::fs::write(dir.join("not a profile.txt"), "x").unwrap();
    {
        // A file name that is not valid Unicode (an unpaired surrogate) is skipped.
        use std::os::windows::ffi::OsStringExt;
        let odd = std::ffi::OsString::from_wide(&[0xD800, u16::from(b'.'), u16::from(b'j'), u16::from(b's'), u16::from(b'o'), u16::from(b'n')]);
        std::fs::write(dir.join(odd), "{}").unwrap();
    }
    std::fs::write(dir.join("bad..name.json"), "{}").unwrap();
    assert_eq!(profiles::list(&dir), ["Attack", "Cruise", "dogfight"]);
    assert_eq!(profiles::load(&dir, "dogfight").unwrap(), dogfight);
    assert!(profiles::load(&dir, "Missing").unwrap_err().contains("no profile named Missing"));
    assert!(profiles::load(&dir, "../x").unwrap_err().contains("Profile names"));
    std::fs::write(dir.join("Broken.json"), "{").unwrap();
    assert!(profiles::load(&dir, "Broken").unwrap_err().starts_with("Invalid profile"));
    std::fs::remove_file(dir.join("Broken.json")).unwrap();
    let mut invalid = config::defaults();
    invalid.camera_strength = 3.0;
    assert!(profiles::save(&dir, "Bad", &invalid).unwrap_err().contains("Camera strength"));
    assert!(profiles::save(&dir, "a/b", &config::defaults()).is_err());
    // Cycling wraps round, and starts at the first one when the active profile is unknown.
    assert_eq!([profiles::next(&dir, None), profiles::next(&dir, Some("Cruise")), profiles::next(&dir, Some("dogfight")), profiles::next(&dir, Some("Gone"))],
        [Some("Attack".into()), Some("dogfight".into()), Some("Attack".into()), Some("Attack".into())]);
    profiles::delete(&dir, "Cruise").unwrap();
    assert!(profiles::delete(&dir, "Cruise").unwrap_err().contains("no profile named Cruise"));
    assert!(profiles::delete(&dir, "").is_err());
    assert_eq!(profiles::list(&dir), ["Attack", "dogfight"]);
    // A file where the folder should be.
    let blocked = temp("blocked").join("profiles");
    std::fs::write(&blocked, "file").unwrap();
    assert!(profiles::save(&blocked, "X", &config::defaults()).unwrap_err().contains("folder could not be created"));
    assert!(profiles::set_active(&blocked, Some("X")).unwrap_err().contains("could not be recorded"));
}

#[test]
fn the_active_profile_is_remembered_and_activation_replaces_the_saved_one() {
    let root = temp("active");
    let shared = Shared::new(root.join("config.json"), config::defaults());
    let dir = shared.profiles_dir();
    assert_eq!(dir, root.join("profiles"));
    assert_eq!(profiles::active(&dir), None);
    let mut attack = config::defaults();
    attack.axes.pitch.curve = -0.2;
    profiles::save(&dir, "Attack", &attack).unwrap();
    shared.set_preview(Some(config::defaults())).unwrap();
    assert_eq!(shared.activate("Attack").unwrap(), attack);
    assert_eq!((shared.config().1, shared.previewing(), profiles::active(&dir)), (attack.clone(), false, Some("Attack".into())));
    assert_eq!(config::load(&root.join("config.json")).unwrap(), attack);
    assert!(shared.activate("Nope").is_err());
    profiles::set_active(&dir, None).unwrap();
    assert_eq!(profiles::active(&dir), None);
    std::fs::write(dir.join("active.txt"), "../evil").unwrap();
    assert_eq!(profiles::active(&dir), None);
    // Activation fails cleanly when the active profile cannot be written or recorded.
    std::fs::remove_file(dir.join("active.txt")).unwrap();
    std::fs::create_dir_all(dir.join("active.txt")).unwrap();
    assert!(shared.activate("Attack").unwrap_err().contains("could not be recorded"));
    let blocked = temp("blocked-config");
    std::fs::create_dir_all(blocked.join("config.json")).unwrap();
    let stuck = Shared::new(blocked.join("config.json"), config::defaults());
    profiles::save(&stuck.profiles_dir(), "Attack", &attack).unwrap();
    assert!(stuck.activate("Attack").unwrap_err().contains("could not be saved"));
    // Notices queue in order.
    assert!(shared.take_notice().is_none());
    shared.notice(Notice { title: "A", text: "1".into(), error: false });
    shared.notice(Notice { title: "B", text: "2".into(), error: true });
    assert_eq!(shared.take_notice().map(|n| n.title), Some("A"));
    assert_eq!(shared.take_notice().map(|n| (n.title, n.error)), Some(("B", true)));
}
