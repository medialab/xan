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

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "duplicated"],
        svec!["1", "1", "false"],
        svec!["2", "2", "false"],
        svec!["2", "2", "true"],
        svec!["1", "1", "true"],
    ];
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

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("-e").args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "duplicated"],
        svec!["1", "1", "false"],
        svec!["2", "2", "false"],
        svec!["2", "2", "true"],
        svec!["1", "1", "true"],
    ];
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

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .args(["-s", "a"])
        .arg("-l")
        .args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "i", "duplicated"],
        svec!["2", "2", "true"],
        svec!["3", "1", "true"],
        svec!["2", "3", "true"],
        svec!["1", "4", "true"],
        svec!["3", "5", "false"],
        svec!["2", "6", "false"],
        svec!["1", "7", "false"],
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

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv").arg("-S").args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "duplicated"],
        svec!["1", "1", "false"],
        svec!["2", "2", "false"],
        svec!["2", "2", "true"],
        svec!["1", "1", "false"],
        svec!["1", "1", "true"],
        svec!["3", "3", "false"],
        svec!["3", "3", "true"],
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

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .arg("-S")
        .args(["-s", "a"])
        .arg("-l")
        .args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["a", "i", "duplicated"],
        ["1", "1", "false"],
        ["2", "2", "true"],
        ["2", "3", "false"],
        ["3", "4", "true"],
        ["3", "5", "false"],
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

#[test]
fn dedup_keep_duplicates() {
    let wrk = Workdir::new("dedup_keep_duplicates");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["4", "1"],
            svec!["2", "2"],
            svec!["2", "3"],
            svec!["3", "4"],
        ],
    );

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .arg("--keep-duplicates")
        .args(["-s", "a"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"], svec!["2", "2"], svec!["2", "3"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .arg("--keep-duplicates")
        .args(["-s", "a"])
        .args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["a", "b", "duplicated"],
        ["4", "1", "false"],
        ["2", "2", "true"],
        ["2", "3", "true"],
        ["3", "4", "false"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn dedup_keep_duplicates_sorted() {
    let wrk = Workdir::new("dedup_keep_duplicates_sorted");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "3"],
            svec!["3", "4"],
        ],
    );

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .arg("--keep-duplicates")
        .args(["-s", "a"])
        .arg("-S");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"], svec!["2", "2"], svec!["2", "3"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .arg("--keep-duplicates")
        .args(["-s", "a"])
        .arg("-S")
        .args(["-f", "duplicated"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "duplicated"],
        svec!["1", "1", "false"],
        svec!["2", "2", "true"],
        svec!["2", "3", "true"],
        svec!["3", "4", "false"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn dedup_keep_duplicates_sorted_trailing() {
    let wrk = Workdir::new("dedup_keep_duplicates_sorted_trailing");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "1"],
            svec!["2", "2"],
            svec!["2", "3"],
            svec!["3", "4"],
            svec!["3", "5"],
        ],
    );

    let mut cmd = wrk.command("dedup");
    cmd.arg("data.csv")
        .arg("--keep-duplicates")
        .args(["-s", "a"])
        .arg("-S");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b"],
        svec!["2", "2"],
        svec!["2", "3"],
        svec!["3", "4"],
        svec!["3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn dedup_choose() {
    let wrk = Workdir::new("dedup_choose");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "count"],
            svec!["mary", "1"],
            svec!["john", "2"],
            svec!["mary", "8"],
            svec!["mary", "7"],
            svec!["john", "1"],
            svec!["lucy", "1"],
        ],
    );

    let mut cmd = wrk.command("dedup");
    cmd.args(["-s", "name"])
        .args(["--choose", "new_count > current_count"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "count"],
        svec!["mary", "8"],
        svec!["john", "2"],
        svec!["lucy", "1"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("dedup");
    cmd.args(["-s", "name"])
        .args(["-f", "duplicated"])
        .args(["--choose", "new_count > current_count"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name", "count", "duplicated"],
        ["mary", "1", "true"],
        ["mary", "7", "true"],
        ["john", "1", "true"],
        ["mary", "8", "false"],
        ["john", "2", "false"],
        ["lucy", "1", "false"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn dedup_choose_sorted() {
    let wrk = Workdir::new("dedup_choose_sorted");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "count"],
            svec!["mary", "1"],
            svec!["mary", "8"],
            svec!["mary", "7"],
            svec!["john", "2"],
            svec!["john", "1"],
            svec!["lucy", "1"],
        ],
    );

    let mut cmd = wrk.command("dedup");
    cmd.args(["-s", "name"])
        .arg("-S")
        .args(["--choose", "new_count > current_count"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "count"],
        svec!["mary", "8"],
        svec!["john", "2"],
        svec!["lucy", "1"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("dedup");
    cmd.args(["-s", "name"])
        .arg("-S")
        .args(["--choose", "new_count > current_count"])
        .args(["-f", "duplicated"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name", "count", "duplicated"],
        ["mary", "1", "true"],
        ["mary", "7", "true"],
        ["mary", "8", "false"],
        ["john", "1", "true"],
        ["john", "2", "false"],
        ["lucy", "1", "false"],
    ];
    assert_eq!(got, expected);
}
