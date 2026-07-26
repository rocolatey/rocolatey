use assert_cmd::prelude::*;
use std::process::Command;

fn has_ansi_escape(text: &str) -> bool {
    text.contains('\x1b')
}

#[test]
fn outdated_json_output_contains_no_ansi_sequences() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_roco"));
    let assert = cmd.args(["outdated", "--json"]).assert().success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("stdout must be valid UTF-8 for json output");

    assert!(
        !has_ansi_escape(&stdout),
        "JSON output must not contain ANSI escapes, got: {:?}",
        stdout
    );

    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("outdated --json should emit valid JSON");
}

#[test]
fn outdated_limit_output_contains_no_ansi_sequences() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_roco"));
    let assert = cmd.args(["outdated", "-r"]).assert().success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("stdout must be valid UTF-8 for -r output");

    assert!(
        !has_ansi_escape(&stdout),
        "-r output must not contain ANSI escapes, got: {:?}",
        stdout
    );
}
