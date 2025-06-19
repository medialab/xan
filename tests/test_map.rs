use crate::workdir::Workdir;

#[test]
fn map() {
    let wrk = Workdir::new("map");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)").arg("c").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_index() {
    let wrk = Workdir::new("map_index");
    wrk.create("data.csv", vec![svec!["n"], svec!["10"], svec!["15"]]);

    let mut cmd = wrk.command("map");
    cmd.arg("index()").arg("r").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "r"], svec!["10", "0"], svec!["15", "1"]];
    assert_eq!(got, expected);
}

#[test]
fn map_parallel() {
    let wrk = Workdir::new("map_parallel");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)").arg("c").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_threads() {
    let wrk = Workdir::new("map_threads");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)")
        .arg("c")
        .args(&["-t", "1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_errors_panic() {
    let wrk = Workdir::new("map_errors_panic");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "test"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)").arg("c").arg("data.csv");

    wrk.assert_err(&mut cmd);
}

#[test]
fn map_errors_report() {
    let wrk = Workdir::new("map_errors_report");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "test"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)")
        .arg("c")
        .args(&["-E", "report"])
        .args(&["--error-column", "error"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c", "error"],
        svec![
            "1",
            "test",
            "",
            "add() cannot safely cast Bytes(\"test\") from type \"bytes\" to type \"number\""
        ],
        svec!["2", "3", "5", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_errors_ignore() {
    let wrk = Workdir::new("map_errors_ignore");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "test"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)")
        .arg("c")
        .args(&["-E", "ignore"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "test", ""],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_errors_log() {
    let wrk = Workdir::new("map_errors_log");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "test"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b)")
        .arg("c")
        .args(&["-E", "log"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "test", ""],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}
