use crate::workdir::Workdir;

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
    cmd.arg("colors").arg("data.csv");

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
fn explode_drop_empty() {
    let wrk = Workdir::new("explode_drop_empty");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "colors"],
            svec!["Mary", ""],
            svec!["John", "blue|orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("colors").arg("--drop-empty").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "colors"],
        svec!["John", "blue"],
        svec!["John", "orange"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn explode_rename() {
    let wrk = Workdir::new("explode_rename");
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
fn explode_singularize() {
    let wrk = Workdir::new("explode_singularize");
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
    cmd.arg("colors").arg("-S").arg("data.csv");

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
    let wrk = Workdir::new("explode_no_headers");
    wrk.create(
        "data.csv",
        vec![
            svec!["Mary", "yellow"],
            svec!["John", "blue|orange"],
            svec!["Jack", ""],
        ],
    );
    let mut cmd = wrk.command("explode");
    cmd.arg("1").arg("--no-headers").arg("data.csv");

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
    cmd.arg("colors").args(["--sep", "[x]"]).arg("data.csv");

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
    cmd.arg("colors,letters").arg("data.csv");

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
