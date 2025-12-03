use crate::workdir::Workdir;

#[test]
fn reverse() {
    let wrk = Workdir::new("reverse");
    wrk.create(
        "data.csv",
        vec![svec!["n"], svec!["1"], svec!["2"], svec!["3"]],
    );
    let mut cmd = wrk.command("reverse");
    cmd.arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["3"], svec!["2"], svec!["1"]];
    assert_eq!(got, expected);
}

#[test]
fn reverse_no_headers() {
    let wrk = Workdir::new("reverse_no_headers");
    wrk.create("data.csv", vec![svec!["1"], svec!["2"], svec!["3"]]);
    let mut cmd = wrk.command("reverse");
    cmd.arg("-n").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["3"], svec!["2"], svec!["1"]];
    assert_eq!(got, expected);
}

#[test]
fn reverse_empty() {
    let wrk = Workdir::new("reverse_empty");
    wrk.create::<Vec<Vec<String>>>("data.csv", vec![]);
    let mut cmd = wrk.command("reverse");
    cmd.arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, Vec::<Vec<String>>::new());
}
