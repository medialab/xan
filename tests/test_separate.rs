use crate::workdir::Workdir;

fn data() -> Vec<Vec<String>> {
    vec![
        svec!["id", "locution"],
        svec!["0", "a priori"],
        svec!["1", "de   facto"],
        svec!["2", ""],
        svec!["3", "au cas   où"],
        svec!["4", " "],
        svec!["5", "ex-æquo"],
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
    let wrk = Workdir::new("separate");
    wrk.create("data.csv", data());
    let mut cmd1 = wrk.command("separate");
    cmd1.arg("locution").arg(" ").arg("data.csv");

    let got1: Vec<Vec<String>> = wrk.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["id", "split1", "split2", "split3", "split4", "split5"],
        svec!["0", "a", "priori", "", "", ""],
        svec!["1", "de", "", "", "facto", ""],
        svec!["2", "", "", "", "", ""],
        svec!["3", "au", "cas", "", "", "où"],
        svec!["4", "", "", "", "", ""],
        svec!["5", "ex-æquo", "", "", "", ""],
    ];
    assert_eq!(got1, expected1);

    wrk.create("data.csv", people());
    let mut cmd2 = wrk.command("separate");
    cmd2.arg("fullname,birthdate").arg(" ").arg("data.csv");
    let got2: Vec<Vec<String>> = wrk.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["split1", "split2", "split3", "split4", "split5"],
        svec!["John", "Doe", "1990", "05", "15"],
        svec!["Jane", "Smith", "1985", "10", "30"],
        svec!["Alice", "Johnson", "2000", "01", "01"],
    ];
    assert_eq!(got2, expected2);

    let mut cmd3 = wrk.command("separate");
    cmd3.arg("fullname").arg(" ").arg("data.csv");
    let got3: Vec<Vec<String>> = wrk.read_stdout(&mut cmd3);
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
    let wrk = Workdir::new("separate_extra");
    wrk.create("data.csv", people());
    let mut cmd1 = wrk.command("separate");
    cmd1.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--max-splitted-cells")
        .arg("3");
    let got1: Vec<Vec<String>> = wrk.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["split1", "split2", "split3"],
        svec!["John", "Doe", "1990"],
        svec!["Jane", "Smith", "1985"],
        svec!["Alice", "Johnson", "2000"],
    ];
    assert_eq!(got1, expected1);

    let mut cmd2 = wrk.command("separate");
    cmd2.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--into")
        .arg("firstname,lastname,birthyear");
    let got2: Vec<Vec<String>> = wrk.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["firstname", "lastname", "birthyear"],
        svec!["John", "Doe", "1990"],
        svec!["Jane", "Smith", "1985"],
        svec!["Alice", "Johnson", "2000"],
    ];
    assert_eq!(got2, expected2);

    let mut cmd3 = wrk.command("separate");
    cmd3.arg("fullname")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--max-splitted-cells")
        .arg("5");
    let got3: Vec<Vec<String>> = wrk.read_stdout(&mut cmd3);
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

    let mut cmd4 = wrk.command("separate");
    cmd4.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("merge")
        .arg("--into")
        .arg("firstname,lastname,birthdate");
    let got4: Vec<Vec<String>> = wrk.read_stdout(&mut cmd4);
    let expected4 = vec![
        svec!["firstname", "lastname", "birthdate"],
        svec!["John", "Doe", "1990 05 15"],
        svec!["Jane", "Smith", "1985 10 30"],
        svec!["Alice", "Johnson", "2000 01 01"],
    ];
    assert_eq!(got4, expected4);

    let mut cmd5 = wrk.command("separate");
    cmd5.arg("fullname,birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("merge")
        .arg("--max-splitted-cells")
        .arg("3");
    let got5: Vec<Vec<String>> = wrk.read_stdout(&mut cmd5);
    let expected5 = vec![
        svec!["split1", "split2", "split3"],
        svec!["John", "Doe", "1990 05 15"],
        svec!["Jane", "Smith", "1985 10 30"],
        svec!["Alice", "Johnson", "2000 01 01"],
    ];
    assert_eq!(got5, expected5);

    let mut cmd6 = wrk.command("separate");
    cmd6.arg("fullname")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--max-splitted-cells")
        .arg("5");
    let got6: Vec<Vec<String>> = wrk.read_stdout(&mut cmd6);
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
        svec!["id", "locution", "split1", "split2", "split3", "split4", "split5"],
        svec!["0", "a priori", "a", "priori", "", "", ""],
        svec!["1", "de   facto", "de", "", "", "facto", ""],
        svec!["2", "", "", "", "", "", ""],
        svec!["3", "au cas   où", "au", "cas", "", "", "où"],
        svec!["4", " ", "", "", "", "", ""],
        svec!["5", "ex-æquo", "ex-æquo", "", "", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_regex_sep() {
    let wrk = Workdir::new("separate_regex_sep");
    wrk.create("data.csv", data());
    let mut cmd = wrk.command("separate");
    cmd.arg("locution").arg(r"\s+").arg("data.csv").arg("-r");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "split1", "split2", "split3"],
        svec!["0", "a", "priori", ""],
        svec!["1", "de", "facto", ""],
        svec!["2", "", "", ""],
        svec!["3", "au", "cas", "où"],
        svec!["4", "", "", ""],
        svec!["5", "ex-æquo", "", ""],
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
    let wrk = Workdir::new("separate_named_and_known_max_splits");
    wrk.create("data.csv", dates());
    let mut cmd1 = wrk.command("separate");
    cmd1.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max-splitted-cells")
        .arg("4")
        .arg("--into")
        .arg("year,month,day");
    let got1: Vec<Vec<String>> = wrk.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["year", "month", "day", "split1"],
        svec!["2023", "01", "15", ""],
        svec!["1999", "12", "31", ""],
        svec!["2024", "07", "04", ""],
    ];
    assert_eq!(got1, expected1);

    let mut cmd2 = wrk.command("separate");
    cmd2.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max-splitted-cells")
        .arg("3")
        .arg("--into")
        .arg("year,month,day");
    let got2: Vec<Vec<String>> = wrk.read_stdout(&mut cmd2);
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
        .arg("--max-splitted-cells")
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
        .arg("--max-splitted-cells")
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

#[test]
fn separate_fixed_width() {
    let wrk = Workdir::new("separate_fixed_width");
    wrk.create("dates.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--fixed-width")
        .arg("3")
        .arg("dates.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2", "split3", "split4"],
        svec!["202", "3-0", "1-1", "5"],
        svec!["199", "9-1", "2-3", "1"],
        svec!["202", "4-0", "7-0", "4"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--fixed-width")
        .arg("5")
        .arg("dates.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2"],
        svec!["2023-", "01-15"],
        svec!["1999-", "12-31"],
        svec!["2024-", "07-04"],
    ];
    assert_eq!(got, expected);

    wrk.create("data.csv", people());
    let mut cmd = wrk.command("separate");
    cmd.arg("fullname,birthdate")
        .arg("--fixed-width")
        .arg("5")
        .arg("data.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2", "split3", "split4", "split5"],
        svec!["John", "Doe", "1990", "05 15", ""],
        svec!["Jane", "Smith", "1985", "10 30", ""],
        svec!["Alice", "John", "son", "2000", "01 01"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_widths() {
    let wrk = Workdir::new("separate_widths");
    wrk.create("dates.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--widths")
        .arg("4,3,3")
        .arg("dates.csv")
        .arg("--into")
        .arg("year,month,day");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month", "day"],
        svec!["2023", "-01", "-15"],
        svec!["1999", "-12", "-31"],
        svec!["2024", "-07", "-04"],
    ];
    assert_eq!(got, expected);

    wrk.create("data.csv", people());
    let mut cmd = wrk.command("separate");
    cmd.arg("fullname,birthdate")
        .arg("--widths")
        .arg("4,3,3")
        .arg("data.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2", "split3", "split4", "split5", "split6"],
        svec!["John", "Do", "e", "1990", "05", "15"],
        svec!["Jane", "Sm", "ith", "1985", "10", "30"],
        svec!["Alic", "e J", "ohn", "2000", "01", "01"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_offsets() {
    let wrk = Workdir::new("separate_offsets");
    wrk.create("dates.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--offsets")
        .arg("4,7,10")
        .arg("dates.csv")
        .arg("--into")
        .arg("year,month,day");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month", "day"],
        svec!["2023", "-01", "-15"],
        svec!["1999", "-12", "-31"],
        svec!["2024", "-07", "-04"],
    ];
    assert_eq!(got, expected);

    wrk.create("data.csv", people());
    let mut cmd = wrk.command("separate");
    cmd.arg("fullname,birthdate")
        .arg("--offsets")
        .arg("4,7,10")
        .arg("data.csv");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["split1", "split2", "split3", "split4", "split5", "split6"],
        svec!["John", "Do", "e", "1990", "05", "15"],
        svec!["Jane", "Sm", "ith", "1985", "10", "30"],
        svec!["Alic", "e J", "ohn", "2000", "01", "01"],
    ];
    assert_eq!(got, expected);
}
