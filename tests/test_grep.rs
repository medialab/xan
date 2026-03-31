use crate::workdir::Workdir;

#[test]
fn grep() {
    let wrk = Workdir::new("grep");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );
    let mut cmd = wrk.command("grep");
    cmd.arg("evan").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name", "surname"], svec!["evan", "choucroute"]];
    assert_eq!(got, expected);
}

#[test]
fn grep_invert_match() {
    let wrk = Workdir::new("grep_invert_match");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );
    let mut cmd = wrk.command("grep");
    cmd.arg("evan").arg("-v").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "surname"],
        svec!["john", "landy"],
        svec!["béatrice", "babka"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn grep_count() {
    let wrk = Workdir::new("grep_count");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );
    let mut cmd = wrk.command("grep");
    cmd.arg("evan").arg("-c").arg("data.csv");

    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got.trim(), "1");
}

#[test]
fn grep_no_headers() {
    let wrk = Workdir::new("grep_no_headers");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );
    let mut cmd = wrk.command("grep");
    cmd.arg("evan").arg("-n").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["evan", "choucroute"]];
    assert_eq!(got, expected);
}

#[test]
fn grep_regex() {
    let wrk = Workdir::new("grep_regex");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );
    let mut cmd = wrk.command("grep");
    cmd.arg("evan|babka").arg("-r").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "surname"],
        svec!["evan", "choucroute"],
        svec!["béatrice", "babka"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn grep_case_insensitive() {
    let wrk = Workdir::new("grep_case_insensitive");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );

    let mut cmd = wrk.command("grep");
    cmd.arg("EVAN").arg("-i").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name", "surname"], svec!["evan", "choucroute"]];
    assert_eq!(got, expected);
}

#[test]
fn grep_regex_case_insensitive() {
    let wrk = Workdir::new("grep_regex_case_insensitive");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "surname"],
            svec!["john", "landy"],
            svec!["evan", "choucroute"],
            svec!["béatrice", "babka"],
        ],
    );
    let mut cmd = wrk.command("grep");
    cmd.arg("EVAN|BABKA").arg("-r").arg("-i").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "surname"],
        svec!["evan", "choucroute"],
        svec!["béatrice", "babka"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn grep_context() {
    let wrk = Workdir::new("grep_context");
    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["clarice"],
            svec!["john"],
            svec!["john"],
            svec!["john"],
            svec!["john"],
            svec!["lucy"],
            svec!["amy"],
        ],
    );

    let mut cmd = wrk.command("grep");
    cmd.arg("john").args(["-B", "3"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name"],
        ["clarice"],
        ["john"],
        ["john"],
        ["john"],
        ["john"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("grep");
    cmd.arg("lucy").args(["-B", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], ["lucy"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("grep");
    cmd.arg("lucy").args(["-A", "2"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["lucy"], ["amy"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("grep");
    cmd.arg("lucy")
        .args(["-A", "1"])
        .args(["-B", "1"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], ["lucy"], ["amy"]];
    assert_eq!(got, expected);
}
