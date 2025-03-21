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
fn search_count() {
    let wrk = Workdir::new("search_count");
    wrk.create("data.csv", data(false));
    let mut cmd = wrk.command("search");
    cmd.arg("-r")
        .arg("foo")
        .arg("data.csv")
        .args(["--count", "matches"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["foobar", "barfoo", "matches"],
        svec!["a", "b", "0"],
        svec!["barfoo", "foobar", "2"],
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
fn search_patterns_substring_case_insensitive() {
    let wrk = Workdir::new("search_patterns_substring_case_insensitive");

    wrk.create("index.csv", vec![svec!["name"], svec!["SUZ"], svec!["JO"]]);

    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["JOHN"],
            svec!["ABigail"],
            svec!["SuZy"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "index.csv"])
        .arg("-i")
        .args(["--pattern-column", "name"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["JOHN"], svec!["SuZy"]];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_exact() {
    let wrk = Workdir::new("search_patterns_exact");

    // NOTE: testing with two columns to make sure --pattern-colum is working
    wrk.create(
        "index.csv",
        vec![
            svec!["name", "color"],
            svec!["suzy", "red"],
            svec!["john", "yellow"],
        ],
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

#[test]
fn search_count_patterns_regex() {
    let wrk = Workdir::new("search_count_patterns_regex");

    wrk.create(
        "patterns.csv",
        vec![svec!["pattern"], svec!["john"], svec!["lucy"]],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["text"],
            svec!["Lucy went to school with John."],
            svec!["The dog was running on the grass."],
            svec!["john is dead. poor john"],
            svec!["Lucy in the sky with diamonds"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("-r")
        .arg("-i")
        .args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "pattern"])
        .args(["--count", "matches"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "matches"],
        svec!["Lucy went to school with John.", "2"],
        svec!["The dog was running on the grass.", "0"],
        svec!["john is dead. poor john", "2"],
        svec!["Lucy in the sky with diamonds", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_count_overlapping_patterns_substring() {
    let wrk = Workdir::new("search_count_overlapping_patterns_substring");

    wrk.create(
        "patterns.csv",
        vec![svec!["pattern"], svec!["ab"], svec!["a"], svec!["b"]],
    );

    wrk.create("data.csv", vec![svec!["text"], svec!["baba"]]);

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "pattern"])
        .args(["--count", "matches"])
        .arg("--overlapping")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["text", "matches"], svec!["baba", "5"]];
    assert_eq!(got, expected);
}

#[test]
fn search_count_overlapping_regex() {
    let wrk = Workdir::new("search_count_overlapping_regex");

    wrk.create("data.csv", vec![svec!["text"], svec!["baba"]]);

    let mut cmd = wrk.command("search");
    cmd.arg("--regex")
        .arg("(ba|a|b)")
        .args(["--count", "matches"])
        .arg("--overlapping")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["text", "matches"], svec!["baba", "4"]];
    assert_eq!(got, expected);
}

#[test]
fn search_count_overlapping_patterns_regex() {
    let wrk = Workdir::new("search_count_overlapping_patterns_regex");

    wrk.create(
        "patterns.csv",
        vec![svec!["pattern"], svec!["(ab|b|a)"], svec!["a"], svec!["b"]],
    );

    wrk.create("data.csv", vec![svec!["text"], svec!["baba"]]);

    let mut cmd = wrk.command("search");
    cmd.arg("--regex")
        .args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "pattern"])
        .args(["--count", "matches"])
        .arg("--overlapping")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["text", "matches"], svec!["baba", "8"]];
    assert_eq!(got, expected);
}

#[test]
fn search_url_prefix() {
    let wrk = Workdir::new("search_url_prefix");

    wrk.create(
        "data.csv",
        vec![
            svec!["url"],
            svec!["http://lemonde.fr/pixels/one.html"],
            svec!["http://lefigaro.fr"],
            svec!["http://lemonde.fr/business/one.html"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("-u").arg("lemonde.fr/pixels").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["url"], svec!["http://lemonde.fr/pixels/one.html"]];
    assert_eq!(got, expected);
}
