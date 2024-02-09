use workdir::Workdir;

#[test]
fn count() {
    let wrk = Workdir::new("count");
    wrk.create("data.csv", vec![svec!["n"], svec!["1"], svec!["2"]]);

    let mut cmd = wrk.command("count");
    cmd.arg("data.csv");

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "2");
}

#[test]
fn count_no_headers() {
    let wrk = Workdir::new("count_no_headers");
    wrk.create("data.csv", vec![svec!["1"], svec!["2"]]);

    let mut cmd = wrk.command("count");
    cmd.arg("data.csv").arg("-n");

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "2");
}

#[test]
fn count_no_rows() {
    let wrk = Workdir::new("count_no_rows");
    wrk.create("data.csv", vec![svec!["n"]]);

    let mut cmd = wrk.command("count");
    cmd.arg("data.csv");

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "0");
}

#[test]
fn count_empty() {
    let wrk = Workdir::new("count_empty");
    wrk.create::<Vec<Vec<String>>>("data.csv", vec![]);

    let mut cmd = wrk.command("count");
    cmd.arg("data.csv");

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "0");
}

#[test]
fn count_empty_no_headers() {
    let wrk = Workdir::new("count_empty_no_headers");
    wrk.create::<Vec<Vec<String>>>("data.csv", vec![]);

    let mut cmd = wrk.command("count");
    cmd.arg("data.csv").arg("-n");

    let got: String = wrk.stdout(&mut cmd);

    assert_eq!(got.trim(), "0");
}
