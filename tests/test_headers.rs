use std::process;

use crate::workdir::Workdir;

fn setup(name: &str) -> (Workdir, process::Command) {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", "b"]];
    let rows2 = vec![svec!["h2", "h3"], svec!["y", "z"]];

    let wrk = Workdir::new(name);
    wrk.create("in1.csv", rows1);
    wrk.create("in2.csv", rows2);

    let mut cmd = wrk.command("headers");
    cmd.arg("in1.csv");

    (wrk, cmd)
}

#[test]
fn headers_basic() {
    let (wrk, mut cmd) = setup("headers_basic");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "\
0 h1
1 h2";
    assert_eq!(got, expected.to_string());
}

#[test]
fn headers_just_names() {
    let (wrk, mut cmd) = setup("headers_just_names");
    cmd.arg("--just-names");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "\
h1
h2";
    assert_eq!(got, expected.to_string());
}

#[test]
fn headers_wide_column_index_has_trailing_space() {
    let wrk = Workdir::new("headers_wide_column_index_has_trailing_space");
    let header: Vec<String> = (0..1001).map(|i| format!("c{}", i)).collect();
    let row: Vec<String> = (0..1001).map(|i| format!("v{}", i)).collect();
    wrk.create("in.csv", vec![header, row]);

    let mut cmd = wrk.command("headers");
    cmd.arg("in.csv");

    let got: String = wrk.stdout(&mut cmd);
    let last_line = got.lines().last().unwrap();
    // Index 1000 must be separated from header name by whitespace, not glued.
    assert_eq!(last_line, "1000 c1000");
}

#[test]
fn headers_multiple() {
    let (wrk, mut cmd) = setup("headers_multiple");
    cmd.arg("in2.csv").arg("-j");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "\
in1.csv
h1
h2

in2.csv
h2
h3

All files don't have the same headers!
Diverging headers: h1, h3";
    assert_eq!(got, expected.to_string());
}
