use crate::workdir::Workdir;

#[test]
fn rename() {
    let wrk = Workdir::new("rename");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.arg("NAME,AGE").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["NAME", "AGE"], svec!["John", "24"]];
    assert_eq!(got, expected);
}

#[test]
fn rename_alignment_error() {
    let wrk = Workdir::new("rename_alignment_error");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.arg("NAME").arg("data.csv");

    wrk.assert_err(&mut cmd);

    let mut cmd = wrk.command("rename");
    cmd.arg("NAME,AGE").args(["-s", "name"]).arg("data.csv");

    wrk.assert_err(&mut cmd);
}

#[test]
fn rename_select() {
    let wrk = Workdir::new("rename_select");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.arg("A,B,C").args(["-s", "name,0,name"]).arg("data.csv");
    wrk.assert_err(&mut cmd);

    let mut cmd = wrk.command("rename");
    cmd.arg("NAME").args(["-s", "name"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["NAME", "age"], svec!["John", "24"]];
    assert_eq!(got, expected);
}

#[test]
fn rename_select_invert() {
    let wrk = Workdir::new("rename_select_invert");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.arg("NAME,AGE").args(["-s", "age,name"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["AGE", "NAME"], svec!["John", "24"]];
    assert_eq!(got, expected);
}

#[test]
fn rename_prefix() {
    let wrk = Workdir::new("rename_prefix");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.args(["--prefix", "test_"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["test_name", "test_age"], svec!["John", "24"]];
    assert_eq!(got, expected);
}

#[test]
fn rename_prefix_select() {
    let wrk = Workdir::new("rename_prefix_select");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.args(["--prefix", "test_"])
        .args(["-s", "age"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name", "test_age"], svec!["John", "24"]];
    assert_eq!(got, expected);
}

#[test]
fn rename_escapable_name() {
    let wrk = Workdir::new("rename_escapable_name");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.arg("NAME OF PERSON,\"AGE, \"\"OF\"\" PERSON\"")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["NAME OF PERSON", "AGE, \"OF\" PERSON"],
        svec!["John", "24"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn rename_no_headers() {
    let wrk = Workdir::new("rename_no_headers");
    wrk.create("data.csv", vec![svec!["John", "24"], svec!["Lisa", "28"]]);

    let mut cmd = wrk.command("rename");
    cmd.arg("-n").args(["--prefix", "test_"]).arg("data.csv");
    wrk.assert_err(&mut cmd);

    let mut cmd = wrk.command("rename");
    cmd.arg("-n").arg("NAME").arg("data.csv");
    wrk.assert_err(&mut cmd);

    let mut cmd = wrk.command("rename");
    cmd.arg("-n").arg("name,age").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "age"],
        svec!["John", "24"],
        svec!["Lisa", "28"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn rename_force() {
    let wrk = Workdir::new("rename_force");
    wrk.create("data.csv", vec![svec!["name", "age"], svec!["John", "24"]]);

    let mut cmd = wrk.command("rename");
    cmd.args(["-s", "surname,name,surname,age"])
        .arg("-f")
        .arg("SURNAME,NAME,SURNAME,AGE")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["NAME", "AGE"], svec!["John", "24"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("rename");
    cmd.args(["-s", "surname"])
        .arg("-f")
        .arg("SURNAME")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["name", "age"], svec!["John", "24"]];
    assert_eq!(got, expected);
}
