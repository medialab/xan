use workdir::Workdir;

#[test]
fn range() {
    let wrk = Workdir::new("range");

    let mut cmd = wrk.command("range");
    cmd.arg("3");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["0"], svec!["1"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn range_column_name() {
    let wrk = Workdir::new("range_column_name");

    let mut cmd = wrk.command("range");
    cmd.arg("3").args(&["-c", "id"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["id"], svec!["0"], svec!["1"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn range_start() {
    let wrk = Workdir::new("range_start");

    let mut cmd = wrk.command("range");
    cmd.arg("3").args(&["-s", "1"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["1"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn range_step() {
    let wrk = Workdir::new("range_step");

    let mut cmd = wrk.command("range");
    cmd.arg("11").args(&["--step", "5"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["0"], svec!["5"], svec!["10"]];
    assert_eq!(got, expected);
}

#[test]
fn range_inclusive() {
    let wrk = Workdir::new("range_inclusive");

    let mut cmd = wrk.command("range");
    cmd.arg("3").arg("-i");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["0"], svec!["1"], svec!["2"], svec!["3"]];
    assert_eq!(got, expected);
}
