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

#[test]
fn search_patterns_url_prefix() {
    let wrk = Workdir::new("search_patterns_url_prefix");

    wrk.create(
        "patterns.csv",
        vec![
            svec!["url"],
            svec!["http://www.lemonde.fr"],
            svec!["lefigaro.fr/business"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["url"],
            svec!["http://lemonde.fr"],
            svec!["http://lemonde.fr/path/to.html"],
            svec!["http://lefigaro.fr"],
            svec!["http://lefigaro.fr/business/article.html"],
            svec!["http://liberation.fr"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("-u")
        .arg("lemonde.fr/pixels")
        .args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "url"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["url"],
        svec!["http://lemonde.fr"],
        svec!["http://lemonde.fr/path/to.html"],
        svec!["http://lefigaro.fr/business/article.html"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_replace() {
    let wrk = Workdir::new("search_replace");

    wrk.create(
        "data1.csv",
        vec![svec!["number"], svec!["3,4"], svec!["2"], svec!["10,7"]],
    );

    wrk.create(
        "data2.csv",
        vec![
            svec!["id", "number"],
            svec!["1", "3,4"],
            svec!["2", "2"],
            svec!["3", "10,7"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.arg(",").args(["--replace", "."]).arg("data1.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["number"], svec!["3.4"], svec!["2"], svec!["10.7"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("search");
    cmd.arg(",")
        .args(["--replace", "."])
        .args(["-s", "number"])
        .arg("data2.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "number"],
        svec!["1", "3.4"],
        svec!["2", "2"],
        svec!["3", "10.7"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_replace_regex() {
    let wrk = Workdir::new("search_replace_regex");

    wrk.create(
        "data.csv",
        vec![svec!["name"], svec!["john berry"], svec!["mike apple"]],
    );

    let mut cmd = wrk.command("search");
    cmd.arg("\\w+ (\\w+)")
        .arg("--regex")
        .args(["--replace", "name: $1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name"], svec!["name: berry"], svec!["name: apple"]];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_replace() {
    let wrk = Workdir::new("search_patterns_replace");

    wrk.create(
        "patterns.csv",
        vec![
            svec!["color", "replacement"],
            svec!["red", "rouge"],
            svec!["green", "vert"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["color"],
            svec!["this is red"],
            svec!["this is blue"],
            svec!["this is green"],
            svec!["this is yellow"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--replace", "."])
        .args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "color"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color"],
        svec!["this is ."],
        svec!["this is blue"],
        svec!["this is ."],
        svec!["this is yellow"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "color"])
        .args(["--replacement-column", "replacement"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color"],
        svec!["this is rouge"],
        svec!["this is blue"],
        svec!["this is vert"],
        svec!["this is yellow"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_exact_replace() {
    let wrk = Workdir::new("search_patterns_exact_replace");

    wrk.create(
        "patterns.csv",
        vec![
            svec!["color", "replacement"],
            svec!["red", "rouge"],
            svec!["green", "vert"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["color"],
            svec!["this is red"],
            svec!["red"],
            svec!["this is green"],
            svec!["green"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--replace", "."])
        .arg("-e")
        .args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "color"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color"],
        svec!["this is red"],
        svec!["."],
        svec!["this is green"],
        svec!["."],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .arg("-e")
        .args(["--pattern-column", "color"])
        .args(["--replacement-column", "replacement"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color"],
        svec!["this is red"],
        svec!["rouge"],
        svec!["this is green"],
        svec!["vert"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_regex_replace() {
    let wrk = Workdir::new("search_patterns_regex_replace");

    wrk.create(
        "patterns.csv",
        vec![
            svec!["color", "replacement"],
            svec!["this is (orange|red)", "color=$1"],
            svec!["this is (green)", "vert=$1"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["color"],
            svec!["this is red; this is orange"],
            svec!["this is blue ok"],
            svec!["this is green ok"],
            svec!["this is yellow"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--replace", "."])
        .args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "color"])
        .arg("-r")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color"],
        svec![".; ."],
        svec!["this is blue ok"],
        svec![". ok"],
        svec!["this is yellow"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "color"])
        .args(["--replacement-column", "replacement"])
        .arg("-r")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color"],
        svec!["color=red; color=orange"],
        svec!["this is blue ok"],
        svec!["vert=green ok"],
        svec!["this is yellow"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_url_replace() {
    let wrk = Workdir::new("search_patterns_url_replace");

    wrk.create(
        "urls.csv",
        vec![
            svec!["url", "color"],
            svec!["lemonde.fr", "yellow"],
            svec!["lefigaro.fr", "blue"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["url"],
            svec!["lemonde.fr/test"],
            svec!["liberation.fr/test"],
            svec!["lefigaro.fr/test"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--replace", "."])
        .arg("-u")
        .args(["--patterns", "urls.csv"])
        .args(["--pattern-column", "url"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["url"],
        svec!["."],
        svec!["liberation.fr/test"],
        svec!["."],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "urls.csv"])
        .arg("-u")
        .args(["--pattern-column", "url"])
        .args(["--replacement-column", "color"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["url"],
        svec!["yellow"],
        svec!["liberation.fr/test"],
        svec!["blue"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_breakdown() {
    let wrk = Workdir::new("search_patterns_breakdown");

    wrk.create(
        "patterns.csv",
        vec![
            svec!["article", "name"],
            svec!["le", "LE"],
            svec!["la", "LA"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["text"],
            svec!["le chien mange le fromage"],
            svec!["le chien mange la souris"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "article"])
        .arg("-B")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "le", "la"],
        svec!["le chien mange le fromage", "2", "0"],
        svec!["le chien mange la souris", "1", "1"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "article"])
        .arg("-B")
        .args(["--name-column", "name"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "LE", "LA"],
        svec!["le chien mange le fromage", "2", "0"],
        svec!["le chien mange la souris", "1", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn search_patterns_unique_matches() {
    let wrk = Workdir::new("search_patterns_unique_matches");

    wrk.create(
        "patterns.csv",
        vec![
            svec!["article", "name"],
            svec!["la", "LA"],
            svec!["le", "LE"],
        ],
    );

    wrk.create(
        "data.csv",
        vec![
            svec!["text"],
            svec!["le chien mange le fromage"],
            svec!["le chien mange la souris"],
            svec!["coucou"],
        ],
    );

    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "article"])
        .args(["-U", "matches"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "matches"],
        svec!["le chien mange le fromage", "le"],
        svec!["le chien mange la souris", "la|le"],
    ];
    assert_eq!(got, expected);

    // --name-column
    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "article"])
        .args(["--name-column", "name"])
        .args(["-U", "matches"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "matches"],
        svec!["le chien mange le fromage", "LE"],
        svec!["le chien mange la souris", "LA|LE"],
    ];
    assert_eq!(got, expected);

    // --sep
    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "article"])
        .args(["-U", "matches"])
        .args(["--sep", "§"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "matches"],
        svec!["le chien mange le fromage", "le"],
        svec!["le chien mange la souris", "la§le"],
    ];
    assert_eq!(got, expected);

    // --left
    let mut cmd = wrk.command("search");
    cmd.args(["--patterns", "patterns.csv"])
        .args(["--pattern-column", "article"])
        .args(["-U", "matches"])
        .arg("--left")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["text", "matches"],
        svec!["le chien mange le fromage", "le"],
        svec!["le chien mange la souris", "la|le"],
        svec!["coucou", ""],
    ];
    assert_eq!(got, expected);
}
