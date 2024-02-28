use workdir::Workdir;

#[test]
fn explode() {
    let wrk = Workdir::new("explode");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors"],
            svec!["Mary", "yellow"],
            svec!["John", "blue|orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("colors").arg("|").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors"],
        svec!["Mary", "yellow"],
        svec!["John", "blue"],
        svec!["John", "orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn explode_rename() {
    let wrk = Workdir::new("explode");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors"],
            svec!["Mary", "yellow"],
            svec!["John", "blue|orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("colors")
        .args(["--rename", "color"])
        .arg("|")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "color"],
        svec!["Mary", "yellow"],
        svec!["John", "blue"],
        svec!["John", "orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn explode_no_headers() {
    let wrk = Workdir::new("explode");
    wrk.create(
        "data.csv",
        vec![
            svec!["Mary", "yellow"],
            svec!["John", "blue|orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("1").arg("|").arg("--no-headers").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["Mary", "yellow"],
        svec!["John", "blue"],
        svec!["John", "orange"],
        svec!["Jack", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn explode_multichar_sep() {
    let wrk = Workdir::new("explode_multichar_sep");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors"],
            svec!["John", "blue[x]orange[x]red"],
            svec!["Jack", "yellow[x]green"],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("colors").arg("[x]").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors"],
        svec!["John", "blue"],
        svec!["John", "orange"],
        svec!["John", "red"],
        svec!["Jack", "yellow"],
        svec!["Jack", "green"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn explode_multipe_columns() {
    let wrk = Workdir::new("explode_multipe_columns");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors", "letters"],
            svec!["John", "blue|red", "a|b"],
            svec!["Jack", "", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("colors,letters").arg("|").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors", "letters"],
        svec!["John", "blue", "a"],
        svec!["John", "red", "b"],
        svec!["Jack", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn explode_multipe_columns_rename() {
    let wrk = Workdir::new("explode_multipe_columns_rename");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors", "letters"],
            svec!["John", "blue|red", "a|b"],
            svec!["Jack", "", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("colors,letters")
        .arg("|")
        .args(["-r", "color,letter"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "color", "letter"],
        svec!["John", "blue", "a"],
        svec!["John", "red", "b"],
        svec!["Jack", "", ""],
    ];
    assert_eq!(got, expected);
}
