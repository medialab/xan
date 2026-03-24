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
        ["", "true", "false"],
        ["true", "2", "1"],
        ["false", "4", ""],
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
        ["", "true", "false"],
        ["true", "2", "0.5"],
        ["false", "2", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_count_rectangular() {
    let wrk = Workdir::new("matrix_count_rectangular");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b", "weight"],
            svec!["one", "deux", "1"],
            svec!["one", "trois", "5"],
            svec!["two", "un", "2"],
            svec!["one", "deux", "7"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("count")
        .arg("a")
        .arg("b")
        .args(["-w", "weight"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "one", "two"],
        ["deux", "8", ""],
        ["trois", "5", ""],
        ["un", "", "2"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_adj() {
    let wrk = Workdir::new("matrix_adj");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b", "weight"],
            svec!["one", "deux", "1"],
            svec!["one", "trois", "5"],
            svec!["two", "un", "2"],
            svec!["one", "deux", "7"],
            svec!["one", "one", "4"],
            svec!["two", "two", "1"],
            svec!["two", "one", "5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("adj")
        .arg("a")
        .arg("b")
        .args(["-w", "weight"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "one", "deux", "trois", "two", "un"],
        ["one", "4", "", "", "5", ""],
        ["deux", "8", "", "", "", ""],
        ["trois", "5", "", "", "", ""],
        ["two", "", "", "", "1", ""],
        ["un", "", "", "", "2", ""],
    ];
    assert_eq!(got, expected);
}
