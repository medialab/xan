use crate::workdir::Workdir;

#[test]
fn scrape() {
    let wrk = Workdir::new("scrape");
    wrk.create(
        "data.csv",
        vec![
            svec!["html"],
            svec!["<title>One</title>"],
            svec!["<title>Two</title>"],
        ],
    );
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["title", "canonical_url",],
        svec!["One", ""],
        svec!["Two", ""],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .arg("-k")
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["html", "title", "canonical_url",],
        svec!["<title>One</title>", "One", ""],
        svec!["<title>Two</title>", "Two", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn scrape_xml() {
    let wrk = Workdir::new("scrape_xml");
    wrk.create(
        "data.csv",
        vec![
            svec!["xml"],
            svec!["<agenda><name>John</name><surname>landis</surname></agenda>"],
        ],
    );
    let mut cmd = wrk.command("scrape");
    cmd.args(["-e", "agenda {name {name: text} surname {surname: text}}"])
        .args(["--doc-column", "xml"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name", "surname"], ["John", "landis"]];
    assert_eq!(got, expected);
}

#[test]
fn scrape_parallel() {
    let wrk = Workdir::new("scrape");
    wrk.create(
        "data.csv",
        vec![
            svec!["html"],
            svec!["<title>One</title>"],
            svec!["<title>Two</title>"],
            svec!["<title>Three</title>"],
            svec!["<title>Four</title>"],
        ],
    );
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"])
        .arg("-p");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["title", "canonical_url"],
        svec!["One", ""],
        svec!["Two", ""],
        svec!["Three", ""],
        svec!["Four", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn scrape_paths() {
    let wrk = Workdir::new("scrape_paths");
    wrk.write("one.html", "<title>One</title>");
    wrk.write("two.html", "<title>Two</title>");

    wrk.create(
        "paths.csv",
        vec![svec!["path"], svec!["one.html"], svec!["two.html"]],
    );

    // --path-column
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["--paths", "paths.csv"])
        .args(["--path-column", "path"])
        .args(["-I", "."]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["path", "title", "canonical_url"],
        svec!["one.html", "One", ""],
        svec!["two.html", "Two", ""],
    ];
    assert_eq!(got, expected);

    // <inputs>...
    let mut cmd = wrk.command("scrape");
    cmd.arg("head").args(["one.html", "two.html"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["path", "title", "canonical_url"],
        svec!["one.html", "One", ""],
        svec!["two.html", "Two", ""],
    ];
    assert_eq!(got, expected);

    // --glob
    let mut cmd = wrk.command("scrape");
    cmd.arg("head").args(["--glob", "*.html"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["path", "title", "canonical_url"],
        svec!["one.html", "One", ""],
        svec!["two.html", "Two", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn scrape_evaluate() {
    let wrk = Workdir::new("scrape_evaluate");
    wrk.create(
        "data.csv",
        vec![
            svec!["html"],
            svec!["<a href=\"https://lemonde.fr\">Le Monde</a>"],
            svec!["<a href=\"https://lefigaro.fr\">Le Figaro</a>"],
        ],
    );
    let mut cmd = wrk.command("scrape");
    cmd.args(["-e", "a {title: text; url: attr('href');}"])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["title", "url"],
        svec!["Le Monde", "https://lemonde.fr"],
        svec!["Le Figaro", "https://lefigaro.fr"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn scrape_evaluate_foreach() {
    let wrk = Workdir::new("scrape_evaluate_foreach");
    wrk.create(
        "data.csv",
        vec![svec!["html"], svec!["<ul><li>one</li><li>two</li></ul>"]],
    );
    let mut cmd = wrk.command("scrape");

    cmd.args(["-e", "& {item: text;}"])
        .args(["--foreach", "li"])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["item"], svec!["one"], svec!["two"]];
    assert_eq!(got, expected);
}

#[test]
fn scrape_select() {
    let wrk = Workdir::new("scrape_select");
    wrk.create(
        "data.csv",
        vec![
            svec!["html", "id", "name"],
            svec!["<title>One</title>", "1", "one"],
            svec!["<title>Two</title>", "2", "two"],
        ],
    );

    // Simple selection
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["-s", "id"])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "title", "canonical_url"],
        svec!["1", "One", ""],
        svec!["2", "Two", ""],
    ];
    assert_eq!(got, expected);

    // Empty selection
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["-s", "!"])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["title", "canonical_url"],
        svec!["One", ""],
        svec!["Two", ""],
    ];
    assert_eq!(got, expected);

    // Doc in selection
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["-s", "id,html"])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["id", "html", "title", "canonical_url"],
        ["1", "<title>One</title>", "One", ""],
        ["2", "<title>Two</title>", "Two", ""],
    ];
    assert_eq!(got, expected);

    // -k with selection
    let mut cmd = wrk.command("scrape");
    cmd.arg("head")
        .args(["-s", "id"])
        .arg("-k")
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["id", "html", "title", "canonical_url"],
        ["1", "<title>One</title>", "One", ""],
        ["2", "<title>Two</title>", "Two", ""],
    ];
    assert_eq!(got, expected);
}
