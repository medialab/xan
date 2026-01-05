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
        svec![
            "id",
            "locution1",
            "locution2",
            "locution3",
            "locution4",
            "locution5"
        ],
        svec!["0", "a", "priori", "", "", ""],
        svec!["1", "de", "", "", "facto", ""],
        svec!["2", "", "", "", "", ""],
        svec!["3", "au", "cas", "", "", "où"],
        svec!["4", "", "", "", "", ""],
        svec!["5", "ex-æquo", "", "", "", ""],
    ];
    assert_eq!(got1, expected1);

    wrk.create("data.csv", people());
    let mut cmd3 = wrk.command("separate");
    cmd3.arg("fullname").arg(" ").arg("data.csv");
    let got3: Vec<Vec<String>> = wrk.read_stdout(&mut cmd3);
    let expected3 = vec![
        svec!["fullname1", "fullname2", "birthdate"],
        svec!["John", "Doe", "1990 05 15"],
        svec!["Jane", "Smith", "1985 10 30"],
        svec!["Alice", "Johnson", "2000 01 01"],
    ];
    assert_eq!(got3, expected3);
}

#[test]
fn separate_too_many() {
    let wrk = Workdir::new("separate_too_many");
    wrk.create("data.csv", people());
    let mut cmd1 = wrk.command("separate");
    cmd1.arg("birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .args(["--max", "2"]);
    let got1: Vec<Vec<String>> = wrk.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["fullname", "birthdate1", "birthdate2"],
        svec!["John Doe", "1990", "05"],
        svec!["Jane Smith", "1985", "10"],
        svec!["Alice Johnson", "2000", "01"],
    ];
    assert_eq!(got1, expected1);

    let mut cmd2 = wrk.command("separate");
    cmd2.arg("birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--into")
        .arg("birthyear,birthmonth");
    let got2: Vec<Vec<String>> = wrk.read_stdout(&mut cmd2);
    let expected2 = vec![
        svec!["fullname", "birthyear", "birthmonth"],
        svec!["John Doe", "1990", "05"],
        svec!["Jane Smith", "1985", "10"],
        svec!["Alice Johnson", "2000", "01"],
    ];
    assert_eq!(got2, expected2);

    let mut cmd3 = wrk.command("separate");
    cmd3.arg("fullname")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--max")
        .arg("5");
    let got3: Vec<Vec<String>> = wrk.read_stdout(&mut cmd3);
    let expected3 = vec![
        svec![
            "fullname1",
            "fullname2",
            "fullname3",
            "fullname4",
            "fullname5",
            "birthdate"
        ],
        svec!["John", "Doe", "", "", "", "1990 05 15"],
        svec!["Jane", "Smith", "", "", "", "1985 10 30"],
        svec!["Alice", "Johnson", "", "", "", "2000 01 01"],
    ];
    assert_eq!(got3, expected3);

    let mut cmd4 = wrk.command("separate");
    cmd4.arg("birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("merge")
        .arg("--into")
        .arg("birthyear,birthmonth_day");
    let got4: Vec<Vec<String>> = wrk.read_stdout(&mut cmd4);
    let expected4 = vec![
        svec!["fullname", "birthyear", "birthmonth_day"],
        svec!["John Doe", "1990", "05 15"],
        svec!["Jane Smith", "1985", "10 30"],
        svec!["Alice Johnson", "2000", "01 01"],
    ];
    assert_eq!(got4, expected4);

    let mut cmd5 = wrk.command("separate");
    cmd5.arg("birthdate")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("merge")
        .arg("--max")
        .arg("2");
    let got5: Vec<Vec<String>> = wrk.read_stdout(&mut cmd5);
    let expected5 = vec![
        svec!["fullname", "birthdate1", "birthdate2"],
        svec!["John Doe", "1990", "05 15"],
        svec!["Jane Smith", "1985", "10 30"],
        svec!["Alice Johnson", "2000", "01 01"],
    ];
    assert_eq!(got5, expected5);

    let mut cmd6 = wrk.command("separate");
    cmd6.arg("fullname")
        .arg(" ")
        .arg("data.csv")
        .arg("--too-many")
        .arg("drop")
        .arg("--max")
        .arg("5");
    let got6: Vec<Vec<String>> = wrk.read_stdout(&mut cmd6);
    let expected6 = vec![
        svec![
            "fullname1",
            "fullname2",
            "fullname3",
            "fullname4",
            "fullname5",
            "birthdate"
        ],
        svec!["John", "Doe", "", "", "", "1990 05 15"],
        svec!["Jane", "Smith", "", "", "", "1985 10 30"],
        svec!["Alice", "Johnson", "", "", "", "2000 01 01"],
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
        svec![
            "id",
            "locution",
            "locution1",
            "locution2",
            "locution3",
            "locution4",
            "locution5"
        ],
        svec!["0", "a priori", "a", "priori", "", "", ""],
        svec!["1", "de   facto", "de", "", "", "facto", ""],
        svec!["2", "", "", "", "", "", ""],
        svec!["3", "au cas   où", "au", "cas", "", "", "où"],
        svec!["4", " ", "", "", "", "", ""],
        svec!["5", "ex-æquo", "ex-æquo", "", "", "", ""],
    ];
    assert_eq!(got, expected);

    wrk.create("data.csv", people());
    let mut cmd = wrk.command("separate");
    cmd.arg("fullname").arg(" ").arg("data.csv").arg("--keep");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["fullname", "fullname1", "fullname2", "birthdate"],
        svec!["John Doe", "John", "Doe", "1990 05 15"],
        svec!["Jane Smith", "Jane", "Smith", "1985 10 30"],
        svec!["Alice Johnson", "Alice", "Johnson", "2000 01 01"],
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
        svec!["id", "locution1", "locution2", "locution3"],
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
        svec!["path1", "path2", "path3"],
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
        svec!["locution1", "locution2"],
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
        .arg("--max")
        .arg("4")
        .arg("--into")
        .arg("year,month,day");
    let got1: Vec<Vec<String>> = wrk.read_stdout(&mut cmd1);
    let expected1 = vec![
        svec!["year", "month", "day", "date1"],
        svec!["2023", "01", "15", ""],
        svec!["1999", "12", "31", ""],
        svec!["2024", "07", "04", ""],
    ];
    assert_eq!(got1, expected1);

    let mut cmd2 = wrk.command("separate");
    cmd2.arg("date")
        .arg("-")
        .arg("data.csv")
        .arg("--max")
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
        .arg("--max")
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
        .arg("--max")
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
        svec!["date1", "date2", "date3", "date4"],
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
        svec!["date1", "date2"],
        svec!["2023-", "01-15"],
        svec!["1999-", "12-31"],
        svec!["2024-", "07-04"],
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
}

#[test]
fn separate_split_on_bytes() {
    let wrk = Workdir::new("separate_split_on_bytes");
    wrk.create("dates.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--cuts")
        .arg("4,7")
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
}

#[test]
fn separate_segment_bytes() {
    let wrk = Workdir::new("separate_segment_bytes");
    wrk.create("dates.csv", dates());
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--offsets")
        .arg("0,4,7,10")
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

    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg("--offsets")
        .arg("0,4,7")
        .arg("dates.csv")
        .arg("--into")
        .arg("year,month");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["year", "month"],
        svec!["2023", "-01"],
        svec!["1999", "-12"],
        svec!["2024", "-07"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_no_headers() {
    let wrk = Workdir::new("map");
    wrk.create(
        "data.csv",
        vec![svec!["john landis", "1"], svec!["evan babka", "2"]],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("-n").arg("0").arg(" ").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["john", "landis", "1"], svec!["evan", "babka", "2"]];
    assert_eq!(got, expected);
}
