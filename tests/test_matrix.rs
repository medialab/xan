use crate::workdir::Workdir;

#[test]
fn adj() {
    let wrk = Workdir::new("sample");
    wrk.create(
        "data.csv",
        vec![
            svec!["true", "pred", "weight"],
            svec!["true", "true", "1"],
            svec!["true", "true", "1"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["false", "true", "0.5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("adj").arg("true").arg("pred").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["", "true", "false"],
        svec!["true", "2", "4"],
        svec!["false", "1", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn adj_weight() {
    let wrk = Workdir::new("sample");
    wrk.create(
        "data.csv",
        vec![
            svec!["true", "pred", "weight"],
            svec!["true", "true", "1"],
            svec!["true", "true", "1"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["false", "true", "0.5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("adj").arg("true").arg("pred").arg("data.csv").args(["--weight", "weight"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["", "true", "false"],
        svec!["true", "2", "2"],
        svec!["false", "0.5", "0"],
    ];
    assert_eq!(got, expected);
}