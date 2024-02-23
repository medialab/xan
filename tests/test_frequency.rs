use std::process;

use workdir::Workdir;

fn setup(name: &str) -> (Workdir, process::Command) {
    let rows = vec![
        svec!["h1", "h2"],
        svec!["a", "z"],
        svec!["a", "y"],
        svec!["a", "y"],
        svec!["b", "z"],
        svec!["", "z"],
    ];

    let wrk = Workdir::new(name);
    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("frequency");
    cmd.arg("in.csv");

    (wrk, cmd)
}

#[test]
fn frequency_no_headers() {
    let (wrk, mut cmd) = setup("frequency_no_headers");
    cmd.args(["--limit", "0"])
        .args(["--select", "0"])
        .arg("--no-headers");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got = got.into_iter().skip(1).collect();
    got.sort();
    let expected = vec![
        svec!["0", "<empty>", "1"],
        svec!["0", "a", "3"],
        svec!["0", "b", "1"],
        svec!["0", "h1", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn frequency_no_extra() {
    let (wrk, mut cmd) = setup("frequency_no_extra");
    cmd.arg("--no-extra")
        .args(["--limit", "0"])
        .args(["--select", "h1"]);

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got.sort();
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["h1", "a", "3"],
        svec!["h1", "b", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn frequency_nulls() {
    let (wrk, mut cmd) = setup("frequency_nulls");
    cmd.args(["--limit", "0"]).args(["--select", "h1"]);

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got.sort();
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["h1", "<empty>", "1"],
        svec!["h1", "a", "3"],
        svec!["h1", "b", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn frequency_limit() {
    let (wrk, mut cmd) = setup("frequency_limit");
    cmd.args(["--limit", "1"]).arg("--no-extra");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got.sort();
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["h1", "a", "3"],
        svec!["h2", "z", "3"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn frequency_reverse() {
    let (wrk, mut cmd) = setup("frequency_reverse");
    cmd.args(["--limit", "1"])
        .args(["--select", "h2"])
        .arg("--reverse")
        .arg("--no-extra");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got.sort();
    let expected = vec![svec!["field", "value", "count"], svec!["h2", "y", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn frequency_select() {
    let (wrk, mut cmd) = setup("frequency_select");
    cmd.args(["--limit", "0"]).args(["--select", "h2"]);

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got.sort();
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["h2", "y", "2"],
        svec!["h2", "z", "3"],
    ];
    assert_eq!(got, expected);
}
