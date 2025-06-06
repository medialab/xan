use crate::workdir::Workdir;

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
    cmd.arg("colors").arg("data.csv");

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
fn implode_custom_sep() {
    let wrk = Workdir::new("implode_custom_sep");
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
    cmd.arg("colors").args(["--sep", "%"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors"],
        svec!["Mary", "yellow"],
        svec!["John", "blue%orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn implode_rename() {
    let wrk = Workdir::new("implode_rename");
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
fn implode_pluralize() {
    let wrk = Workdir::new("implode_pluralize");
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
    cmd.arg("color").arg("--pluralize").arg("data.csv");

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
    let wrk = Workdir::new("implode_no_headers");
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
    cmd.arg("1").arg("--no-headers").arg("data.csv");

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
    cmd.arg("color,letter").arg("data.csv");

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

#[test]
fn implode_cmp() {
    let wrk = Workdir::new("implode_cmp");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "n", "colors"],
            svec!["Mary", "0", "yellow"],
            svec!["John", "1", "blue"],
            svec!["John", "2", "orange"],
        ],
    );
    let mut cmd = wrk.command("implode");
    cmd.arg("colors").args(["--cmp", "name"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "n", "colors"],
        svec!["Mary", "0", "yellow"],
        svec!["John", "2", "blue|orange"],
    ];
    assert_eq!(got, expected);
}
