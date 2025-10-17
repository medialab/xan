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
    let wrk1 = Workdir::new("separate_with_one_column");
    wrk1.create("data.csv", data());
    let mut cmd1 = wrk1.command("separate");
    cmd1.arg("locution").arg(" ").arg("data.csv");

    let got1: Vec<Vec<String>> = wrk1.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec![
            "untitled1",
            "untitled2",
            "untitled3",
            "untitled4",
            "untitled5"
        ],
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
        svec![
            "untitled1",
            "untitled2",
            "untitled3",
            "untitled4",
            "untitled5"
        ],
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
        svec!["birthdate", "untitled1", "untitled2"],
        svec!["1990 05 15", "John", "Doe"],
        svec!["1985 10 30", "Jane", "Smith"],
        svec!["2000 01 01", "Alice", "Johnson"],
    ];
    assert_eq!(got3, expected3);
}

#[test]
fn separate_keep_column() {
    let wrk = Workdir::new("separate");
    wrk.create("data.csv", data());
    let mut cmd = wrk.command("separate");
    cmd.arg("locution")
        .arg(" ")
        .arg("data.csv")
        .arg("--keep-column");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec![
            "locution",
            "untitled1",
            "untitled2",
            "untitled3",
            "untitled4",
            "untitled5"
        ],
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
        svec!["untitled1", "untitled2", "untitled3"],
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
        svec!["untitled1", "untitled2", "untitled3"],
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
        svec!["untitled1", "untitled2"],
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
        svec!["year", "month", "day", "untitled1"],
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
