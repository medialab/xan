use crate::workdir::Workdir;

#[test]
fn vocab_corpus() {
    let wrk = Workdir::new("vocab_corpus");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "token"],
            svec!["1", "cat"],
            svec!["1", "dog"],
            svec!["1", "cat"],
            svec!["2", "cat"],
            svec!["2", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("corpus")
        .args(["--doc", "doc"])
        .arg("--implode")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec![
            "doc_count",
            "token_count",
            "distinct_token_count",
            "average_doc_len"
        ],
        svec!["2", "5", "3", "2.5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_corpus_token() {
    let wrk = Workdir::new("vocab_corpus_token");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "word"],
            svec!["1", "cat"],
            svec!["1", "dog"],
            svec!["1", "cat"],
            svec!["2", "cat"],
            svec!["2", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("corpus")
        .args(["--doc", "doc"])
        .arg("--implode")
        .args(["-T", "word"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec![
            "doc_count",
            "token_count",
            "distinct_token_count",
            "average_doc_len"
        ],
        svec!["2", "5", "3", "2.5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_doc() {
    let wrk = Workdir::new("vocab_doc");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "token"],
            svec!["1", "cat"],
            svec!["1", "dog"],
            svec!["1", "cat"],
            svec!["2", "cat"],
            svec!["2", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("doc")
        .args(["--doc", "doc"])
        .arg("--implode")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["doc", "token_count", "distinct_token_count"],
        svec!["1", "3", "2"],
        svec!["2", "2", "2"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_doc_sep() {
    let wrk = Workdir::new("vocab_doc_sep");
    wrk.create(
        "data.csv",
        vec![svec!["tokens"], svec!["cat|dog|cat"], svec!["cat|rabbit"]],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("doc").args(["--sep", "|"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["doc", "token_count", "distinct_token_count"],
        svec!["0", "3", "2"],
        svec!["1", "2", "2"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_token() {
    let wrk = Workdir::new("vocab_token");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "token"],
            svec!["1", "cat"],
            svec!["1", "dog"],
            svec!["1", "cat"],
            svec!["2", "cat"],
            svec!["2", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("token")
        .arg("--implode")
        .args(["--doc", "doc"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["token", "gf", "df", "df_ratio", "idf", "gfidf", "pigeon"],
        svec!["cat", "3", "2", "1", "0", "3", "0.875"],
        svec!["dog", "1", "1", "0.5", "0.6931471805599453", "2", "1"],
        svec!["rabbit", "1", "1", "0.5", "0.6931471805599453", "2", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_token_sep() {
    let wrk = Workdir::new("vocab_token_sep");
    wrk.create(
        "data.csv",
        vec![svec!["tokens"], svec!["cat|dog|cat"], svec!["cat|rabbit"]],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("token").args(["--sep", "|"]).arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec!["token", "gf", "df", "df_ratio", "idf", "gfidf", "pigeon"],
        svec!["cat", "3", "2", "1", "0", "3", "0.875"],
        svec!["dog", "1", "1", "0.5", "0.6931471805599453", "2", "1"],
        svec!["rabbit", "1", "1", "0.5", "0.6931471805599453", "2", "1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_doc_token() {
    let wrk = Workdir::new("vocab_doc_token");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "token"],
            svec!["1", "cat"],
            svec!["1", "dog"],
            svec!["1", "cat"],
            svec!["2", "cat"],
            svec!["2", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("doc-token")
        .arg("--implode")
        .args(["--doc", "doc"])
        .arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec!["doc", "token", "tf", "expected_tf", "tfidf", "bm25", "chi2"],
        svec!["1", "cat", "2", "1.8", "0", "0", "0.13888888888888884"],
        svec![
            "1",
            "dog",
            "1",
            "0.6",
            "0.6931471805599453",
            "0.64072428455121",
            "0.8333333333333334"
        ],
        svec!["2", "cat", "1", "1.2", "0", "0", ""],
        svec![
            "2",
            "rabbit",
            "1",
            "0.4",
            "0.6931471805599453",
            "0.7549127709068711",
            "1.8750000000000002"
        ],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_cooc_sep_no_doc() {
    let wrk = Workdir::new("vocab_cooc_sep_no_doc");
    wrk.create(
        "data.csv",
        vec![svec!["tokens"], svec!["cat|dog|cat"], svec!["cat|rabbit"]],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("cooc").args(["--sep", "|"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec![
            "token1",
            "token2",
            "count",
            "expected_count",
            "chi2",
            "G2",
            "pmi",
            "npmi"
        ],
        svec![
            "cat",
            "cat",
            "1",
            "4",
            "",
            "-1.3862943611198906",
            "-2",
            "-1"
        ],
        svec!["cat", "dog", "2", "2", "0", "0", "0", "0"],
        svec!["cat", "rabbit", "1", "1", "0", "0", "0", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_cooc_no_sep() {
    let wrk = Workdir::new("vocab_cooc_no_sep");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "token"],
            svec!["one", "cat"],
            svec!["one", "dog"],
            svec!["one", "cat"],
            svec!["two", "cat"],
            svec!["two", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("cooc")
        .arg("--implode")
        .args(["--doc", "doc"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec![
            "token1",
            "token2",
            "count",
            "expected_count",
            "chi2",
            "G2",
            "pmi",
            "npmi"
        ],
        svec![
            "cat",
            "cat",
            "1",
            "4",
            "",
            "-1.3862943611198906",
            "-2",
            "-1"
        ],
        svec!["cat", "dog", "2", "2", "0", "0", "0", "0"],
        svec!["cat", "rabbit", "1", "1", "0", "0", "0", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn vocab_cooc_no_sep_window() {
    let wrk = Workdir::new("vocab_cooc_no_sep_window");
    wrk.create(
        "data.csv",
        vec![
            svec!["doc", "token"],
            svec!["one", "cat"],
            svec!["one", "dog"],
            svec!["one", "cat"],
            svec!["two", "cat"],
            svec!["two", "rabbit"],
        ],
    );
    let mut cmd = wrk.command("vocab");
    cmd.arg("cooc")
        .arg("--implode")
        .args(["--doc", "doc"])
        .args(["-w", "10"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec![
            "token1",
            "token2",
            "count",
            "expected_count",
            "chi2",
            "G2",
            "pmi",
            "npmi"
        ],
        svec![
            "cat",
            "cat",
            "1",
            "4",
            "",
            "-1.3862943611198906",
            "-2",
            "-1"
        ],
        svec!["cat", "dog", "2", "2", "0", "0", "0", "0"],
        svec!["cat", "rabbit", "1", "1", "0", "0", "0", "0"],
    ];
    assert_eq!(got, expected);
}
