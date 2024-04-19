use std::process;

use crate::workdir::Workdir;
use crate::Csv;

fn no_headers(cmd: &mut process::Command) {
    cmd.arg("--no-headers");
}

fn pad(cmd: &mut process::Command) {
    cmd.arg("--pad");
}

fn run_cat<X, Y, Z, F>(test_name: &str, which: &str, rows1: X, rows2: Y, modify_cmd: F) -> Z
where
    X: Csv,
    Y: Csv,
    Z: Csv,
    F: FnOnce(&mut process::Command),
{
    let wrk = Workdir::new(test_name);
    wrk.create("in1.csv", rows1);
    wrk.create("in2.csv", rows2);

    let mut cmd = wrk.command("cat");
    modify_cmd(cmd.arg(which).arg("in1.csv").arg("in2.csv"));
    wrk.read_stdout(&mut cmd)
}

#[test]
fn cat_rows_space() {
    let rows = vec![svec!["\u{0085}"]];
    let expected = rows.clone();
    let (rows1, rows2) = if rows.is_empty() {
        (vec![], vec![])
    } else {
        let (rows1, rows2) = rows.split_at(rows.len() / 2);
        (rows1.to_vec(), rows2.to_vec())
    };
    let got: Vec<Vec<String>> = run_cat("cat_rows_space", "rows", rows1, rows2, no_headers);
    assert_eq!(got, expected);
}

#[test]
fn cat_rows_headers() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", "b"]];
    let rows2 = vec![svec!["h1", "h2"], svec!["y", "z"]];

    let mut expected = rows1.clone();
    expected.extend(rows2.clone().into_iter().skip(1));

    let got: Vec<Vec<String>> = run_cat("cat_rows_headers", "rows", rows1, rows2, |_| ());
    assert_eq!(got, expected);
}

#[test]
fn cat_cols_headers() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", "b"]];
    let rows2 = vec![svec!["h3", "h4"], svec!["y", "z"]];

    let expected = vec![svec!["h1", "h2", "h3", "h4"], svec!["a", "b", "y", "z"]];
    let got: Vec<Vec<String>> = run_cat("cat_cols_headers", "columns", rows1, rows2, |_| ());
    assert_eq!(got, expected);
}

#[test]
fn cat_cols_no_pad() {
    let rows1 = vec![svec!["a", "b"]];
    let rows2 = vec![svec!["y", "z"], svec!["y", "z"]];

    let expected = vec![svec!["a", "b", "y", "z"]];
    let got: Vec<Vec<String>> = run_cat("cat_cols_headers", "columns", rows1, rows2, no_headers);
    assert_eq!(got, expected);
}

#[test]
fn cat_cols_pad() {
    let rows1 = vec![svec!["a", "b"]];
    let rows2 = vec![svec!["y", "z"], svec!["y", "z"]];

    let expected = vec![svec!["a", "b", "y", "z"], svec!["", "", "y", "z"]];
    let got: Vec<Vec<String>> = run_cat("cat_cols_headers", "columns", rows1, rows2, pad);
    assert_eq!(got, expected);
}

#[test]
fn cat_input() {
    let wrk = Workdir::new("cat");
    wrk.create("a.csv", vec![svec!["name"], svec!["John"]]);
    wrk.create("b.csv", vec![svec!["name"], svec!["Suzy"]]);
    wrk.create("p.csv", vec![svec!["path"], svec!["a.csv"], svec!["b.csv"]]);

    let mut cmd = wrk.command("cat");
    cmd.arg("rows").arg("path").args(["--input", "p.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["John"], svec!["Suzy"]];
    assert_eq!(got, expected);
}
