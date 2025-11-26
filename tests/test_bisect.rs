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
    cmd.arg("n").arg("1").arg("range.csv").arg("-N");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["1"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("1001").arg("range.csv").arg("-N");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("453").arg("range.csv").arg("-N");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["453"]];
    assert_eq!(got, expected);

    wrk.create("range_neg.csv", range(-500, 500));
    let mut cmd = wrk.command("bisect");
    cmd.arg("-N")
        .arg("--")
        .arg("n")
        .arg("-1")
        .arg("range_neg.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["-1"]];
    assert_eq!(got, expected);
}

#[test]
fn bisect() {
    let wrk = Workdir::new("bisect");
    wrk.create(
        "letters.csv",
        vec![
            svec!["letter"],
            svec!["a"],
            svec!["a"],
            svec!["a"],
            svec!["b"],
            svec!["b"],
            svec!["c"],
            svec!["d"],
            svec!["e"],
        ],
    );
    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("b").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["b"], svec!["b"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("a").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["a"], svec!["a"], svec!["a"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("d").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["d"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("e").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["e"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("z").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"]];
    assert_eq!(got, expected);

    wrk.create(
        "letters.csv",
        vec![
            svec!["letter"],
            svec!["a"],
            svec!["b"],
            svec!["b"],
            svec!["c"],
            svec!["d"],
            svec!["e"],
            svec!["e"],
            svec!["e"],
            svec!["e"],
        ],
    );
    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("b").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["b"], svec!["b"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("e").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["letter"],
        svec!["e"],
        svec!["e"],
        svec!["e"],
        svec!["e"],
    ];
    assert_eq!(got, expected);

    wrk.create(
        "sentences.csv",
        vec![
            svec!["sentence"],
            svec!["Goodbye world"],
            svec!["Hello there"],
            svec!["Hello world"],
            svec!["Hello xan"],
            svec!["May the force be with you"],
            svec!["xan is great"],
            svec!["you can't tell the opposite"],
        ],
    );
    let mut cmd = wrk.command("bisect");
    cmd.arg("sentence")
        .arg("Goodbye world")
        .arg("sentences.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["sentence"], svec!["Goodbye world"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("sentence").arg("xan is great").arg("sentences.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["sentence"], svec!["xan is great"]];
    assert_eq!(got, expected);
}
