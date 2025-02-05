use crate::workdir::Workdir;

fn data(headers: bool) -> Vec<Vec<String>> {
    let mut rows = vec![
        svec!["foobar", "barfoo"],
        svec!["a", "b"],
        svec!["barfoo", "foobar"],
    ];
    if headers {
        rows.insert(0, svec!["h1", "h2"]);
    }
    rows
}

#[test]
fn search() {
    let wrk = Workdir::new("search");
    wrk.create("data.csv", data(true));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^foo").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["h1", "h2"],
        svec!["foobar", "barfoo"],
        svec!["barfoo", "foobar"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_empty_regex() {
    let wrk = Workdir::new("search_empty_regex");
    wrk.create("data.csv", data(true));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("xxx").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["h1", "h2"]];
    assert_eq!(got, expected);
}

#[test]
fn search_empty_regex_no_headers() {
    let wrk = Workdir::new("search_empty_regex_no_headers");
    wrk.create("data.csv", data(true));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("xxx").arg("data.csv");
    cmd.arg("--no-headers");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![];
    assert_eq!(got, expected);
}

#[test]
fn search_ignore_case() {
    let wrk = Workdir::new("search_ignore_case");
    wrk.create("data.csv", data(true));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^FoO").arg("data.csv");
    cmd.arg("--ignore-case");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["h1", "h2"],
        svec!["foobar", "barfoo"],
        svec!["barfoo", "foobar"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_no_headers() {
    let wrk = Workdir::new("search_no_headers");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^foo").arg("data.csv");
    cmd.arg("--no-headers");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["foobar", "barfoo"], svec!["barfoo", "foobar"]];
    assert_eq!(got, expected);
}

#[test]
fn search_select() {
    let wrk = Workdir::new("search_select");
    wrk.create("data.csv", data(true));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^foo").arg("data.csv");
    cmd.arg("--select").arg("h2");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["h1", "h2"], svec!["barfoo", "foobar"]];
    assert_eq!(got, expected);
}

#[test]
fn search_select_no_headers() {
    let wrk = Workdir::new("search_select_no_headers");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^foo").arg("data.csv");
    cmd.arg("--select").arg("1");
    cmd.arg("--no-headers");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["barfoo", "foobar"]];
    assert_eq!(got, expected);
}

#[test]
fn search_invert_match() {
    let wrk = Workdir::new("search_invert_match");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^foo").arg("data.csv");
    cmd.arg("--invert-match");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["foobar", "barfoo"], svec!["a", "b"]];
    assert_eq!(got, expected);
}

#[test]
fn search_invert_match_no_headers() {
    let wrk = Workdir::new("search_invert_match");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r").arg("^foo").arg("data.csv");
    cmd.arg("--invert-match");
    cmd.arg("--no-headers");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["a", "b"]];
    assert_eq!(got, expected);
}

#[test]
fn search_flag() {
    let wrk = Workdir::new("search_flag");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r")
        .arg("^foo")
        .arg("data.csv")
        .args(["--flag", "flagged"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["foobar", "barfoo", "flagged"],
        svec!["a", "b", "0"],
        svec!["barfoo", "foobar", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_flag_invert_match() {
    let wrk = Workdir::new("search_flag");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r")
        .arg("^foo")
        .arg("data.csv")
        .args(["--flag", "flagged"]);
    cmd.arg("--invert-match");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["foobar", "barfoo", "flagged"],
        svec!["a", "b", "1"],
        svec!["barfoo", "foobar", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_substring() {
    let wrk = Workdir::new("search_substring");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "number"],
            svec!["John", "13"],
            svec!["JohnJohn", "24"],
            svec!["Abigail", "72"],
        ],
    );
    let mut cmd = wrk.command("search");
    cmd.arg("John").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "number"],
        svec!["John", "13"],
        svec!["JohnJohn", "24"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_substring_case_insensitive() {
    let wrk = Workdir::new("search_substring_case_insensitive");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "number"],
            svec!["JOHN", "13"],
            svec!["John", "24"],
            svec!["Abigail", "72"],
        ],
    );
    let mut cmd = wrk.command("search");
    cmd.arg("jO").arg("data.csv").arg("-i");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "number"],
        svec!["JOHN", "13"],
        svec!["John", "24"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_flag_exact() {
    let wrk = Workdir::new("search_exact");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "number"],
            svec!["John", "13"],
            svec!["JohnJohn", "24"],
            svec!["Abigail", "72"],
        ],
    );
    let mut cmd = wrk.command("search");
    cmd.arg("John").arg("data.csv").arg("--exact");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name", "number"], svec!["John", "13"]];
    assert_eq!(got, expected);
}

