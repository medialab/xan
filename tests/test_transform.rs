use crate::workdir::Workdir;

#[test]
fn transform() {
    let wrk = Workdir::new("transform");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("transform");
    cmd.arg("b").arg("add(a, b)").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"], svec!["1", "3"], svec!["2", "5"]];
    assert_eq!(got, expected);
}

#[test]
fn transform_no_headers() {
    let wrk = Workdir::new("transform_no_headers");
    wrk.create("data.csv", vec![svec!["1", "2"], svec!["2", "3"]]);
    let mut cmd = wrk.command("transform");
    cmd.arg("-n")
        .arg("1")
        .arg("add(col(0), col(1))")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["1", "3"], svec!["2", "5"]];
    assert_eq!(got, expected);
}

#[test]
fn transform_rename() {
    let wrk = Workdir::new("transform_rename");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("transform");
    cmd.arg("b")
        .arg("add(a, b)")
        .args(&["-r", "c"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "c"], svec!["1", "3"], svec!["2", "5"]];
    assert_eq!(got, expected);
}

#[test]
fn transform_multi() {
    let wrk = Workdir::new("transform_multi");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b", "c"],
            svec!["1", "2", "5"],
            svec!["2", "3", "8"],
        ],
    );
    let mut cmd = wrk.command("transform");
    cmd.arg("b,c")
        .arg("_ * 10")
        .args(["-r", "B,C"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "B", "C"],
        svec!["1", "20", "50"],
        svec!["2", "30", "80"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn transform_implicit() {
    let wrk = Workdir::new("transform_implicit");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "davis"],
            svec!["mary", "sue"],
        ],
    );
    let mut cmd = wrk.command("transform");
    cmd.arg("surname")
        .arg("upper")
        .args(&["-r", "upper_surname"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "upper_surname"],
        svec!["john", "DAVIS"],
        svec!["mary", "SUE"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn transform_errors_panic() {
    let wrk = Workdir::new("transform_errors_panic");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "test"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("transform");
    cmd.arg("b").arg("add(a, b)").arg("data.csv");

    wrk.assert_err(&mut cmd);
}
