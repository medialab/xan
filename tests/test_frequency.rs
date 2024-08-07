use std::process;

use crate::workdir::Workdir;

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

#[test]
fn frequency_stability() {
    let wrk = Workdir::new("frequency_stability");
    wrk.create(
        "data.csv",
        vec![
            svec!["a"],
            svec!["x"],
            svec!["x"],
            svec!["y"],
            svec!["y"],
            svec!["z"],
            svec!["z"],
        ],
    );

    let mut cmd = wrk.command("frequency");
    cmd.arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["a", "x", "2"],
        svec!["a", "y", "2"],
        svec!["a", "z", "2"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("frequency");
    cmd.arg("data.csv").args(&["-l", "0"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["a", "x", "2"],
        svec!["a", "y", "2"],
        svec!["a", "z", "2"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("frequency");
    cmd.arg("data.csv").args(&["-l", "1"]).arg("-N");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["field", "value", "count"], svec!["a", "x", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn frequency_groubby() {
    let wrk = Workdir::new("frequency_groubby");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "color"],
            svec!["john", "blue"],
            svec!["mary", "red"],
            svec!["mary", "red"],
            svec!["mary", "red"],
            svec!["mary", "purple"],
            svec!["john", "yellow"],
            svec!["john", "blue"],
        ],
    );

    let mut cmd = wrk.command("frequency");
    cmd.args(["-s", "color"])
        .args(["-g", "name"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["field", "name", "value", "count"],
        svec!["color", "john", "blue", "2"],
        svec!["color", "john", "yellow", "1"],
        svec!["color", "mary", "red", "3"],
        svec!["color", "mary", "purple", "1"],
    ];
    assert_eq!(got, expected);

    // With limit
    let mut cmd = wrk.command("frequency");
    cmd.args(["-s", "color"])
        .args(["-g", "name"])
        .args(["-l", "1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["field", "name", "value", "count"],
        svec!["color", "john", "blue", "2"],
        svec!["color", "john", "<rest>", "1"],
        svec!["color", "mary", "red", "3"],
        svec!["color", "mary", "<rest>", "1"],
    ];
    assert_eq!(got, expected);

    // With limit, without extras
    let mut cmd = wrk.command("frequency");
    cmd.args(["-s", "color"])
        .args(["-g", "name"])
        .args(["-l", "1"])
        .arg("--no-extra")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["field", "name", "value", "count"],
        svec!["color", "john", "blue", "2"],
        svec!["color", "mary", "red", "3"],
    ];
    assert_eq!(got, expected);
}
