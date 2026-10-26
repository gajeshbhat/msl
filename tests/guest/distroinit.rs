use super::*;

fn states(list: &[&str]) -> impl FnMut() -> String {
    let mut it = list.iter().map(|s| s.to_string()).collect::<Vec<_>>().into_iter();
    move || it.next().unwrap_or_default()
}

#[test]
fn waits_until_systemd_is_running() {
    let s = wait_systemd(Duration::from_secs(5), Duration::ZERO, states(&["", "initializing", "starting", "running"]));
    assert_eq!(s, "running");
}

#[test]
fn degraded_counts_as_ready() {
    let s = wait_systemd(Duration::from_secs(5), Duration::ZERO, states(&["starting", "degraded"]));
    assert_eq!(s, "degraded");
}

#[test]
fn gives_up_at_the_timeout() {
    let s = wait_systemd(Duration::from_millis(30), Duration::from_millis(1), || "initializing".into());
    assert_eq!(s, "initializing");
}

fn root_with(zones: &[&str]) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("msl-tz-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(root.join("etc")).unwrap();
    for z in zones {
        let f = root.join("usr/share/zoneinfo").join(z);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, "tz").unwrap();
    }
    root
}

fn link(root: &std::path::Path) -> Option<String> {
    std::fs::read_link(root.join("etc/localtime")).ok().map(|p| p.to_string_lossy().into_owned())
}

#[test]
fn localtime_points_at_the_zone() {
    let root = root_with(&["America/Vancouver"]);
    assert!(set_localtime(root.to_str().unwrap(), "America/Vancouver"));
    assert_eq!(link(&root).as_deref(), Some("/usr/share/zoneinfo/America/Vancouver"));
}

#[test]
fn localtime_replaces_an_existing_file_or_link() {
    let root = root_with(&["America/Vancouver"]);
    std::fs::write(root.join("etc/localtime"), "old").unwrap();
    assert!(set_localtime(root.to_str().unwrap(), "America/Vancouver"));
    assert!(set_localtime(root.to_str().unwrap(), "America/Vancouver"));
    assert_eq!(link(&root).as_deref(), Some("/usr/share/zoneinfo/America/Vancouver"));
}

#[test]
fn localtime_is_kept_when_the_image_lacks_the_zone() {
    let root = root_with(&["Etc/UTC"]);
    std::os::unix::fs::symlink("/usr/share/zoneinfo/Etc/UTC", root.join("etc/localtime")).unwrap();
    assert!(!set_localtime(root.to_str().unwrap(), "Asia/Kolkata"));
    assert_eq!(link(&root).as_deref(), Some("/usr/share/zoneinfo/Etc/UTC"));
}

#[test]
fn localtime_rejects_names_that_leave_zoneinfo() {
    let root = root_with(&["America/Vancouver"]);
    std::fs::create_dir_all(root.join("etc/ssh")).unwrap();
    std::fs::write(root.join("etc/passwd"), "x").unwrap();
    for bad in ["", "/etc/passwd", "../../etc/passwd", "America/../../../etc/passwd", "America//Vancouver", "a b"] {
        assert!(!set_localtime(root.to_str().unwrap(), bad), "{bad:?}");
    }
    assert_eq!(link(&root), None);
}

#[test]
fn use_windows_timezone_false_leaves_localtime_alone() {
    let root = root_with(&["America/Vancouver"]);
    let off = crate::config::Ini::parse("[time]\nuseWindowsTimezone=false\n");
    assert!(!sync_timezone(&off, root.to_str().unwrap(), "America/Vancouver"));
    assert_eq!(link(&root), None);
    let on = crate::config::Ini::parse("");
    assert!(sync_timezone(&on, root.to_str().unwrap(), "America/Vancouver"));
    assert_eq!(link(&root).as_deref(), Some("/usr/share/zoneinfo/America/Vancouver"));
}
