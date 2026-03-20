use crate::workdir::Workdir;

#[test]
fn matrix_count() {
    let wrk = Workdir::new("matrix_count");
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
    cmd.arg("count").arg("true").arg("pred").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["", "true", "false"],
        svec!["true", "2", "4"],
        svec!["false", "1", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_count_weight() {
    let wrk = Workdir::new("matrix_count_weight");
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
    cmd.arg("count")
        .arg("true")
        .arg("pred")
        .arg("data.csv")
        .args(["--weight", "weight"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["", "true", "false"],
        svec!["true", "2", "2"],
        svec!["false", "0.5", "0"],
    ];
    assert_eq!(got, expected);
}
