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
    cmd.arg("title").arg("html").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["html", "title"],
        svec!["<title>One</title>", "One"],
        svec!["<title>Two</title>", "Two"],
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
    cmd.arg("title").arg("html").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["html", "title"],
        svec!["<title>One</title>", "One"],
        svec!["<title>Two</title>", "Two"],
        svec!["<title>Three</title>", "Three"],
        svec!["<title>Four</title>", "Four"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn scrape_input_dir() {
    let wrk = Workdir::new("scrape_input_dir");
    wrk.write("one.html", "<title>One</title>");
    wrk.write("two.html", "<title>Two</title>");

    wrk.create(
        "data.csv",
        vec![svec!["path"], svec!["one.html"], svec!["two.html"]],
    );
    let mut cmd = wrk.command("scrape");
    cmd.arg("title")
        .arg("path")
        .args(["-I", "."])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["path", "title"],
        svec!["one.html", "One"],
        svec!["two.html", "Two"],
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
    cmd.arg("title")
        .arg("html")
        .args(["--keep", ""])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["title"], svec!["One"], svec!["Two"]];
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
    cmd.arg("html")
        .args(["-e", "a {title: text; url: attr('href');}"])
        .args(["--keep", ""])
        .arg("data.csv");

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

    cmd.arg("html")
        .args(["-e", "& {item: text;}"])
        .args(["--keep", ""])
        .args(["--foreach", "li"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["item"], svec!["one"], svec!["two"]];
    assert_eq!(got, expected);
}

#[test]
fn scrape_sep() {
    let wrk = Workdir::new("scrape_sep");
    wrk.create(
        "data.csv",
        vec![svec!["html"], svec!["<ul><li>one</li><li>two</li></ul>"]],
    );
    let mut cmd = wrk.command("scrape");
    cmd.arg("html")
        .args(["-e", "all('li') {text: text;}"])
        .args(["-k", ""])
        .args(["--sep", "§"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["text"], svec!["one§two"]];
    assert_eq!(got, expected);
}
