//! The real executable's argument handling: `--permissions` prints the list, nothing else does.

mod run_support;

use std::process::Command;

use run_support::{DEADLINE, TempDir, run_full};

#[test]
fn the_executable_prints_its_list_only_for_exactly_permissions() {
    let dir = TempDir::new("permissions-run");
    let source = dir.0.join("p.gloss");
    std::fs::write(&source, "feature start\n  show \"ran\"\n").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_gloss"))
        .arg("build")
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(built.status.code(), Some(0), "{built:?}");
    let cases: [(&[&str], &[u8]); 4] = [
        (&["--permissions"], b"This program can:\n  nothing else\n"),
        (&["--permissionsX"], b"ran\n"),
        (&["--permissions", "x"], b"ran\n"),
        (&[""], b"ran\n"),
    ];
    for (args, want) in cases {
        let Some(ran) = run_full(&dir.0.join("p"), b"", DEADLINE, &[], None, args) else {
            return;
        };
        assert_eq!(ran.unwrap(), (0, want.to_vec(), vec![]), "{args:?}");
    }
}
