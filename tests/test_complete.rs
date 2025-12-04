use crate::workdir::Workdir;

fn people_sorted_uncomplete() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["0", "alice"],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["7", "dave"],
    ]
}

fn people_sorted_uncomplete_reverse() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["7", "dave"],
        svec!["3", "charlie"],
        svec!["2", "bob"],
        svec!["0", "alice"],
    ]
}

fn people_unsorted_uncomplete() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["3", "charlie"],
        svec!["0", "alice"],
        svec!["7", "dave"],
        svec!["2", "bob"],
    ]
}

fn people_sorted_complete() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["0", "alice"],
        svec!["1", "bob"],
        svec!["2", "charlie"],
        svec!["3", "dave"],
    ]
}

fn people_sorted_complete_reverse() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["3", "dave"],
        svec!["2", "charlie"],
        svec!["1", "bob"],
        svec!["0", "alice"],
    ]
}

fn people_unsorted_complete() -> Vec<Vec<String>> {
    vec![
        svec!["id", "name"],
        svec!["3", "dave"],
        svec!["0", "alice"],
        svec!["2", "charlie"],
        svec!["1", "bob"],
    ]
}

fn dates_sorted_uncomplete() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-01", "event1"],
        svec!["2025-03", "event2"],
        svec!["2025-06", "event3"],
    ]
}

fn dates_sorted_uncomplete_reverse() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-06", "event3"],
        svec!["2025-03", "event2"],
        svec!["2025-01", "event1"],
    ]
}

fn dates_unsorted_uncomplete() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-06", "event3"],
        svec!["2025-01", "event1"],
        svec!["2025-03", "event2"],
    ]
}

fn dates_sorted_complete() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-02", "event1"],
        svec!["2025-03", "event2"],
        svec!["2025-04", "event3"],
    ]
}

fn dates_sorted_complete_reverse() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-04", "event3"],
        svec!["2025-03", "event2"],
        svec!["2025-02", "event1"],
    ]
}

fn dates_unsorted_complete() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-04", "event3"],
        svec!["2025-03", "event2"],
        svec!["2025-02", "event1"],
    ]
}

fn dates_sorted_almost_complete() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-01", "event1"],
        svec!["2025-03", "event2"],
        svec!["2025-04", "event3"],
        svec!["2025-05", "event4"],
        svec!["2025-07", "event5"],
    ]
}

fn dates_sorted_almost_complete_reverse() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-07", "event5"],
        svec!["2025-05", "event4"],
        svec!["2025-04", "event3"],
        svec!["2025-03", "event2"],
        svec!["2025-01", "event1"],
    ]
}

fn dates_unsorted_almost_complete() -> Vec<Vec<String>> {
    vec![
        svec!["date", "event"],
        svec!["2025-05", "event4"],
        svec!["2025-01", "event1"],
        svec!["2025-04", "event3"],
        svec!["2025-07", "event5"],
        svec!["2025-03", "event2"],
    ]
}

