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
    cmd.arg("corpus").arg("doc").arg("token").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["doc_count", "token_count", "average_doc_len"],
        svec!["2", "3", "2.5"],
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
    cmd.arg("doc").arg("doc").arg("token").arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec!["doc", "token_count", "distinct_token_count"],
        svec!["1", "3", "2"],
        svec!["2", "2", "2"],
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
    cmd.arg("token").arg("doc").arg("token").arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec!["token", "gf", "df", "idf", "gfidf", "pigeonhole"],
        svec!["cat", "3", "2", "0", "0", "1.1428571428571428"],
        svec![
            "dog",
            "1",
            "1",
            "0.6931471805599453",
            "0.6931471805599453",
            "1"
        ],
        svec![
            "rabbit",
            "1",
            "1",
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
    cmd.arg("doc-token").arg("doc").arg("token").arg("data.csv");

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got[1..].sort();

    let expected = vec![
        svec!["doc", "token", "tf", "tfidf", "bm25"],
        svec!["1", "cat", "2", "0", "0"],
        svec!["1", "dog", "1", "0.6931471805599453", "0.64072428455121"],
        svec!["2", "cat", "1", "0", "0"],
        svec![
            "2",
            "rabbit",
            "1",
            "0.6931471805599453",
            "0.7549127709068711"
        ],
    ];
    assert_eq!(got, expected);
}
