use crate::workdir::Workdir;

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
fn dedup_external() {
    let wrk = Workdir::new("dedup_external");
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
    cmd.arg("data.csv").arg("-e");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"], svec!["1", "1"], svec!["2", "2"]];
    assert_eq!(got, expected);
}

#[test]
fn dedup_keep_last() {
    let wrk = Workdir::new("dedup_keep_last");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "i"],
            svec!["3", "1"],
            svec!["2", "2"],
            svec!["2", "3"],
            svec!["1", "4"],
            svec!["3", "5"],
            svec!["2", "6"],
            svec!["1", "7"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").args(["-s", "a"]).arg("-l");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "i"],
        svec!["3", "5"],
        svec!["2", "6"],
        svec!["1", "7"],
    ];
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
fn dedup_sorted_keep_last() {
    let wrk = Workdir::new("dedup_sorted_keep_last");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "i"],
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "3"],
            svec!["3", "4"],
            svec!["3", "5"],
        ],
    );
    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("-S").args(["-s", "a"]).arg("-l");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "i"],
        svec!["1", "1"],
        svec!["2", "3"],
        svec!["3", "5"],
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

#[test]
fn dedup_check() {
    let wrk = Workdir::new("dedup_check");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "3"],
            svec!["1", "4"],
        ],
    );

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("--check").args(["-s", "a"]);

    wrk.assert_err(&mut cmd);

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("--check").args(["-s", "b"]);

    wrk.assert_success(&mut cmd);
}
