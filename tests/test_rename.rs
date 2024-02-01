use workdir::Workdir;

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