#[test]
fn complete() {
    let wrk = Workdir::new("complete");
    wrk.create("indexes_sorted.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes_sorted.csv");
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

    wrk.create("indexes_unsorted.csv", people_unsorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes_unsorted.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes_sorted.csv").arg("--reverse");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["7", "dave"],
        svec!["6", ""],
        svec!["5", ""],
        svec!["4", ""],
        svec!["3", "charlie"],
        svec!["2", "bob"],
        svec!["1", ""],
        svec!["0", "alice"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes_unsorted.csv").arg("--reverse");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
#[should_panic(expected = "Invalid integer format: 2025-01-02")]
fn complete_type_mismatch() {
    let wrk = Workdir::new("complete_type_mismatch");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "name"],
            svec!["2", "bob"],
            svec!["2025-01-02", "alice"],
            svec!["3", "charlie"],
            svec!["7", "dave"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("data.csv");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_sorted() {
    let wrk = Workdir::new("complete_sorted");
    wrk.create("indexes.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted").arg("id").arg("indexes.csv");
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

    wrk.create("indexes_reverse.csv", people_sorted_uncomplete_reverse());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("--reverse")
        .arg("id")
        .arg("indexes_reverse.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["7", "dave"],
        svec!["6", ""],
        svec!["5", ""],
        svec!["4", ""],
        svec!["3", "charlie"],
        svec!["2", "bob"],
        svec!["1", ""],
        svec!["0", "alice"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn complete_with_min_max() {
    let wrk = Workdir::new("complete_with_min_max");
    wrk.create("indexes_sorted.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_sorted.csv")
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

    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("--reverse")
        .arg("indexes_sorted.csv")
        .arg("-m")
        .arg("-2")
        .arg("-M")
        .arg("8");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["8", ""],
        svec!["7", "dave"],
        svec!["6", ""],
        svec!["5", ""],
        svec!["4", ""],
        svec!["3", "charlie"],
        svec!["2", "bob"],
        svec!["1", ""],
        svec!["0", "alice"],
        svec!["-1", ""],
        svec!["-2", ""],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_sorted.csv")
        .arg("-m")
        .arg("1")
        .arg("-M")
        .arg("5");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["1", ""],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", ""],
        svec!["5", ""],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("--reverse")
        .arg("indexes_sorted.csv")
        .arg("-m")
        .arg("1")
        .arg("-M")
        .arg("5");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["5", ""],
        svec!["4", ""],
        svec!["3", "charlie"],
        svec!["2", "bob"],
        svec!["1", ""],
    ];
    assert_eq!(got, expected);

    wrk.create("indexes_unsorted.csv", people_unsorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_unsorted.csv")
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

    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_unsorted.csv")
        .arg("-m")
        .arg("1")
        .arg("-M")
        .arg("5");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["1", ""],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", ""],
        svec!["5", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn complete_sorted_with_min_max() {
    let wrk = Workdir::new("complete_sorted_with_min_max");
    wrk.create("indexes.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("id")
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

    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("--sorted")
        .arg("indexes.csv")
        .arg("-m")
        .arg("1")
        .arg("-M")
        .arg("5");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["1", ""],
        svec!["2", "bob"],
        svec!["3", "charlie"],
        svec!["4", ""],
        svec!["5", ""],
    ];
    assert_eq!(got, expected);

    wrk.create("indexes_reverse.csv", people_sorted_uncomplete_reverse());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("--reverse")
        .arg("id")
        .arg("indexes_reverse.csv")
        .arg("-m")
        .arg("-2")
        .arg("-M")
        .arg("8");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "name"],
        svec!["8", ""],
        svec!["7", "dave"],
        svec!["6", ""],
        svec!["5", ""],
        svec!["4", ""],
        svec!["3", "charlie"],
        svec!["2", "bob"],
        svec!["1", ""],
        svec!["0", "alice"],
        svec!["-1", ""],
        svec!["-2", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
#[should_panic(expected = "Invalid integer format: 2025-01")]
fn complete_with_min_max_type_mismatch() {
    let wrk = Workdir::new("complete_with_min_max_type_mismatch");
    wrk.create("indexes_sorted.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_sorted.csv")
        .args(&["-m", "2025-01"])
        .args(&["-M", "8"]);
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_dates() {
    let wrk = Workdir::new("complete_dates");
    wrk.create("dates_sorted_uncomplete.csv", dates_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_uncomplete.csv")
        .arg("--dates");
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

    wrk.create("dates_unsorted_uncomplete.csv", dates_unsorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_unsorted_uncomplete.csv")
        .arg("--dates");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_uncomplete.csv")
        .arg("--dates")
        .arg("--reverse");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["date", "event"],
        svec!["2025-06", "event3"],
        svec!["2025-05", ""],
        svec!["2025-04", ""],
        svec!["2025-03", "event2"],
        svec!["2025-02", ""],
        svec!["2025-01", "event1"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_unsorted_uncomplete.csv")
        .arg("--dates")
        .arg("--reverse");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
#[should_panic(
    expected = "Inconsistent value units: first seen was Date(Day) and then found Date(Month)"
)]
fn complete_dates_unit_mismatch() {
    let wrk = Workdir::new("complete_dates_unit_mismatch");
    wrk.create(
        "data.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-05", "event4"],
            svec!["2025-01-02", "event1"],
            svec!["2025-04", "event3"],
            svec!["2025-07", "event5"],
            svec!["2025-03", "event2"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date").arg("data.csv").arg("--dates");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_sorted_dates() {
    let wrk = Workdir::new("complete_sorted_dates");
    wrk.create("dates_uncomplete.csv", dates_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates");
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
#[should_panic(
    expected = "Inconsistent value units: first seen was Date(Month) and then found Date(Day)"
)]
fn complete_sorted_dates_unit_mismatch() {
    let wrk = Workdir::new("complete_sorted_dates_unit_mismatch");
    wrk.create(
        "data.csv",
        vec![
            svec!["date", "event"],
            svec!["2024-05", "event4"],
            svec!["2025-01-02", "event1"],
            svec!["2025-04", "event3"],
            svec!["2025-07", "event5"],
            svec!["2025-12", "event2"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("data.csv")
        .arg("--dates");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_dates_with_min_max() {
    let wrk = Workdir::new("complete_dates_with_min_max");
    wrk.create("dates_sorted_uncomplete.csv", dates_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2024-11")
        .arg("-M")
        .arg("2025-08");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
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
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2025-02")
        .arg("-M")
        .arg("2025-05");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["date", "event"],
        svec!["2025-02", ""],
        svec!["2025-03", "event2"],
        svec!["2025-04", ""],
        svec!["2025-05", ""],
    ];
    assert_eq!(got, expected);

    wrk.create("dates_unsorted_uncomplete.csv", dates_unsorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_unsorted_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2024-11")
        .arg("-M")
        .arg("2025-08");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
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
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_unsorted_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2025-02")
        .arg("-M")
        .arg("2025-05");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["date", "event"],
        svec!["2025-02", ""],
        svec!["2025-03", "event2"],
        svec!["2025-04", ""],
        svec!["2025-05", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn complete_sorted_dates_with_min_max() {
    let wrk = Workdir::new("complete_sorted_dates_with_min_max");
    wrk.create("dates_uncomplete.csv", dates_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("--sorted")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2024-11")
        .arg("-M")
        .arg("2025-08");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
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
    assert_eq!(got, expected);

    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("--sorted")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2025-02")
        .arg("-M")
        .arg("2025-05");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["date", "event"],
        svec!["2025-02", ""],
        svec!["2025-03", "event2"],
        svec!["2025-04", ""],
        svec!["2025-05", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
#[should_panic(expected = "min and max have different units: Date(Month) vs Date(Day)")]
fn completed_dates_with_min_max_unit_mismach() {
    let wrk = Workdir::new("completed_dates_with_min_max_unit_mismatch");
    wrk.create("dates_sorted_uncomplete.csv", dates_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_uncomplete.csv")
        .arg("--dates")
        .arg("-m")
        .arg("2025-01")
        .arg("-M")
        .arg("2025-01-28");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_groupby() {
    let wrk = Workdir::new("complete_groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["group", "id", "value"],
            svec!["A", "0", "foo"],
            svec!["A", "2", "bar"],
            svec!["B", "1", "baz"],
            svec!["B", "3", "qux"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("data.csv").arg("--groupby").arg("group");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["group", "id", "value"],
        svec!["A", "0", "foo"],
        svec!["A", "1", ""],
        svec!["A", "2", "bar"],
        svec!["A", "3", ""],
        svec!["B", "0", ""],
        svec!["B", "1", "baz"],
        svec!["B", "2", ""],
        svec!["B", "3", "qux"],
    ];
    assert_eq!(got, expected);

    wrk.create(
        "data.csv",
        vec![
            svec!["group", "city", "id", "value"],
            svec!["A", "Paris", "0", "foo"],
            svec!["A", "Marseille", "2", "bar"],
            svec!["B", "Londres", "1", "baz"],
            svec!["B", "Paris", "3", "qux"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("data.csv")
        .arg("--reverse")
        .arg("--groupby")
        .arg("group,city");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = vec![
        svec!["group", "city", "id", "value"],
        svec!["A", "Paris", "3", ""],
        svec!["A", "Paris", "2", ""],
        svec!["A", "Paris", "1", ""],
        svec!["A", "Paris", "0", "foo"],
        svec!["A", "Marseille", "3", ""],
        svec!["A", "Marseille", "2", "bar"],
        svec!["A", "Marseille", "1", ""],
        svec!["A", "Marseille", "0", ""],
        svec!["B", "Londres", "3", ""],
        svec!["B", "Londres", "2", ""],
        svec!["B", "Londres", "1", "baz"],
        svec!["B", "Londres", "0", ""],
        svec!["B", "Paris", "3", "qux"],
        svec!["B", "Paris", "2", ""],
        svec!["B", "Paris", "1", ""],
        svec!["B", "Paris", "0", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
#[should_panic(
    expected = "Inconsistent value units: first seen was Date(Month) and then found Date(Year)"
)]
fn complete_groupby_unit_mismatch() {
    let wrk = Workdir::new("complete_groupby_unit_mismatch");
    wrk.create(
        "data.csv",
        vec![
            svec!["group", "date", "value"],
            svec!["A", "2025-01", "foo"],
            svec!["A", "2025-03", "bar"],
            svec!["B", "2025", "baz"],
            svec!["B", "2025-02", "baz"],
            svec!["B", "2025-04", "qux"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("data.csv")
        .arg("--dates")
        .arg("--groupby")
        .arg("group");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check() {
    let wrk = Workdir::new("complete_check");
    wrk.create("indexes_sorted_complete.csv", people_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_sorted_complete.csv")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);

    wrk.create("indexes_unsorted_complete.csv", people_unsorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_unsorted_complete.csv")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn complete_check_panic() {
    let wrk = Workdir::new("complete_check_panic");
    wrk.create("indexes.csv", people_unsorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id").arg("indexes.csv").arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic(
    expected = "Inconsistent value units: first seen was Date(Month) and then found Date(Day)"
)]
fn complete_check_unit_mismatch() {
    let wrk = Workdir::new("complete_check_unit_mismatch");
    wrk.create(
        "data.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-01", "event1"],
            svec!["2025-05", "event5"],
            svec!["2025-03", "event4"],
            svec!["2025-04-01", "event3"],
            svec!["2025-02", "event2"],
            svec!["2025-06", "event3"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("data.csv")
        .arg("--dates")
        .arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check_sorted() {
    let wrk = Workdir::new("complete_check_sorted");
    wrk.create("indexes_complete.csv", people_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("id")
        .arg("indexes_complete.csv")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);

    wrk.create(
        "indexes_complete_reverse.csv",
        people_sorted_complete_reverse(),
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("--reverse")
        .arg("id")
        .arg("indexes_complete_reverse.csv")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn complete_check_sorted_panic() {
    let wrk = Workdir::new("complete_check_sorted_panic");
    wrk.create("indexes.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("id")
        .arg("indexes.csv")
        .arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic(
    expected = "Inconsistent value units: first seen was Date(Month) and then found Date(Day)"
)]
fn complete_check_sorted_unit_mismatch() {
    let wrk = Workdir::new("complete_check_sorted_unit_mismatch");
    wrk.create(
        "data.csv",
        vec![
            svec!["date", "event"],
            svec!["2025-01", "event1"],
            svec!["2025-02", "event2"],
            svec!["2025-03", "event4"],
            svec!["2025-04", "event5"],
            svec!["2025-05-01", "event3"],
            svec!["2025-06", "event3"],
        ],
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("data.csv")
        .arg("--dates")
        .arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn complete_check_panic_min() {
    let wrk = Workdir::new("complete_check_panic_min");
    wrk.create("indexes.csv", people_unsorted_complete());
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
fn complete_check_panic_max() {
    let wrk = Workdir::new("complete_check_panic_max");
    wrk.create("indexes.csv", people_unsorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes.csv")
        .arg("--check")
        .arg("-M")
        .arg("5");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check_min_max() {
    let wrk = Workdir::new("complete_check_min_max");
    wrk.create("indexes_sorted_uncomplete.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_sorted_uncomplete.csv")
        .arg("--check")
        .arg("-m")
        .arg("2")
        .arg("-M")
        .arg("3");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);

    wrk.create(
        "indexes_unsorted_uncomplete.csv",
        people_unsorted_uncomplete(),
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("id")
        .arg("indexes_unsorted_uncomplete.csv")
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
fn complete_check_sorted_panic_min() {
    let wrk = Workdir::new("complete_check_sorted_panic_min");
    wrk.create("indexes.csv", people_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("id")
        .arg("indexes.csv")
        .arg("--check")
        .arg("-m")
        .arg("-1");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn complete_check_sorted_panic_max() {
    let wrk = Workdir::new("complete_check_sorted_panic_max");
    wrk.create("indexes.csv", people_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("id")
        .arg("indexes.csv")
        .arg("--check")
        .arg("-M")
        .arg("5");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check_sorted_min_max() {
    let wrk = Workdir::new("complete_check_sorted_min_max");
    wrk.create("indexes_uncomplete.csv", people_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("id")
        .arg("indexes_uncomplete.csv")
        .arg("--check")
        .arg("-m")
        .arg("2")
        .arg("-M")
        .arg("3");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);

    wrk.create(
        "indexes_uncomplete_reverse.csv",
        people_sorted_uncomplete_reverse(),
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("--reverse")
        .arg("id")
        .arg("indexes_uncomplete_reverse.csv")
        .arg("--check")
        .arg("-m")
        .arg("2")
        .arg("-M")
        .arg("3");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
fn complete_check_dates() {
    let wrk = Workdir::new("complete_check_dates");
    wrk.create("dates_sorted_complete.csv", dates_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_complete.csv")
        .arg("--dates")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);

    wrk.create("dates_unsorted_complete.csv", dates_unsorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_unsorted_complete.csv")
        .arg("--dates")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn complete_check_dates_panic() {
    let wrk = Workdir::new("complete_check_dates_panic");
    wrk.create("dates_uncomplete.csv", dates_unsorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check_sorted_dates() {
    let wrk = Workdir::new("complete_check_sorted_dates");
    wrk.create("dates_complete.csv", dates_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("dates_complete.csv")
        .arg("--dates")
        .arg("--check");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn complete_check_sorted_dates_panic() {
    let wrk = Workdir::new("complete_check_sorted_dates_panic");
    wrk.create("dates_uncomplete.csv", dates_sorted_uncomplete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("--check");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn complete_check_dates_panic_min() {
    let wrk = Workdir::new("complete_check_dates_panic_min");
    wrk.create("dates_uncomplete.csv", dates_unsorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-m")
        .arg("2025-01");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn complete_check_dates_panic_max() {
    let wrk = Workdir::new("complete_check_dates_panic_max");
    wrk.create("dates_uncomplete.csv", dates_unsorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-M")
        .arg("2025-06");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check_dates_min_max() {
    let wrk = Workdir::new("complete_check_dates_min_max");
    wrk.create("dates_sorted_complete.csv", dates_sorted_almost_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_sorted_complete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-m")
        .arg("2025-03")
        .arg("-M")
        .arg("2025-05");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["file is complete!"]];
    assert_eq!(got, expected);

    wrk.create(
        "dates_unsorted_complete.csv",
        dates_unsorted_almost_complete(),
    );
    let mut cmd = wrk.command("complete");
    cmd.arg("date")
        .arg("dates_unsorted_complete.csv")
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
#[should_panic]
fn complete_check_sorted_dates_panic_min() {
    let wrk = Workdir::new("complete_check_sorted_dates_panic_min");
    wrk.create("dates_uncomplete.csv", dates_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-m")
        .arg("2025-01");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
#[should_panic]
fn complete_check_sorted_dates_panic_max() {
    let wrk = Workdir::new("complete_check_sorted_dates_panic_max");
    wrk.create("dates_uncomplete.csv", dates_sorted_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
        .arg("dates_uncomplete.csv")
        .arg("--dates")
        .arg("--check")
        .arg("-M")
        .arg("2025-06");
    let _got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
}

#[test]
fn complete_check_sorted_dates_min_max() {
    let wrk = Workdir::new("complete_check_sorted_dates_min_max");
    wrk.create("dates_complete.csv", dates_sorted_almost_complete());
    let mut cmd = wrk.command("complete");
    cmd.arg("--sorted")
        .arg("date")
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
