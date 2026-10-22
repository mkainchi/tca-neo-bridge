use neo_daemon::headtrack::{self, Listener};
use neo_daemon::launch;
use neo_daemon::process::Runner;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neo-launch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// tasklist and reg answers by tool name; anything else fails.
fn runner(tasklist: Option<&'static str>, reg: Option<String>) -> Runner {
    Box::new(move |tool: &Path, _: &[String]| match (tool.file_name().unwrap().to_str().unwrap(), &tasklist, &reg) {
        ("tasklist.exe", Some(out), _) => Ok((Some(0), out.to_string())),
        ("reg.exe", _, Some(out)) => Ok((Some(0), out.clone())),
        _ => Err(std::io::ErrorKind::NotFound.into()),
    })
}

fn localconfig(steam: &Path, user: &str, text: &str) {
    let dir = steam.join("userdata").join(user).join("config");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("localconfig.vdf"), text).unwrap();
}

const SYSTEM: &str = r"C:\Windows\System32";

#[test]
fn steam_running_comes_from_tasklist() {
    let system = Path::new(SYSTEM);
    assert!(launch::steam_running(&runner(Some("\"steam.exe\",\"4242\",\"Console\""), None), system));
    assert!(!launch::steam_running(&runner(Some("INFO: No tasks are running which match the specified criteria."), None), system));
    assert!(!launch::steam_running(&runner(None, None), system));
}

#[test]
fn steam_path_comes_from_the_registry() {
    let system = Path::new(SYSTEM);
    let reg = |text: &str| runner(None, Some(text.to_string()));
    let out = "\r\nHKEY_CURRENT_USER\\Software\\Valve\\Steam\r\n    SteamPath    REG_SZ    c:/program files (x86)/steam\r\n";
    assert_eq!(launch::steam_path(&reg(out), system), Some(PathBuf::from("c:/program files (x86)/steam")));
    for bad in ["", "    SteamPath    REG_DWORD    0x1", "    SteamPath    REG_SZ    ", "    Other    REG_SZ    x"] {
        assert_eq!(launch::steam_path(&reg(bad), system), None, "{bad}");
    }
    assert_eq!(launch::steam_path(&runner(None, None), system), None);
}

#[test]
fn vdf_values_are_read_from_the_named_block_only() {
    let text = r#""UserLocalConfigStore" { "apps" { "2288340" { "LastPlayed" "1" "nested" { "UseSteamControllerConfig" "9" } "UseSteamControllerConfig" "0" } "other" { "UseSteamControllerConfig" "2" } } }"#;
    assert_eq!(launch::vdf_value(text, "2288340", "UseSteamControllerConfig"), Some("0".into()));
    assert_eq!(launch::vdf_value(text, "other", "UseSteamControllerConfig"), Some("2".into()));
    assert_eq!(launch::vdf_value(text, "2288340", "Missing"), None);
    assert_eq!(launch::vdf_value(text, "absent", "UseSteamControllerConfig"), None);
    assert_eq!(launch::vdf_value(r#""2288340" { "key" { } }"#, "2288340", "key"), None); // a block, not a value
    assert_eq!(launch::vdf_value(r#""2288340" { "key""#, "2288340", "key"), None); // cut short
    assert_eq!(launch::vdf_value(r#""2288340" { "a" "b""#, "2288340", "key"), None); // unterminated block
}

#[test]
fn steam_input_and_the_warnings() {
    let steam = temp("steam");
    assert_eq!(launch::steam_input(&steam), None); // no userdata folder
    std::fs::create_dir_all(steam.join("userdata").join("empty")).unwrap();
    assert_eq!(launch::steam_input(&steam), None); // a user without a localconfig
    localconfig(&steam, "1", r#""apps" { "2288340" { "UseSteamControllerConfig" "0" } }"#);
    assert_eq!(launch::steam_input(&steam), Some(false));
    localconfig(&steam, "2", r#""apps" { "2288340" { "UseSteamControllerConfig" "2" } }"#);
    assert_eq!(launch::steam_input(&steam), Some(true));
    let reg = format!("    SteamPath    REG_SZ    {}", steam.display());
    let system = Path::new(SYSTEM);
    assert_eq!(launch::warnings(&runner(Some("\"steam.exe\""), Some(reg.clone())), system), [launch::STEAM_RUNNING, launch::STEAM_INPUT]);
    assert!(launch::warnings(&runner(None, None), system).is_empty());
    assert!(launch::warnings(&runner(Some("none"), Some("nothing".into())), system).is_empty());
}

#[test]
fn opentrack_datagrams_parse_and_reach_the_listener() {
    let mut datagram = [0u8; 48];
    datagram[24..32].copy_from_slice(&(-12.5f64).to_le_bytes());
    datagram[32..40].copy_from_slice(&7.0f64.to_le_bytes());
    assert_eq!(headtrack::parse(&datagram), Some([-12.5, 7.0]));
    assert_eq!(headtrack::parse(&datagram[..40]), None);
    let mut bad = datagram;
    bad[24..32].copy_from_slice(&f64::NAN.to_le_bytes());
    assert_eq!(headtrack::parse(&bad), None);
    let port = 45_000 + (std::process::id() % 2000) as u16;
    let listener = Listener::start(port).unwrap();
    assert!(Listener::start(port).err().unwrap().contains(&format!("UDP port {port}")));
    assert_eq!(listener.sample(Duration::from_secs(1)), None);
    let sender = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(&[1, 2, 3], ("127.0.0.1", port)).unwrap(); // ignored
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while listener.sample(Duration::from_secs(1)).is_none() {
        assert!(std::time::Instant::now() < deadline, "no sample");
        sender.send_to(&datagram, ("127.0.0.1", port)).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(listener.sample(Duration::from_secs(1)), Some([-12.5, 7.0]));
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(listener.sample(Duration::from_millis(1)), None); // stale
}
