use crate::workdir::Workdir;

fn data() -> Vec<Vec<String>> {
    vec![
        svec!["locution"],
        svec!["a priori"],
        svec!["de   facto"],
        svec![""],
        svec!["au cas   où"],
        svec![" "],
        svec!["ex-æquo"],
    ]
}

fn dates() -> Vec<Vec<String>> {
    vec![
        svec!["date"],
        svec!["2023-01-15"],
        svec!["1999-12-31"],
        svec!["2024-07-04"],
    ]
}

fn people() -> Vec<Vec<String>> {
    vec![
        svec!["fullname", "birthdate"],
        svec!["John Doe", "1990 05 15"],
        svec!["Jane Smith", "1985 10 30"],
        svec!["Alice Johnson", "2000 01 01"],
    ]
}

#[test]
fn separate() {
    let wrk1 = Workdir::new("separate");
    wrk1.create("data.csv", data());
    let mut cmd1 = wrk1.command("separate");
    cmd1.arg("locution").arg(" ").arg("data.csv");

    let got1: Vec<Vec<String>> = wrk1.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["split1", "split2", "split3", "split4", "split5"],
        svec!["a", "priori", "", "", ""],
        svec!["de", "", "", "facto", ""],
        svec!["", "", "", "", ""],
        svec!["au", "cas", "", "", "où"],
        svec!["", "", "", "", ""],
        svec!["ex-æquo", "", "", "", ""],
    ];
    assert_eq!(got1, expected1);

    let wrk2 = Workdir::new("separate_all_columns");
    wrk2.create("data.csv", people());
    let mut cmd2 = wrk2.command("separate");
    cmd2.arg("fullname,birthdate").arg(" ").arg("data.csv");
    let got2: Vec<Vec<String>> = wrk2.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["split1", "split2", "split3", "split4", "split5"],
        svec!["John", "Doe", "1990", "05", "15"],
        svec!["Jane", "Smith", "1985", "10", "30"],
        svec!["Alice", "Johnson", "2000", "01", "01"],
    ];
    assert_eq!(got2, expected2);

    let wrk3 = Workdir::new("separate_one_column_with_two_columns");
    wrk3.create("data.csv", people());
    let mut cmd3 = wrk3.command("separate");
    cmd3.arg("fullname").arg(" ").arg("data.csv");
    let got3: Vec<Vec<String>> = wrk3.read_stdout(&mut cmd3);
    let expected3 = vec![
        svec!["birthdate", "split1", "split2"],
        svec!["1990 05 15", "John", "Doe"],
        svec!["1985 10 30", "Jane", "Smith"],
        svec!["2000 01 01", "Alice", "Johnson"],
    ];
    assert_eq!(got3, expected3);
}

