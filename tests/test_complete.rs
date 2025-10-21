use crate::workdir::Workdir;

#[test]
fn test_complete_basic() {
    let wrk = Workdir::new("complete_basic");
    wrk.create(
        "indexes.csv",
        vec![
            svec!["id", "name"],
            svec!["0", "alice"],
            svec!["2", "bob"],
            svec!["3", "charlie"],
            svec!["7", "dave"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["0", "alice"],
        svec!["1", ""],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", ""],
        svec!["5", ""],
        svec!["6", ""],
        svec!["7", "dave"],
    ];
    assert_eq!(got, expected);
}
