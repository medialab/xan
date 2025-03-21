use crate::workdir::Workdir;

#[test]
fn sample() {
    let wrk = Workdir::new("sample");
    wrk.create(
        "data.csv",
        vec![
            svec!["number"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
        ],
    );
    let mut cmd = wrk.command("sample");
    cmd.arg("2").arg("data.csv").args(["--seed", "123"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["number"], svec!["3"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn sample_grouped() {
    let wrk = Workdir::new("sample_grouped");
    wrk.create(
        "data.csv",
        vec![
            svec!["number", "group"],
            svec!["1", "group1"],
            svec!["2", "group1"],
            svec!["3", "group1"],
            svec!["4", "group2"],
        ],
    );
    let mut cmd = wrk.command("sample");
    cmd.arg("2")
        .arg("data.csv")
        .args(["--seed", "123"])
        .args(["-g", "group"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["number", "group"],
        svec!["3", "group1"],
        svec!["2", "group1"],
        svec!["4", "group2"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn sample_weighted() {
    let wrk = Workdir::new("sample_weighted");
    wrk.create(
        "data.csv",
        vec![
            svec!["number", "weight"],
            svec!["1", "0.001"],
            svec!["2", "0.001"],
            svec!["3", "0.5"],
            svec!["4", "0.9"],
        ],
    );
    let mut cmd = wrk.command("sample");
    cmd.arg("2")
        .args(["--weight", "weight"])
        .arg("data.csv")
        .args(["--seed", "123"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["number", "weight"],
        svec!["3", "0.5"],
        svec!["4", "0.9"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn sample_weighted_grouped() {
    let wrk = Workdir::new("sample_weighted_grouped");
    wrk.create(
        "data.csv",
        vec![
            svec!["number", "weight", "group"],
            svec!["1", "0.001", "group1"],
            svec!["2", "0.001", "group1"],
            svec!["3", "0.5", "group1"],
            svec!["4", "0.9", "group2"],
        ],
    );
    let mut cmd = wrk.command("sample");
    cmd.arg("2")
        .args(["--weight", "weight"])
        .arg("data.csv")
        .args(["--seed", "123"])
        .args(["-g", "group"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["number", "weight", "group"],
        svec!["1", "0.001", "group1"],
        svec!["3", "0.5", "group1"],
        svec!["4", "0.9", "group2"],
    ];
    assert_eq!(got, expected);
}
