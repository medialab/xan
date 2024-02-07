use workdir::Workdir;

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
