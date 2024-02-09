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

// TODO: test --no-headers
