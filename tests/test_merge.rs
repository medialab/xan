use crate::workdir::Workdir;

#[test]
fn merge() {
    let wrk = Workdir::new("merge");
    wrk.create(
        "a.csv",
        vec![svec!["name"], svec!["bautista"], svec!["caroline"]],
    );
    wrk.create(
        "b.csv",
        vec![svec!["name"], svec!["anna"], svec!["delphine"]],
    );
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv").arg("b.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name"],
        svec!["anna"],
        svec!["bautista"],
        svec!["caroline"],
        svec!["delphine"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn merge_no_headers() {
    let wrk = Workdir::new("merge");
    wrk.create("a.csv", vec![svec!["bautista"], svec!["caroline"]]);
    wrk.create("b.csv", vec![svec!["anna"], svec!["delphine"]]);
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv").arg("b.csv").arg("--no-headers");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["anna"],
        svec!["bautista"],
        svec!["caroline"],
        svec!["delphine"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn merge_padding() {
    let wrk = Workdir::new("merge");
    wrk.create(
        "a.csv",
        vec![svec!["name"], svec!["bautista"], svec!["caroline"]],
    );
    wrk.create(
        "b.csv",
        vec![
            svec!["name"],
            svec!["anna"],
            svec!["delphine"],
            svec!["edna"],
            svec!["farid"],
        ],
    );
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv").arg("b.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name"],
        svec!["anna"],
        svec!["bautista"],
        svec!["caroline"],
        svec!["delphine"],
        svec!["edna"],
        svec!["farid"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn merge_reverse() {
    let wrk = Workdir::new("merge");
    wrk.create(
        "a.csv",
        vec![svec!["name"], svec!["caroline"], svec!["bautista"]],
    );
    wrk.create(
        "b.csv",
        vec![svec!["name"], svec!["delphine"], svec!["anna"]],
    );
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv").arg("b.csv").arg("--reverse");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name"],
        svec!["delphine"],
        svec!["caroline"],
        svec!["bautista"],
        svec!["anna"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn merge_numeric() {
    let wrk = Workdir::new("merge");
    wrk.create("a.csv", vec![svec!["n"], svec!["1"], svec!["3"]]);
    wrk.create("b.csv", vec![svec!["n"], svec!["2"], svec!["4"]]);
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv").arg("b.csv").arg("--numeric");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["1"], svec!["2"], svec!["3"], svec!["4"]];
    assert_eq!(got, expected);
}

#[test]
fn merge_numeric_reverse() {
    let wrk = Workdir::new("merge");
    wrk.create("a.csv", vec![svec!["n"], svec!["3"], svec!["1"]]);
    wrk.create("b.csv", vec![svec!["n"], svec!["4"], svec!["2"]]);
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv")
        .arg("b.csv")
        .arg("--numeric")
        .arg("--reverse");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["4"], svec!["3"], svec!["2"], svec!["1"]];
    assert_eq!(got, expected);
}

#[test]
fn merge_select() {
    let wrk = Workdir::new("merge");
    wrk.create(
        "a.csv",
        vec![
            svec!["name", "age"],
            svec!["bautista", "34"],
            svec!["caroline", "21"],
        ],
    );
    wrk.create(
        "b.csv",
        vec![
            svec!["name", "age"],
            svec!["anna", "37"],
            svec!["delphine", "18"],
        ],
    );
    let mut cmd = wrk.command("merge");
    cmd.arg("a.csv").arg("b.csv").args(["-s", "name"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "age"],
        svec!["anna", "37"],
        svec!["bautista", "34"],
        svec!["caroline", "21"],
        svec!["delphine", "18"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn merge_uniq() {
    let wrk = Workdir::new("merge");
    wrk.create(
        "a.csv",
        vec![
            svec!["n", "id"],
            svec!["1", "1"],
            svec!["1", "2"],
            svec!["3", "1"],
            svec!["3", "2"],
        ],
    );
    wrk.create(
        "b.csv",
        vec![
            svec!["n", "id"],
            svec!["1", "3"],
            svec!["2", "1"],
            svec!["4", "1"],
            svec!["4", "2"],
        ],
    );
    let mut cmd = wrk.command("merge");
    cmd.arg("b.csv")
        .arg("a.csv")
        .arg("--numeric")
        .arg("--uniq")
        .args(["-s", "n"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "id"],
        svec!["1", "1"],
        svec!["2", "1"],
        svec!["3", "1"],
        svec!["4", "1"],
    ];
    assert_eq!(got, expected);
}