#[test]
fn separate_extra() {
    let wrk1 = Workdir::new("separate_extra_drop_max_splits");
    wrk1.create("data.csv", people());
    let mut cmd1 = wrk1.command("separate");
    cmd1.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--extra")
        .arg("drop")
        .arg("--max-splits")
        .arg("3");
    let got1: Vec<Vec<String>> = wrk1.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["split1", "split2", "split3"],
        svec!["John", "Doe", "1990"],
        svec!["Jane", "Smith", "1985"],
        svec!["Alice", "Johnson", "2000"],
    ];
    assert_eq!(got1, expected1);

    let wrk2 = Workdir::new("separate_extra_drop_named");
    wrk2.create("data.csv", people());
    let mut cmd2 = wrk2.command("separate");
    cmd2.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--extra")
        .arg("drop")
        .arg("--into")
        .arg("firstname,lastname,birthyear");
    let got2: Vec<Vec<String>> = wrk2.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["firstname", "lastname", "birthyear"],
        svec!["John", "Doe", "1990"],
        svec!["Jane", "Smith", "1985"],
        svec!["Alice", "Johnson", "2000"],
    ];
    assert_eq!(got2, expected2);

    let wrk3 = Workdir::new("separate_extra_drop_no_effect");
    wrk3.create("data.csv", people());
    let mut cmd3 = wrk3.command("separate");
    cmd3.arg("fullname")
        .arg(" ")
        .arg("data.csv")
        .arg("--extra")
        .arg("drop")
        .arg("--max-splits")
        .arg("5");
    let got3: Vec<Vec<String>> = wrk3.read_stdout(&mut cmd3);
    let expected3 = vec![
        svec![
            "birthdate",
            "split1",
            "split2",
            "split3",
            "split4",
            "split5"
        ],
        svec!["1990 05 15", "John", "Doe", "", "", ""],
        svec!["1985 10 30", "Jane", "Smith", "", "", ""],
        svec!["2000 01 01", "Alice", "Johnson", "", "", ""],
    ];
    assert_eq!(got3, expected3);

    let wrk4 = Workdir::new("separate_extra_merge_named");
    wrk4.create("data.csv", people());
    let mut cmd4 = wrk4.command("separate");
    cmd4.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--extra")
        .arg("merge")
        .arg("--into")
        .arg("firstname,lastname,birthdate");
    let got4: Vec<Vec<String>> = wrk4.read_stdout(&mut cmd4);
    let expected4 = vec![
        svec!["firstname", "lastname", "birthdate"],
        svec!["John", "Doe", "1990|05|15"],
        svec!["Jane", "Smith", "1985|10|30"],
        svec!["Alice", "Johnson", "2000|01|01"],
    ];
    assert_eq!(got4, expected4);

    let wrk5 = Workdir::new("separate_extra_merge_max_splits");
    wrk5.create("data.csv", people());
    let mut cmd5 = wrk5.command("separate");
    cmd5.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--extra")
        .arg("merge")
        .arg("--max-splits")
        .arg("3");
    let got5: Vec<Vec<String>> = wrk5.read_stdout(&mut cmd5);
    let expected5 = vec![
        svec!["split1", "split2", "split3"],
        svec!["John", "Doe", "1990|05|15"],
        svec!["Jane", "Smith", "1985|10|30"],
        svec!["Alice", "Johnson", "2000|01|01"],
    ];
    assert_eq!(got5, expected5);

    let wrk6 = Workdir::new("separate_extra_no_effect");
    wrk6.create("data.csv", people());
    let mut cmd6 = wrk6.command("separate");
    cmd6.arg("fullname")
        .arg(" ")
        .arg("data.csv")
        .arg("--extra")
        .arg("no_effect")
        .arg("--max-splits")
        .arg("5");
    let got6: Vec<Vec<String>> = wrk6.read_stdout(&mut cmd6);
    let expected6 = vec![
        svec![
            "birthdate",
            "split1",
            "split2",
            "split3",
            "split4",
            "split5"
        ],
        svec!["1990 05 15", "John", "Doe", "", "", ""],
        svec!["1985 10 30", "Jane", "Smith", "", "", ""],
        svec!["2000 01 01", "Alice", "Johnson", "", "", ""],
    ];
    assert_eq!(got6, expected6);
}

