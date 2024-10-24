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
        .arg("token")
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
        .arg("token")
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
    cmd.arg("doc")
        .args(["--sep", "|"])
        .arg("tokens")
        .arg("data.csv");

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
        .args(["--doc", "doc"])
        .arg("token")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec![
            "token",
            "gf",
            "df",
            "df_ratio",
            "idf",
            "gfidf",
            "pigeonhole"
        ],
        svec!["cat", "3", "2", "1", "0", "0", "1.1428571428571428"],
        svec![
            "dog",
            "1",
            "1",
            "0.5",
            "0.6931471805599453",
            "0.6931471805599453",
            "1"
        ],
        svec![
            "rabbit",
            "1",
            "1",
            "0.5",
            "0.6931471805599453",
            "0.6931471805599453",
            "1"
        ],
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
    cmd.arg("token")
        .args(["--sep", "|"])
        .arg("tokens")
        .arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec![
            "token",
            "gf",
            "df",
            "df_ratio",
            "idf",
            "gfidf",
            "pigeonhole"
        ],
        svec!["cat", "3", "2", "1", "0", "0", "1.1428571428571428"],
        svec![
            "dog",
            "1",
            "1",
            "0.5",
            "0.6931471805599453",
            "0.6931471805599453",
            "1"
        ],
        svec![
            "rabbit",
            "1",
            "1",
            "0.5",
            "0.6931471805599453",
            "0.6931471805599453",
            "1"
        ],
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
        .args(["--doc", "doc"])
        .arg("token")
        .arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec!["doc", "token", "tf", "tfidf", "bm25", "chi2"],
        svec!["1", "cat", "2", "0", "0", "0.007407407407407405"],
        svec![
            "1",
            "dog",
            "1",
            "0.6931471805599453",
            "0.64072428455121",
            "0.08888888888888885"
        ],
        svec!["2", "cat", "1", "0", "0", "0.01666666666666666"],
        svec![
            "2",
            "rabbit",
            "1",
            "0.6931471805599453",
            "0.7549127709068711",
            "0.44999999999999996"
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
    cmd.arg("cooc")
        .args(["--sep", "|"])
        .arg("tokens")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["token1", "token2", "count", "chi2", "G2", "pmi", "ppmi", "npmi"],
        svec![
            "cat",
            "cat",
            "1",
            "2.25",
            "-2.772588722239781",
            "-2",
            "0",
            "-1"
        ],
        svec!["cat", "dog", "2", "0", "0", "0", "0", "0"],
        svec!["cat", "rabbit", "1", "0", "0", "0", "0", "0"],
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
        .arg("token")
        .args(["--doc", "doc"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["token1", "token2", "count", "chi2", "G2", "pmi", "ppmi", "npmi"],
        svec![
            "cat",
            "cat",
            "1",
            "2.25",
            "-2.772588722239781",
            "-2",
            "0",
            "-1"
        ],
        svec!["cat", "dog", "2", "0", "0", "0", "0", "0"],
        svec!["cat", "rabbit", "1", "0", "0", "0", "0", "0"],
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
        .arg("token")
        .args(["--doc", "doc"])
        .args(["-w", "10"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);

    let expected = vec![
        svec!["token1", "token2", "count", "chi2", "G2", "pmi", "ppmi", "npmi"],
        svec![
            "cat",
            "cat",
            "1",
            "2.25",
            "-2.772588722239781",
            "-2",
            "0",
            "-1"
        ],
        svec!["cat", "dog", "2", "0", "0", "0", "0", "0"],
        svec!["cat", "rabbit", "1", "0", "0", "0", "0", "0"],
    ];
    assert_eq!(got, expected);
}
