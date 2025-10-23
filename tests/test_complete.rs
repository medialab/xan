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

fn dates() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-01", "event1"],
        svec!["2025-03", "event2"],
        svec!["2025-06", "event3"],
    ]
}

#[test]
#[should_panic]
fn test_complete_sorted_check_panic() {
    let wrk = Workdir::new("complete_sorted_check_panic");
    wrk.create("indexes.csv", people());
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes.csv").arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn test_complete_sorted_check_panic_min() {
    let wrk = Workdir::new("complete_sorted_check_panic_min");
    wrk.create(
        "indexes.csv",
        vec![
            svec!["id", "name"],
            svec!["0", "alice"],
            svec!["1", "bob"],
            svec!["2", "charlie"],
            svec!["3", "dave"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes.csv")
        .arg("--check")
        .arg("-m")
        .arg("-1");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn test_complete_sorted_check_panic_max() {
    let wrk = Workdir::new("complete_sorted_check_panic_max");
    wrk.create(
        "indexes.csv",
        vec![
            svec!["id", "name"],
            svec!["0", "alice"],
            svec!["1", "bob"],
            svec!["2", "charlie"],
            svec!["3", "dave"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes.csv")
        .arg("--check")
        .arg("-M")
        .arg("5");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn test_complete_sorted_check() {
    let wrk = Workdir::new("complete_sorted_check");
    wrk.create(
        "indexes_complete.csv",
        vec![
            svec!["id", "name"],
            svec!["0", "alice"],
            svec!["1", "bob"],
            svec!["2", "charlie"],
            svec!["3", "dave"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes_complete.csv").arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);
}

#[test]
fn test_complete_sorted_check_min_max() {
    let wrk = Workdir::new("complete_sorted_check_min_max");
    wrk.create("indexes_complete.csv", people());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_complete.csv")
        .arg("--check")
        .arg("-m")
        .arg("2")
        .arg("-M")
        .arg("3");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn test_complete_sorted_check_dates_panic() {
    let wrk = Workdir::new("complete_sorted_check_dates_panic");
    wrk.create("dates_incomplete.csv", dates());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_incomplete.csv")
        .arg("--dates")
        .arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn test_complete_sorted_check_dates_panic_min() {
    let wrk = Workdir::new("complete_sorted_check_dates_panic_min");
    wrk.create(
        "dates_incomplete.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-02", "event1"],
            svec!["2025-03", "event2"],
            svec!["2025-04", "event3"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_incomplete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-m")
        .arg("2025-01");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn test_complete_sorted_check_dates_panic_max() {
    let wrk = Workdir::new("complete_sorted_check_dates_panic_max");
    wrk.create(
        "dates_incomplete.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-02", "event1"],
            svec!["2025-03", "event2"],
            svec!["2025-04", "event3"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_incomplete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-M")
        .arg("2025-06");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn test_complete_sorted_check_dates() {
    let wrk = Workdir::new("complete_sorted_check_dates");
    wrk.create(
        "dates_complete.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-01", "event1"],
            svec!["2025-02", "event2"],
            svec!["2025-03", "event3"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_complete.csv")
        .arg("--dates")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);
}

#[test]
fn test_complete_sorted_check_dates_min_max() {
    let wrk = Workdir::new("complete_sorted_check_dates_min_max");
    wrk.create(
        "dates_complete.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-02", "event1"],
            svec!["2025-03", "event2"],
            svec!["2025-04", "event3"],
            svec!["2025-05", "event4"],
            svec!["2025-06", "event5"],
            svec!["2025-07", "event6"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_complete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-m")
        .arg("2025-03")
        .arg("-M")
        .arg("2025-05");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);
}

#[test]
fn test_complete_sorted_basic() {
    let wrk = Workdir::new("complete_sorted_basic");
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
fn test_complete_sorted_with_min_max() {
    let wrk1 = Workdir::new("complete_sorted_with_min_max");
    wrk1.create("indexes.csv", people());
    let mut cmd1 = wrk1.command("complete");
    cmd1.arg("id")
        .arg("indexes.csv")
        .arg("-m")
        .arg("-2")
        .arg("-M")
        .arg("8");
    let got1: Vec<Vec<String>> = wrk1.read_stdout(&mut cmd1);
    let expected1 = vec![
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
    assert_eq!(got1, expected1);

    let wrk2 = Workdir::new("complete_sorted_with_min_max_dropping");
    wrk2.create("indexes.csv", people());
    let mut cmd2 = wrk2.command("complete");
    cmd2.arg("id")
        .arg("indexes.csv")
        .arg("-m")
        .arg("1")
        .arg("-M")
        .arg("5");
    let got2: Vec<Vec<String>> = wrk2.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["id", "name"],
        svec!["1", ""],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", ""],
        svec!["5", ""],
    ];
    assert_eq!(got2, expected2);
}

#[test]
fn test_complete_sorted_with_zero_value() {
    let wrk = Workdir::new("complete_sorted_with_zero_value");
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

#[test]
fn test_complete_sorted_dates() {
    let wrk = Workdir::new("complete_sorted_dates");
    wrk.create("dates_incomplete.csv", dates());
    let mut cmd = wrk.command("complete");
    cmd.arg("date").arg("dates_incomplete.csv").arg("--dates");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["date", "event"],
        svec!["2025-01", "event1"],
        svec!["2025-02", ""],
        svec!["2025-03", "event2"],
        svec!["2025-04", ""],
        svec!["2025-05", ""],
        svec!["2025-06", "event3"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn test_complete_sorted_dates_with_min_max() {
    let wrk1 = Workdir::new("complete_sorted_dates_with_min_max");
    wrk1.create("dates_incomplete.csv", dates());
    let mut cmd1 = wrk1.command("complete");
    cmd1.arg("date")
        .arg("dates_incomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2024-11")
        .arg("-M")
        .arg("2025-08");
    let got1: Vec<Vec<String>> = wrk1.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["date", "event"],
        svec!["2024-11", ""],
        svec!["2024-12", ""],
        svec!["2025-01", "event1"],
        svec!["2025-02", ""],
        svec!["2025-03", "event2"],
        svec!["2025-04", ""],
        svec!["2025-05", ""],
        svec!["2025-06", "event3"],
        svec!["2025-07", ""],
        svec!["2025-08", ""],
    ];
    assert_eq!(got1, expected1);

    let wrk2 = Workdir::new("complete_sorted_dates_with_min_max_dropping");
    wrk2.create("dates_incomplete.csv", dates());
    let mut cmd2 = wrk2.command("complete");
    cmd2.arg("date")
        .arg("dates_incomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2025-02")
        .arg("-M")
        .arg("2025-05");
    let got2: Vec<Vec<String>> = wrk2.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["date", "event"],
        svec!["2025-02", ""],
        svec!["2025-03", "event2"],
        svec!["2025-04", ""],
        svec!["2025-05", ""],
    ];
    assert_eq!(got2, expected2);
}
