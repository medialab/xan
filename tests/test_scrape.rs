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
        svec!["html", "title", "canonical_url",],
        svec!["<title>One</title>", "One", ""],
        svec!["<title>Two</title>", "Two", ""],
    ];
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
        svec!["html", "title", "canonical_url"],
        svec!["<title>One</title>", "One", ""],
        svec!["<title>Two</title>", "Two", ""],
        svec!["<title>Three</title>", "Three", ""],
        svec!["<title>Four</title>", "Four", ""],
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
fn scrape_keep() {
    let wrk = Workdir::new("scrape_keep");
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
        .args(["--keep", ""])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["title", "canonical_url"],
        svec!["One", ""],
        svec!["Two", ""],
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
        .args(["--keep", ""])
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
        .args(["--keep", ""])
        .args(["--foreach", "li"])
        .args(["--doc-column", "html"])
        .args(["--docs", "data.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["item"], svec!["one"], svec!["two"]];
    assert_eq!(got, expected);
}
