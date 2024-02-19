use workdir::Workdir;

#[test]
fn dedup() {
    let wrk = Workdir::new("dedup");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "2"],
            svec!["1", "1"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"], svec!["1", "1"], svec!["2", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn dedup_no_headers() {
    let wrk = Workdir::new("dedup_no_headers");
    wrk.create(
        "data.csv",
        vec![
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "2"],
            svec!["1", "1"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("-n");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["1", "1"], svec!["2", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn dedup_select() {
    let wrk = Workdir::new("dedup_select");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "1"],
            svec!["1", "2"],
            svec!["2", "2"],
            svec!["1", "3"],
            svec!["2", "3"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.args(&["-s", "a"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"], svec!["1", "1"], svec!["2", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn dedup_sorted() {
    let wrk = Workdir::new("dedup_sorted");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "2"],
            svec!["1", "1"],
            svec!["1", "1"],
            svec!["3", "3"],
            svec!["3", "3"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("-S");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b"],
        svec!["1", "1"],
        svec!["2", "2"],
        svec!["1", "1"],
        svec!["3", "3"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn dedup_sorted_no_headers() {
    let wrk = Workdir::new("dedup_sorted_no_headers");
    wrk.create(
        "data.csv",
        vec![
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "2"],
            svec!["1", "1"],
            svec!["1", "1"],
            svec!["3", "3"],
            svec!["3", "3"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("-S").arg("-n");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["1", "1"],
        svec!["2", "2"],
        svec!["1", "1"],
        svec!["3", "3"],
    ];
    assert_eq!(got, expected);
}
