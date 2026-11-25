use super::*;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

#[test]
fn the_private_folder_is_0700_and_removed() {
    let a = PrivateDir::new().unwrap();
    let b = PrivateDir::new().unwrap();
    let mode = std::fs::metadata(a.path()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_ne!(a.path(), b.path());
    let path = a.path().to_path_buf();
    drop(a);
    assert!(!path.exists());
}

#[test]
fn names_carry_the_process_id_and_do_not_repeat() {
    let (a, b) = (candidate_name(), candidate_name());
    assert_ne!(a, b);
    assert!(
        a.starts_with(&format!("gloss-test-{}-", std::process::id())),
        "{a}"
    );
    // The random part differs even when everything else could be guessed.
    let random = |n: &str| n.rsplit('-').next().unwrap().to_string();
    assert_eq!(random(&a).len(), 16);
    assert_ne!(random(&a), random(&b));
}

#[test]
fn a_taken_name_is_skipped() {
    let base = PrivateDir::new().unwrap();
    std::fs::create_dir(base.path().join("first")).unwrap();
    let mut names = ["first", "second"].into_iter().map(String::from);
    let dir = PrivateDir::create_in(base.path(), &mut names).unwrap();
    assert_eq!(dir.path(), base.path().join("second"));
    assert!(base.path().join("first").exists());
    // With every name taken it fails instead of looping.
    let mut taken = ["first"].into_iter().map(String::from);
    assert!(PrivateDir::create_in(base.path(), &mut taken).is_err());
}

#[test]
fn a_panic_still_removes_the_folder() {
    let mut path = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let dir = PrivateDir::new().unwrap();
        path = Some(dir.path().to_path_buf());
        panic!("boom");
    }));
    assert!(result.is_err());
    assert!(!path.unwrap().exists());
}

#[test]
fn status_for_maps_exit_codes() {
    let at = |raw| status_for(ExitStatus::from_raw(raw));
    assert_eq!(at(0), (0, None));
    assert_eq!(at(1 << 8), (1, None));
    let ended = |n: i32| Some(format!("the test run ended without finishing (status {n})"));
    assert_eq!(at(7 << 8), (1, ended(7)));
    assert_eq!(at(11), (1, ended(267)));
}

#[test]
fn host_decision_is_linux_x86_64_only() {
    assert!(host_can_run("linux", "x86_64"));
    assert!(!host_can_run("linux", "aarch64"));
    assert!(!host_can_run("macos", "aarch64"));
    assert!(!host_can_run("macos", "x86_64"));
}
