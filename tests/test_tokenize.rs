use crate::workdir::Workdir;

#[test]
fn tokenize() {
    let wrk = Workdir::new("tokenize");
    wrk.create(
        "data.csv",
        vec![
            svec!["n", "text"],
            svec!["1", "le chat mange"],
            svec!["2", "la souris"],
            svec!["3", ""],
        ],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "token"],
        svec!["1", "le"],
        svec!["1", "chat"],
        svec!["1", "mange"],
        svec!["2", "la"],
        svec!["2", "souris"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_sep() {
    let wrk = Workdir::new("tokenize_sep");
    wrk.create(
        "data.csv",
        vec![
            svec!["n", "text"],
            svec!["1", "le chat mange"],
            svec!["2", "la souris"],
            svec!["3", ""],
        ],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text").args(["--sep", "|"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "text", "tokens"],
        svec!["1", "le chat mange", "le|chat|mange"],
        svec!["2", "la souris", "la|souris"],
        svec!["3", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_keep_text() {
    let wrk = Workdir::new("tokenize_keep_text");
    wrk.create(
        "data.csv",
        vec![
            svec!["n", "text"],
            svec!["1", "le chat mange"],
            svec!["2", "la souris"],
            svec!["3", ""],
        ],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text").arg("--keep-text").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "text", "token"],
        svec!["1", "le chat mange", "le"],
        svec!["1", "le chat mange", "chat"],
        svec!["1", "le chat mange", "mange"],
        svec!["2", "la souris", "la"],
        svec!["2", "la souris", "souris"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_column() {
    let wrk = Workdir::new("tokenize_column");
    wrk.create(
        "data.csv",
        vec![
            svec!["n", "text"],
            svec!["1", "le chat mange"],
            svec!["2", "la souris"],
            svec!["3", ""],
        ],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text").args(["-c", "word"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "word"],
        svec!["1", "le"],
        svec!["1", "chat"],
        svec!["1", "mange"],
        svec!["2", "la"],
        svec!["2", "souris"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_token_type() {
    let wrk = Workdir::new("tokenize_token_type");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "1 chat mange 😎"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text").args(["-T", "type"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "token", "type"],
        svec!["1", "1", "number"],
        svec!["1", "chat", "word"],
        svec!["1", "mange", "word"],
        svec!["1", "😎", "emoji"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_parallel() {
    let wrk = Workdir::new("tokenize_parallel");
    wrk.create(
        "data.csv",
        vec![
            svec!["n", "text"],
            svec!["1", "le chat mange"],
            svec!["2", "la souris"],
            svec!["3", ""],
        ],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "token"],
        svec!["1", "le"],
        svec!["1", "chat"],
        svec!["1", "mange"],
        svec!["2", "la"],
        svec!["2", "souris"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_drop() {
    let wrk = Workdir::new("tokenize_drop");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "1 chat 😎"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text")
        .args(["--drop", "number,emoji"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "token"], svec!["1", "chat"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_keep() {
    let wrk = Workdir::new("tokenize_keep");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "1 chat 😎"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text")
        .args(["--keep", "number,emoji"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "token"], svec!["1", "1"], svec!["1", "😎"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_min_token_len() {
    let wrk = Workdir::new("tokenize_min_token_len");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chaton"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text")
        .args(["--min-token-len", "3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "token"], svec!["1", "chaton"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_max_token_len() {
    let wrk = Workdir::new("tokenize_max_token_len");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chaton"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("text")
        .args(["--max-token-len", "3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "token"], svec!["1", "le"]];
    assert_eq!(got, expected);
}
