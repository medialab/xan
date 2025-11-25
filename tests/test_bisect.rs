use crate::workdir::Workdir;

fn range(start: i32, end: i32) -> Vec<Vec<String>> {
    let mut header = vec![svec!["n"]];
    header.append(
        &mut (start..=end)
            .map(|n| vec![n.to_string()])
            .collect::<Vec<_>>(),
    );
    header
}

#[test]
fn bisect_numeric() {
    let wrk = Workdir::new("bisect_numeric");
    wrk.create("range.csv", range(0, 1000));
    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("1").arg("range.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["1"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("1001").arg("range.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("453").arg("range.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["453"]];
    assert_eq!(got, expected);

    // wrk.create("range_neg.csv", range(-500, 500));
    // let mut cmd = wrk.command("bisect");
    // cmd.arg("n").arg("'-1'").arg("range_neg.csv");
    // let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    // let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["-1"]];
    // assert_eq!(got, expected);
}
