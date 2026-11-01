use std::process::Command;

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rs-demo"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn demo_and_comparison_recover_data() {
    let demo = cli(&[
        "demo",
        "--bytes",
        "1001",
        "--missing",
        "0,4,7",
        "--iterations",
        "1",
    ]);
    assert!(
        demo.status.success(),
        "{}",
        String::from_utf8_lossy(&demo.stderr)
    );
    assert!(String::from_utf8_lossy(&demo.stdout).contains("verified byte-for-byte"));
    let compare = cli(&["compare", "--bytes", "257", "--iterations", "1"]);
    assert!(compare.status.success());
    assert_eq!(
        String::from_utf8_lossy(&compare.stdout)
            .matches("verified byte-for-byte")
            .count(),
        4
    );
    assert!(
        cli(&[
            "demo",
            "--bytes",
            "0",
            "--missing",
            "none",
            "--iterations",
            "1"
        ])
        .status
        .success()
    );
}

#[test]
fn rejects_invalid_cli_inputs() {
    for args in [
        vec!["--data", "0"],
        vec!["--iterations", "0"],
        vec!["--missing", "0,0"],
        vec!["--missing", "9"],
        vec!["--missing", "0,1,2,3"],
        vec!["compare", "--data", "4"],
        vec!["--unknown", "1"],
        vec!["--bytes"],
        vec!["--bytes", "bad"],
    ] {
        assert!(!cli(&args).status.success(), "{args:?}");
    }
    assert!(cli(&["--help"]).status.success());
}
