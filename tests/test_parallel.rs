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
