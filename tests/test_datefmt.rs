use workdir::Workdir;

#[test]
fn datefmt() {
    let wrk = Workdir::new("datefmt");
    wrk.create("data.csv", vec![
        svec!["datetime"],
        svec!["2021-04-30 21:14:10"],
        svec!["May 8, 2009 5:57:51 PM"],
        svec!["2012/03/19 10:11:59.3186369"],
        svec!["2021-05-01T01:17:02.604456Z"],
    ]);
    let mut cmd = wrk.command("datefmt");
    cmd.arg("datetime").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["datetime",
            "formatted_date"],
        svec!["2021-04-30 21:14:10",
            "2021-04-30 21:14:10 UTC"],
        svec!["May 8, 2009 5:57:51 PM",
            "2009-05-08 17:57:51 UTC"],
        svec!["2012/03/19 10:11:59.3186369",
            "2012-03-19 10:11:59.318636900 UTC"],
        svec!["2021-05-01T01:17:02.604456Z",
            "2021-05-01 01:17:02.604456 UTC"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn datefmt_no_headers() {
    let wrk = Workdir::new("datefmt");
    wrk.create("data.csv",  vec![
        svec!["2021-04-30 21:14:10"],
        svec!["May 8, 2009 5:57:51 PM"],
        svec!["2012/03/19 10:11:59.3186369"],
        svec!["2021-05-01T01:17:02.604456Z"],
    ]);
    let mut cmd = wrk.command("datefmt");
    cmd.arg("1").arg("--no-headers").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["2021-04-30 21:14:10",
            "2021-04-30 21:14:10 UTC"],
        svec!["May 8, 2009 5:57:51 PM",
            "2009-05-08 17:57:51 UTC"],
        svec!["2012/03/19 10:11:59.3186369",
            "2012-03-19 10:11:59.318636900 UTC"],
        svec!["2021-05-01T01:17:02.604456Z",
            "2021-05-01 01:17:02.604456 UTC"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn datefmt_column_name() {
    let wrk = Workdir::new("datefmt");
    wrk.create("data.csv", vec![
        svec!["datetime"],
        svec!["2021-04-30 21:14:10"],
        svec!["May 8, 2009 5:57:51 PM"],
        svec!["2012/03/19 10:11:59.3186369"],
        svec!["2021-05-01T01:17:02.604456Z"],
    ]);
    let mut cmd = wrk.command("datefmt");
    cmd.arg("datetime").arg("-c").arg("date").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["datetime",
            "date"],
        svec!["2021-04-30 21:14:10",
            "2021-04-30 21:14:10 UTC"],
        svec!["May 8, 2009 5:57:51 PM",
            "2009-05-08 17:57:51 UTC"],
        svec!["2012/03/19 10:11:59.3186369",
            "2012-03-19 10:11:59.318636900 UTC"],
        svec!["2021-05-01T01:17:02.604456Z",
            "2021-05-01 01:17:02.604456 UTC"],
    ];
    assert_eq!(got, expected);
}
