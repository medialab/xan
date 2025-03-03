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
    cmd.arg("words").arg("text").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "tokens"],
        svec!["1", "le chat mange"],
        svec!["2", "la souris"],
        svec!["3", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_simple() {
    let wrk = Workdir::new("tokenize_simple");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "aujourd'hui"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("words").arg("text").arg("-S").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "aujourd hui"]];
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
    cmd.arg("words")
        .arg("text")
        .args(["--sep", "|"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "tokens"],
        svec!["1", "le|chat|mange"],
        svec!["2", "la|souris"],
        svec!["3", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_types() {
    let wrk = Workdir::new("tokenize_types");
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
    cmd.arg("words")
        .arg("text")
        .args(["-T", "type"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "token", "type"],
        svec!["1", "le", "word"],
        svec!["1", "chat", "word"],
        svec!["1", "mange", "word"],
        svec!["2", "la", "word"],
        svec!["2", "souris", "word"],
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
    cmd.arg("words")
        .arg("--keep-text")
        .arg("text")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "text", "tokens"],
        svec!["1", "le chat mange", "le chat mange"],
        svec!["2", "la souris", "la souris"],
        svec!["3", "", ""],
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
    cmd.arg("words")
        .args(["-c", "words"])
        .arg("text")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "words"],
        svec!["1", "le chat mange"],
        svec!["2", "la souris"],
        svec!["3", ""],
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
    cmd.arg("words").arg("text").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "tokens"],
        svec!["1", "le chat mange"],
        svec!["2", "la souris"],
        svec!["3", ""],
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
    cmd.arg("words")
        .arg("text")
        .args(["--drop", "number,emoji"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "chat"]];
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
    cmd.arg("words")
        .arg("text")
        .args(["--keep", "number,emoji"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "1 😎"]];
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
    cmd.arg("words")
        .arg("text")
        .args(["--min-token", "3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "chaton"]];
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
    cmd.arg("words")
        .arg("text")
        .args(["--max-token", "3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "le"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_stoplist() {
    let wrk = Workdir::new("tokenize_stoplist");
    wrk.create("stoplist.txt", vec![svec!["le"], svec!["la"]]);
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chaton mange la souris"]],
    );

    let mut cmd = wrk.command("tokenize");
    cmd.arg("words")
        .arg("text")
        .args(["--stoplist", "stoplist.txt"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "chaton mange souris"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_ngrams() {
    let wrk = Workdir::new("tokenize_ngrams");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chat mange"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("words")
        .arg("text")
        .args(["--ngrams", "2"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "le§chat chat§mange"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_ngrams_sep() {
    let wrk = Workdir::new("tokenize_ngrams_sep");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chat mange"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("words")
        .arg("text")
        .args(["--ngrams", "2"])
        .args(["--ngrams-sep", "|"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "le|chat chat|mange"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_ngrams_range() {
    let wrk = Workdir::new("tokenize_ngrams_range");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chat mange"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("words")
        .arg("text")
        .args(["--ngrams", "1,2"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "tokens"],
        svec!["1", "le chat le§chat mange chat§mange"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_ngrams_parallel() {
    let wrk = Workdir::new("tokenize_ngrams_parallel");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "le chat mange"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("words")
        .arg("text")
        .arg("-p")
        .args(["--ngrams", "2"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "tokens"], svec!["1", "le§chat chat§mange"]];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_paragraphs() {
    let wrk = Workdir::new("tokenize_paragraphs");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "Hello\n\nBonjour"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("paragraphs").arg("text").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "paragraph"],
        svec!["1", "Hello"],
        svec!["1", "Bonjour"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_paragraphs_parallel() {
    let wrk = Workdir::new("tokenize_paragraphs_parallel");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "Hello\n\nBonjour"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("paragraphs").arg("text").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "paragraph"],
        svec!["1", "Hello"],
        svec!["1", "Bonjour"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_sentences() {
    let wrk = Workdir::new("tokenize_sentences");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "Bonjour. Je suis John!"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("sentences").arg("text").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "sentence"],
        svec!["1", "Bonjour."],
        svec!["1", "Je suis John!"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_sentences_parallel() {
    let wrk = Workdir::new("tokenize_sentences_parallel");
    wrk.create(
        "data.csv",
        vec![svec!["n", "text"], svec!["1", "Bonjour. Je suis John!"]],
    );
    let mut cmd = wrk.command("tokenize");
    cmd.arg("sentences").arg("text").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "sentence"],
        svec!["1", "Bonjour."],
        svec!["1", "Je suis John!"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn tokenize_flatmap() {
    let wrk = Workdir::new("tokenize_flatmap");
    wrk.create("data.csv", vec![svec!["text"], svec!["a 1, b"]]);

    let mut cmd = wrk.command("tokenize");
    cmd.arg("words")
        .arg("text")
        .args([
            "-F",
            "if(token_type eq 'word', upper(token), if(token_type eq 'punct', null, token + 3))",
        ])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["tokens"], svec!["A 4 B"]];
    assert_eq!(got, expected);
}
