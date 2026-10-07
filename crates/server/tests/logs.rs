//! The server binary's log, as an operator collects it.

use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

#[test]
fn logs_sent_to_a_pipe_carry_no_colour_codes() {
    let mut server = Command::new(env!("CARGO_BIN_EXE_code-racer-server"))
        .args(["--port", "0"])
        .env("RUST_LOG", "info")
        .env_remove("NO_COLOR")
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server starts");
    let stderr = server.stderr.take().expect("a piped standard error");
    let first_line = BufReader::new(stderr).lines().next();
    server.kill().expect("the server stops");
    server.wait().expect("the server exits");
    let line = first_line
        .expect("a log line")
        .expect("a readable log line");
    assert!(line.contains("race server listening"), "{line}");
    assert!(!line.contains('\u{1b}'), "{line:?}");
}
