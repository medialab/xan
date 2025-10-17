use crate::workdir::Workdir;

#[test]
fn parallel_count() {
    let wrk = Workdir::new("parallel_count");
    wrk.create(
        "data1.csv",
        vec![svec!["color"], svec!["blue"], svec!["yellow"]],
    );
    wrk.create("data2.csv", vec![svec!["color"], svec!["red"]]);

    let mut cmd = wrk.command("parallel");
    cmd.arg("count").arg("data1.csv").arg("data2.csv");

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "3");
}

#[test]
fn parallel_count_single_file() {
    let wrk = Workdir::new("parallel_count_single_file");

    let mut cmd = wrk.command("parallel");
    cmd.arg("count").arg(wrk.resource("series.csv"));

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "432");
}

#[test]
fn parallel_count_single_file_preprocess() {
    let wrk = Workdir::new("parallel_count_single_file_preprocess");

    let mut cmd = wrk.command("parallel");
    cmd.arg("count")
        .args(["-P", "search -es Category Disc"])
        .arg(wrk.resource("series.csv"));

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "85");
}

#[test]
fn parallel_count_source_column() {
    let wrk = Workdir::new("parallel_count_source_column");
    wrk.create(
        "data1.csv",
        vec![svec!["color"], svec!["blue"], svec!["yellow"], svec!["red"]],
    );
    wrk.create("data2.csv", vec![svec!["color"], svec!["red"]]);

    let mut cmd = wrk.command("parallel");
    cmd.arg("count")
        .args(["--source-column", "source"])
        .arg("data1.csv")
        .arg("data2.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort_by_key(|r| r[0].to_owned());

    let expected = vec![
        svec!["source", "count"],
        svec!["data1.csv", "3"],
        svec!["data2.csv", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn parallel_freq() {
    let wrk = Workdir::new("parallel_freq");
    wrk.create(
        "data1.csv",
        vec![
            svec!["color"],
            svec!["blue"],
            svec!["blue"],
            svec!["yellow"],
        ],
    );
    wrk.create(
        "data2.csv",
        vec![svec!["color"], svec!["red"], svec!["red"], svec!["blue"]],
    );

    let mut cmd = wrk.command("parallel");
    cmd.arg("freq")
        .args(["-s", "color"])
        .arg("data1.csv")
        .arg("data2.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["color", "blue", "3"],
        svec!["color", "red", "2"],
        svec!["color", "yellow", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn parallel_freq_single_file() {
    let wrk = Workdir::new("parallel_freq_single_file");

    let mut cmd = wrk.command("parallel");
    cmd.arg("freq")
        .args(["-s", "Category"])
        .arg(wrk.resource("series.csv"));

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["Category", "Vinyl", "94"],
        svec!["Category", "Disc", "85"],
        svec!["Category", "Other", "75"],
        svec!["Category", "Download", "66"],
        svec!["Category", "Tape", "64"],
        svec!["Category", "Streaming", "48"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn parallel_freq_sep() {
    let wrk = Workdir::new("parallel_freq_sep");
    wrk.create(
        "data1.csv",
        vec![
            svec!["color"],
            svec!["blue"],
            svec!["blue|red"],
            svec!["yellow|red"],
        ],
    );
    wrk.create(
        "data2.csv",
        vec![svec!["color"], svec!["red"], svec!["red"], svec!["blue"]],
    );

    let mut cmd = wrk.command("parallel");
    cmd.arg("freq")
        .args(["-s", "color"])
        .args(["--sep", "|"])
        .arg("data1.csv")
        .arg("data2.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["field", "value", "count"],
        svec!["color", "red", "4"],
        svec!["color", "blue", "3"],
        svec!["color", "yellow", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn parallel_cat() {
    let wrk = Workdir::new("parallel_cat");
    wrk.create(
        "data1.csv",
        vec![
            svec!["color"],
            svec!["blue"],
            svec!["blue"],
            svec!["yellow"],
        ],
    );
    wrk.create(
        "data2.csv",
        vec![svec!["color"], svec!["red"], svec!["red"], svec!["blue"]],
    );

    let mut cmd = wrk.command("parallel");
    cmd.arg("cat")
        .args(["-P", "search -e 'yellow'"])
        .arg("data1.csv")
        .arg("data2.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["color"], svec!["yellow"]];
    assert_eq!(got, expected);
}

#[test]
fn parallel_cat_source_column() {
    let wrk = Workdir::new("parallel_cat_source_column");
    wrk.create(
        "data1.csv",
        vec![svec!["color"], svec!["blue"], svec!["yellow"]],
    );
    wrk.create(
        "data2.csv",
        vec![svec!["color"], svec!["red"], svec!["red"], svec!["blue"]],
    );

    let mut cmd = wrk.command("parallel");
    cmd.arg("cat")
        .args(["-P", "search -e 'blue'"])
        .args(["--source-column", "file"])
        .arg("data1.csv")
        .arg("data2.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort_by_key(|r| r[1].to_owned());

    let expected = vec![
        svec!["color", "file"],
        svec!["blue", "data1.csv"],
        svec!["blue", "data2.csv"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn parallel_agg() {
    let wrk = Workdir::new("parallel_agg");
    wrk.create("data1.csv", vec![svec!["n"], svec!["4"], svec!["7"]]);
    wrk.create("data2.csv", vec![svec!["n"], svec!["8"]]);

    let mut cmd = wrk.command("parallel");
    cmd.arg("agg")
        .arg("sum(n) as sum")
        .arg("data1.csv")
        .arg("data2.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![svec!["sum"], svec!["19"]];
    assert_eq!(got, expected);
}

#[test]
fn parallel_groupby() {
    let wrk = Workdir::new("parallel_groupby");
    wrk.create(
        "data1.csv",
        vec![svec!["n", "name"], svec!["4", "john"], svec!["7", "mary"]],
    );
    wrk.create("data2.csv", vec![svec!["n", "name"], svec!["8", "john"]]);

    let mut cmd = wrk.command("parallel");
    cmd.arg("groupby")
        .arg("name")
        .arg("sum(n) as sum")
        .arg("data1.csv")
        .arg("data2.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort_by_key(|r| r[0].to_owned());

    let expected = vec![
        svec!["name", "sum"],
        svec!["john", "12"],
        svec!["mary", "7"],
    ];
    assert_eq!(got, expected);
}
