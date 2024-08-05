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
