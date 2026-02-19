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
    cmd.arg("n").arg("1").arg("range.csv").arg("-NS");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["1"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("997").arg("range.csv").arg("-NE");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["998"], svec!["999"], svec!["1000"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("1001").arg("range.csv").arg("-NS");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("n").arg("453").arg("range.csv").arg("-NS");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["n"], svec!["453"]];
    assert_eq!(got, expected);

    wrk.create("range_neg.csv", range(-500, 500));
    let mut cmd = wrk.command("bisect");
    cmd.arg("-NS")
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
    cmd.arg("-S").arg("letter").arg("b").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["b"], svec!["b"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S").arg("letter").arg("a").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["a"], svec!["a"], svec!["a"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("-E").arg("b").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["c"], svec!["d"], svec!["e"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("b").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["letter"],
        svec!["b"],
        svec!["b"],
        svec!["c"],
        svec!["d"],
        svec!["e"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S").arg("letter").arg("d").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["d"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S").arg("letter").arg("e").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["e"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("e").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["e"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("-E").arg("e").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("letter").arg("a").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["letter"],
        svec!["a"],
        svec!["a"],
        svec!["a"],
        svec!["b"],
        svec!["b"],
        svec!["c"],
        svec!["d"],
        svec!["e"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S").arg("letter").arg("z").arg("letters.csv");
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
    cmd.arg("-S").arg("letter").arg("b").arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["b"], svec!["b"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S").arg("letter").arg("e").arg("letters.csv");
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
    cmd.arg("-S")
        .arg("sentence")
        .arg("Goodbye world")
        .arg("sentences.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["sentence"], svec!["Goodbye world"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("sentence").arg("Hello yoda").arg("sentences.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["sentence"],
        svec!["May the force be with you"],
        svec!["xan is great"],
        svec!["you can't tell the opposite"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S")
        .arg("sentence")
        .arg("xan is great")
        .arg("sentences.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["sentence"], svec!["xan is great"]];
    assert_eq!(got, expected);

    wrk.create(
        "duplicates.csv",
        vec![
            svec!["letter"],
            svec!["a"],
            svec!["a"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["b"],
            svec!["c"],
        ],
    );
    let mut cmd = wrk.command("bisect");
    cmd.arg("-S").arg("letter").arg("b").arg("duplicates.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["letter"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
        svec!["b"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn bisect_reverse() {
    let wrk = Workdir::new("bisect_reverse");
    wrk.create(
        "letters.csv",
        vec![
            svec!["letter"],
            svec!["e"],
            svec!["e"],
            svec!["e"],
            svec!["d"],
            svec!["c"],
            svec!["b"],
            svec!["b"],
            svec!["a"],
            svec!["a"],
            svec!["a"],
        ],
    );
    let mut cmd = wrk.command("bisect");
    cmd.arg("-S")
        .arg("--reverse")
        .arg("letter")
        .arg("b")
        .arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["b"], svec!["b"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S")
        .arg("--reverse")
        .arg("letter")
        .arg("a")
        .arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["a"], svec!["a"], svec!["a"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-S")
        .arg("--reverse")
        .arg("letter")
        .arg("d")
        .arg("letters.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![svec!["letter"], svec!["d"]];
    assert_eq!(got, expected);
}

#[test]
fn bisect_all() {
    let wrk = Workdir::new("bisect_all");
    let data = vec![
        svec!["name", "surname"],
        svec!["amelie", "earlhart"],
        svec!["berenice", "bejo"],
        svec!["carol", "denvers"],
        svec!["dominique", "boutin"],
        svec!["ereven", "nijoul"],
        svec!["fareed", "hakmad"],
        svec!["guillaume", "loris"],
        svec!["horatio", "caine"],
    ];

    wrk.create("data.csv", data.clone());

    for row in data.iter().skip(1) {
        let mut cmd = wrk.command("bisect");
        cmd.arg("name").arg(&row[0]).arg("-S").arg("data.csv");

        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = vec![svec!["name", "surname"], row.clone()];
        assert_eq!(got, expected);
    }
}

#[test]
fn bisect_no_headers() {
    let wrk = Workdir::new("bisect_no_headers");

    wrk.create(
        "data.csv",
        vec![
            svec!["amelie", "earlhart"],
            svec!["berenice", "bejo"],
            svec!["carol", "denvers"],
            svec!["dominique", "boutin"],
            svec!["ereven", "nijoul"],
            svec!["fareed", "hakmad"],
            svec!["guillaume", "loris"],
            svec!["horatio", "caine"],
        ],
    );

    let mut cmd = wrk.command("bisect");
    cmd.arg("-n")
        .arg("0")
        .arg("fareed")
        .arg("-S")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["fareed", "hakmad"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("bisect");
    cmd.arg("-n").arg("0").arg("fo").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["guillaume", "loris"], ["horatio", "caine"]];
    assert_eq!(got, expected);
}