#[test]
fn search_flag_exact_case_insensitive() {
    let wrk = Workdir::new("search_exact_case_insensitive");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "number"],
            svec!["JOHN", "13"],
            svec!["John", "24"],
            svec!["Abigail", "72"],
        ],
    );
    let mut cmd = wrk.command("search");
    cmd.arg("joHn").arg("data.csv").arg("--exact").arg("-i");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "number"],
        svec!["JOHN", "13"],
        svec!["John", "24"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_substring() {
    let wrk = Workdir::new("search_patterns_substring");

    wrk.create("index.csv", vec![svec!["name"], svec!["suz"], svec!["jo"]]);

    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["john"],
            svec!["abigail"],
            svec!["suzy"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "index.csv"])
        .args(["--pattern-column", "name"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["john"], svec!["suzy"]];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_exact() {
    let wrk = Workdir::new("search_patterns_exact");

    wrk.create(
        "index.csv",
        vec![svec!["name"], svec!["suzy"], svec!["john"]],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["john"],
            svec!["abigail"],
            svec!["suzy"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "index.csv"])
        .args(["--pattern-column", "name"])
        .arg("data.csv")
        .arg("--exact");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["john"], svec!["suzy"]];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_exact_case_insensitive() {
    let wrk = Workdir::new("search_patterns_exact_case_insensitive");

    wrk.create(
        "index.csv",
        vec![svec!["name"], svec!["sUzy"], svec!["jOhn"]],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["John"],
            svec!["Abigail"],
            svec!["suZy"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "index.csv"])
        .args(["--pattern-column", "name"])
        .arg("data.csv")
        .arg("--exact")
        .arg("-i");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["John"], svec!["suZy"]];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_regex() {
    let wrk = Workdir::new("search_patterns_regex");

    wrk.create(
        "index.csv",
        vec![svec!["name"], svec!["^su"], svec!["hn$"], svec![r"^a\."]],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["John"],
            svec!["Abigail"],
            svec!["Suzy"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("-r")
        .args(["--patterns", "index.csv"])
        .args(["--pattern-column", "name"])
        .arg("data.csv")
        .arg("-i");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["John"], svec!["Suzy"]];
    assert_eq!(got, expected);
}

#[test]
fn search_non_empty() {
    let wrk = Workdir::new("search_non_empty");

    wrk.create(
        "data.csv",
        vec![svec!["name"], svec!["John"], svec![""], svec!["Suzy"]],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("data.csv").arg("--non-empty");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["John"], svec!["Suzy"]];
    assert_eq!(got, expected);
}

#[test]
fn search_non_empty_invert_match() {
    let wrk = Workdir::new("search_non_empty_invert_match");

    wrk.create(
        "data.csv",
        vec![svec!["name"], svec!["John"], svec![""], svec!["Suzy"]],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("data.csv").arg("--non-empty").arg("-v");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec![""]];
    assert_eq!(got, expected);
}

#[test]
fn search_empty() {
    let wrk = Workdir::new("search_empty");

    wrk.create(
        "data.csv",
        vec![svec!["name"], svec!["John"], svec![""], svec!["Suzy"]],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("data.csv").arg("--empty");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec![""]];
    assert_eq!(got, expected);
}

#[test]
fn search_all() {
    let wrk = Workdir::new("search_all");

    wrk.create(
        "data.csv",
        vec![
            svec!["name", "color"],
            svec!["John", "red"],
            svec!["", "yellow"],
            svec!["Suzy", ""],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("data.csv").arg("--non-empty").arg("--all");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name", "color"], svec!["John", "red"]];
    assert_eq!(got, expected);
}
