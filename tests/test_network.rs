use crate::workdir::Workdir;

#[test]
fn network() {
    let wrk = Workdir::new("network");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .args(["source", "target"])
        .args(["-f", "nodelist"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["node"], ["A"], ["B"], ["C"]];
    assert_eq!(got, expected);
}

#[test]
fn network_simple() {
    let wrk = Workdir::new("network_simple");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .arg("--simple")
        .args(["source", "target"])
        .args(["-f", "nodelist"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["node"], ["A"], ["B"], ["C"]];
    assert_eq!(got, expected);
}
