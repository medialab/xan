use workdir::Workdir;

#[test]
fn implode() {
    let wrk = Workdir::new("implode");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors"],
            svec!["Mary", "yellow"],
            svec!["John", "blue"],
            svec!["John", "orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("implode");
    cmd.arg("colors").arg("|").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors"],
        svec!["Mary", "yellow"],
        svec!["John", "blue|orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn implode_rename() {
    let wrk = Workdir::new("implode");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "color"],
            svec!["Mary", "yellow"],
            svec!["John", "blue"],
            svec!["John", "orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("implode");
    cmd.arg("color")
        .args(["--rename", "colors"])
        .arg("|")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors"],
        svec!["Mary", "yellow"],
        svec!["John", "blue|orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn implode_no_headers() {
    let wrk = Workdir::new("implode");
    wrk.create(
        "data.csv",
        vec![
            svec!["Mary", "yellow"],
            svec!["John", "blue"],
            svec!["John", "orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("implode");
    cmd.arg("1").arg("|").arg("--no-headers").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["Mary", "yellow"],
        svec!["John", "blue|orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn implode_multiple_columns() {
    let wrk = Workdir::new("implode_multiple_columns");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "color", "letter"],
            svec!["Mary", "yellow", "a"],
            svec!["John", "blue", "b"],
            svec!["John", "orange", "c"],
            svec!["Jack", "", "d"],
        ],
    );
    let mut cmd = wrk.command("implode");
    cmd.arg("color,letter").arg("|").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "color", "letter"],
        svec!["Mary", "yellow", "a"],
        svec!["John", "blue|orange", "b|c"],
        svec!["Jack", "", "d"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn implode_multiple_columns_rename() {
    let wrk = Workdir::new("implode_multiple_columns_rename");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "color", "letter"],
            svec!["Mary", "yellow", "a"],
            svec!["John", "blue", "b"],
            svec!["John", "orange", "c"],
            svec!["Jack", "", "d"],
        ],
    );
    let mut cmd = wrk.command("implode");
    cmd.arg("color,letter")
        .arg("|")
        .args(["-r", "colors,letters"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors", "letters"],
        svec!["Mary", "yellow", "a"],
        svec!["John", "blue|orange", "b|c"],
        svec!["Jack", "", "d"],
    ];
    assert_eq!(got, expected);
}
