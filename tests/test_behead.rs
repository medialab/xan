use crate::workdir::Workdir;

#[test]
fn behead() {
    let wrk = Workdir::new("behead");
    wrk.create(
        "data.csv",
        vec![svec!["letter", "number"], svec!["a", "1"], svec!["b", "2"]],
    );
    let mut cmd = wrk.command("behead");
    cmd.arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "1"], svec!["b", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn behead_tsv() {
    let wrk = Workdir::new("behead_tsv");
    wrk.create_with_delimiter(
        b'\t',
        "data.tsv",
        vec![svec!["letter", "number"], svec!["a", "1"], svec!["b", "2"]],
    );
    let mut cmd = wrk.command("behead");
    cmd.arg("data.tsv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "1"], svec!["b", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn behead_raw() {
    let wrk = Workdir::new("behead_raw");
    wrk.create(
        "data.csv",
        vec![svec!["letter", "number"], svec!["a", "1"], svec!["b", "2"]],
    );
    let mut cmd = wrk.command("behead");
    cmd.arg("data.csv").arg("--raw");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "1"], svec!["b", "2"]];
    assert_eq!(got, expected);
}
