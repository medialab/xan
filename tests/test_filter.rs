use crate::workdir::Workdir;

#[test]
fn filter() {
    let wrk = Workdir::new("filter");
    wrk.create(
        "data.csv",
        vec![svec!["a"], svec!["1"], svec!["2"], svec!["3"]],
    );
    let mut cmd = wrk.command("filter");
    cmd.arg("eq(a, 3)").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a"], svec!["3"]];
    assert_eq!(got, expected);
}

#[test]
fn filter_invert_match() {
    let wrk = Workdir::new("filter_invert_match");
    wrk.create(
        "data.csv",
        vec![svec!["a"], svec!["1"], svec!["2"], svec!["3"]],
    );
    let mut cmd = wrk.command("filter");
    cmd.arg("eq(a, 3)").arg("-v").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a"], svec!["1"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn filter_limit() {
    let wrk = Workdir::new("filter_limit");
    wrk.create(
        "data.csv",
        vec![svec!["a"], svec!["1"], svec!["2"], svec!["3"]],
    );
    let mut cmd = wrk.command("filter");
    cmd.arg("a > 1").args(["-l", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn filter_context() {
    let wrk = Workdir::new("filter_context");
    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["clarice"],
            svec!["john"],
            svec!["john"],
            svec!["john"],
            svec!["john"],
            svec!["lucy"],
            svec!["amy"],
        ],
    );

    let mut cmd = wrk.command("filter");
    cmd.arg("name eq 'john'").args(["-B", "3"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name"],
        ["clarice"],
        ["john"],
        ["john"],
        ["john"],
        ["john"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("filter");
    cmd.arg("name eq 'lucy'").args(["-B", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], ["lucy"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("filter");
    cmd.arg("name eq 'lucy'").args(["-A", "2"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["lucy"], ["amy"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("filter");
    cmd.arg("name eq 'lucy'")
        .args(["-A", "1"])
        .args(["-B", "1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], ["lucy"], ["amy"]];
    assert_eq!(got, expected);
}

#[test]
fn filter_context_parallel() {
    let wrk = Workdir::new("filter_context_parallel");
    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["clarice"],
            svec!["john"],
            svec!["john"],
            svec!["john"],
            svec!["john"],
            svec!["lucy"],
            svec!["amy"],
        ],
    );

    let mut cmd = wrk.command("filter");
    cmd.arg("-p")
        .arg("name eq 'john'")
        .args(["-B", "3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name"],
        ["clarice"],
        ["john"],
        ["john"],
        ["john"],
        ["john"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("filter");
    cmd.arg("-p")
        .arg("name eq 'lucy'")
        .args(["-B", "1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], ["lucy"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("filter");
    cmd.arg("-p")
        .arg("name eq 'lucy'")
        .args(["-A", "2"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["lucy"], ["amy"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("filter");
    cmd.arg("-p")
        .arg("name eq 'lucy'")
        .args(["-A", "1"])
        .args(["-B", "1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], ["lucy"], ["amy"]];
    assert_eq!(got, expected);
}