#[test]
fn separate_keep_column() {
    let wrk = Workdir::new("separate_keep_column");
    wrk.create("data.csv", data());
    let mut cmd = wrk.command("separate");
    cmd.arg("locution").arg(" ").arg("data.csv").arg("--keep");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["locution", "split1", "split2", "split3", "split4", "split5"],
        svec!["a priori", "a", "priori", "", "", ""],
        svec!["de   facto", "de", "", "", "facto", ""],
        svec!["", "", "", "", "", ""],
        svec!["au cas   où", "au", "cas", "", "", "où"],
        svec![" ", "", "", "", "", ""],
        svec!["ex-æquo", "ex-æquo", "", "", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_regex_sep() {
    let wrk = Workdir::new("separate_regex");
    wrk.create("data.csv", data());
    let mut cmd = wrk.command("separate");
    cmd.arg("locution").arg(r"\s+").arg("data.csv").arg("-r");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2", "split3"],
        svec!["a", "priori", ""],
        svec!["de", "facto", ""],
        svec!["", "", ""],
        svec!["au", "cas", "où"],
        svec!["", "", ""],
        svec!["ex-æquo", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_regex_capture_groups() {
    let wrk = Workdir::new("separate_regex_capture_groups");
    wrk.create(
        "data.csv",
        vec![
            svec!["path"],
            svec!["path/to/foo:54:Blue Harvest"],
            svec!["path/to/bar:90:Something, Something, Something, Dark Side"],
            svec!["path/to/baz:3:It's a Trap!"],
        ],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("path")
        .arg(r"(?m)^([^:]+):([0-9]+):(.+)$")
        .arg("data.csv")
        .arg("-r")
        .arg("-c");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2", "split3"],
        svec!["path/to/foo", "54", "Blue Harvest"],
        svec![
            "path/to/bar",
            "90",
            "Something, Something, Something, Dark Side"
        ],
        svec!["path/to/baz", "3", "It's a Trap!"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_regex_match() {
    let wrk = Workdir::new("separate_regex_match");
    wrk.create(
        "data.csv",
        vec![
            svec!["locution"],
            svec!["abc123def456ghi"],
            svec!["test"],
            svec!["789xyz"],
        ],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("locution")
        .arg(r"\d+")
        .arg("data.csv")
        .arg("-r")
        .arg("-m");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2"],
        svec!["123", "456"],
        svec!["", ""],
        svec!["789", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_regex_named() {
    let wrk = Workdir::new("separate_regex_named");
    wrk.create("data.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--into")
        .arg("year,month,day");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month", "day"],
        svec!["2023", "01", "15"],
        svec!["1999", "12", "31"],
        svec!["2024", "07", "04"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_named_and_known_max_splits() {
    let wrk1 = Workdir::new("separate_more_named_than_known_max_splits");
    wrk1.create("data.csv", dates());
    let mut cmd1 = wrk1.command("separate");
    cmd1.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max-splits")
        .arg("4")
        .arg("--into")
        .arg("year,month,day");
    let got1: Vec<Vec<String>> = wrk1.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["year", "month", "day", "split1"],
        svec!["2023", "01", "15", ""],
        svec!["1999", "12", "31", ""],
        svec!["2024", "07", "04", ""],
    ];
    assert_eq!(got1, expected1);

    let wrk2 = Workdir::new("separate_named_and_known_max_splits");
    wrk2.create("data.csv", dates());
    let mut cmd2 = wrk2.command("separate");
    cmd2.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max-splits")
        .arg("3")
        .arg("--into")
        .arg("year,month,day");
    let got2: Vec<Vec<String>> = wrk2.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["year", "month", "day"],
        svec!["2023", "01", "15"],
        svec!["1999", "12", "31"],
        svec!["2024", "07", "04"],
    ];
    assert_eq!(got2, expected2);
}

#[test]
#[should_panic]
fn separate_less_named_than_known_max_splits() {
    let wrk = Workdir::new("separate_less_named_than_known_max_splits");
    wrk.create("data.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max-splits")
        .arg("2")
        .arg("--into")
        .arg("year,month,day");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month", "day"],
        svec!["2023", "01", "15"],
        svec!["1999", "12", "31"],
        svec!["2024", "07", "04"],
    ];
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn separate_named_too_much_splits() {
    let wrk = Workdir::new("separate_named_too_much_splits");
    wrk.create("data.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--into")
        .arg("year,month");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month"],
        svec!["2023", "01"],
        svec!["1999", "12"],
        svec!["2024", "07"],
    ];
    assert_eq!(got, expected);
}

#[test]
#[should_panic]
fn separate_known_max_splits_too_much_splits() {
    let wrk = Workdir::new("separate_known_max_splits_too_much_splits");
    wrk.create("data.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max-splits")
        .arg("2");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month"],
        svec!["2023", "01"],
        svec!["1999", "12"],
        svec!["2024", "07"],
    ];
    assert_eq!(got, expected);
}
