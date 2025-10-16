use crate::workdir::Workdir;

#[test]
fn separate() {
    let wrk = Workdir::new("separate");
    wrk.create(
        "data.csv",
        vec![
            svec!["locution"],
            svec!["a priori"],
            svec!["de   facto"],
            svec![""],
            svec!["au cas   où"],
            svec![" "],
            svec!["ex-æquo"],
        ],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("locution").arg(" ").arg("data.csv");

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
    wrk.create(
        "data.csv",
        vec![
            svec!["locution"],
            svec!["a priori"],
            svec!["de   facto"],
            svec![""],
            svec!["au cas   où"],
            svec![" "],
            svec!["ex-æquo"],
        ],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("locution").arg(r"\s+").arg("data.csv").arg("-r");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["locution", "untitled1", "untitled2", "untitled3"],
        svec!["a priori", "a", "priori", ""],
        svec!["de   facto", "de", "facto", ""],
        svec!["", "", "", ""],
        svec!["au cas   où", "au", "cas", "où"],
        svec![" ", "", "", ""],
        svec!["ex-æquo", "ex-æquo", "", ""],
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
        svec!["path", "untitled1", "untitled2", "untitled3"],
        svec![
            "path/to/foo:54:Blue Harvest",
            "path/to/foo",
            "54",
            "Blue Harvest"
        ],
        svec![
            "path/to/bar:90:Something, Something, Something, Dark Side",
            "path/to/bar",
            "90",
            "Something, Something, Something, Dark Side"
        ],
        svec![
            "path/to/baz:3:It's a Trap!",
            "path/to/baz",
            "3",
            "It's a Trap!"
        ],
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
    cmd.arg("path")
        .arg(r"\d+")
        .arg("data.csv")
        .arg("-r")
        .arg("-m");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["locution", "untitled1", "untitled2"],
        svec!["abc123def456ghi", "123", "456"],
        svec!["test", "", ""],
        svec!["789xyz", "789", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn separate_regex_named_and_known_max_splits() {
    let wrk = Workdir::new("separate_regex_known_max_splits");
    wrk.create(
        "data.csv",
        vec![
            svec!["date"],
            svec!["2023-01-15"],
            svec!["1999-12-31"],
            svec!["2024-07-04"],
        ],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("date")
        .arg(r"(\d{4})-(\d{2})-(\d{2})$")
        .arg("data.csv")
        .arg("-r")
        .arg("-c")
        .arg("--max-splits")
        .arg("4")
        .arg("--into")
        .arg("year,month");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["date", "year", "month", "untitled1", "untitled2"],
        svec!["2023-01-15", "2023", "01", "15", ""],
        svec!["1999-12-31", "1999", "12", "31", ""],
        svec!["2024-07-04", "2024", "07", "04", ""],
    ];
    assert_eq!(got, expected);
}
