use crate::workdir::Workdir;

fn people() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["0", "alice"],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["7", "dave"],
    ]
}

#[test]
fn test_complete_basic() {
    let wrk = Workdir::new("complete_basic");
    wrk.create("indexes.csv", people());
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

#[test]
fn test_complete_with_min_max() {
    let wrk = Workdir::new("complete_with_min_max");
    wrk.create("indexes.csv", people());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes.csv")
        .arg("-m")
        .arg("-2")
        .arg("-M")
        .arg("8");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["-2", ""],
        svec!["-1", ""],
        svec!["0", "alice"],
        svec!["1", ""],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", ""],
        svec!["5", ""],
        svec!["6", ""],
        svec!["7", "dave"],
        svec!["8", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn test_complete_with_zero_value() {
    let wrk = Workdir::new("complete_with_zero_value");
    wrk.create("indexes.csv", people());
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes.csv").arg("-z").arg("MISSING");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["0", "alice"],
        svec!["1", "MISSING"],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", "MISSING"],
        svec!["5", "MISSING"],
        svec!["6", "MISSING"],
        svec!["7", "dave"],
    ];
    assert_eq!(got, expected);
}
