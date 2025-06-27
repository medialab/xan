use crate::workdir::Workdir;

#[test]
fn map() {
    let wrk = Workdir::new("map");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b) as c").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_multi() {
    let wrk = Workdir::new("map_multi");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b) as c, mul(a, b) as d").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c", "d"],
        svec!["1", "2", "3", "2"],
        svec!["2", "3", "5", "6"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_index() {
    let wrk = Workdir::new("map_index");
    wrk.create("data.csv", vec![svec!["n"], svec!["10"], svec!["15"]]);

    let mut cmd = wrk.command("map");
    cmd.arg("index() as r").arg("data.csv");

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
    cmd.arg("add(a, b) as c").arg("-p").arg("data.csv");

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
    cmd.arg("add(a, b) as c").args(["-t", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}
