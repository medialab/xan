use crate::workdir::Workdir;

#[test]
fn window_row_number() {
    let wrk = Workdir::new("window_row_number");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("row_number() as row_number").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "row_number"],
        svec!["1", "1"],
        svec!["2", "2"],
        svec!["3", "3"],
        svec!["4", "4"],
        svec!["5", "5"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_lag() {
    let wrk = Workdir::new("window_lag");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );

    // n-1
    let mut cmd = wrk.command("window");
    cmd.arg("lag(n) as 'n-1'").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n-1"],
        svec!["1", ""],
        svec!["2", "1"],
        svec!["3", "2"],
        svec!["4", "3"],
        svec!["5", "4"],
    ];

    assert_eq!(got, expected);

    // n-3
    let mut cmd = wrk.command("window");
    cmd.arg("lag(n, 3) as 'n-3'").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n-3"],
        svec!["1", ""],
        svec!["2", ""],
        svec!["3", ""],
        svec!["4", "1"],
        svec!["5", "2"],
    ];

    assert_eq!(got, expected);

    // n-1 & n-3
    let mut cmd = wrk.command("window");
    cmd.arg("lag(n) as 'n-1', lag(n, 3) as 'n-3'")
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n-1", "n-3"],
        svec!["1", "", ""],
        svec!["2", "1", ""],
        svec!["3", "2", ""],
        svec!["4", "3", "1"],
        svec!["5", "4", "2"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_lead() {
    let wrk = Workdir::new("window_lead");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );

    // n+1
    let mut cmd = wrk.command("window");
    cmd.arg("lead(n) as 'n+1'").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n+1"],
        svec!["1", "2"],
        svec!["2", "3"],
        svec!["3", "4"],
        svec!["4", "5"],
        svec!["5", ""],
    ];

    assert_eq!(got, expected);

    // n+3
    let mut cmd = wrk.command("window");
    cmd.arg("lead(n, 3) as 'n+3'").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n+3"],
        svec!["1", "4"],
        svec!["2", "5"],
        svec!["3", ""],
        svec!["4", ""],
        svec!["5", ""],
    ];

    assert_eq!(got, expected);

    // n+1 & n+3
    let mut cmd = wrk.command("window");
    cmd.arg("lead(n) as 'n+1', lead(n, 3) as 'n+3'")
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n+1", "n+3"],
        svec!["1", "2", "4"],
        svec!["2", "3", "5"],
        svec!["3", "4", ""],
        svec!["4", "5", ""],
        svec!["5", "", ""],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_lag_and_lead() {
    let wrk = Workdir::new("window_lag_and_lead");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );

    // n-1 & n-3 & n+1 & n+3
    let mut cmd = wrk.command("window");
    cmd.arg("lag(n) as 'n-1', lag(n, 3, -1) as 'n-3', lead(n) as 'n+1', lead(n, 3, -2) as 'n+3'")
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "n-1", "n-3", "n+1", "n+3"],
        svec!["1", "", "-1", "2", "4"],
        svec!["2", "1", "-1", "3", "5"],
        svec!["3", "2", "-1", "4", "-2"],
        svec!["4", "3", "1", "5", "-2"],
        svec!["5", "4", "2", "", "-2"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_cumsum() {
    let wrk = Workdir::new("window_cumsum");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("cumsum(n) as cumsum").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "cumsum"],
        svec!["1", "1"],
        svec!["2", "3"],
        svec!["3", "6"],
        svec!["4", "10"],
        svec!["5", "15"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_cummin_cummax() {
    let wrk = Workdir::new("window_cummin_cummax");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["0"],
            svec!["5"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("cummin(n), cummax(n)").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "cummin(n)", "cummax(n)"],
        svec!["1", "1", "1"],
        svec!["2", "1", "2"],
        svec!["3", "1", "3"],
        svec!["0", "0", "3"],
        svec!["5", "0", "5"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_rolling_sum() {
    let wrk = Workdir::new("window_rolling_sum");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("rolling_sum(3, n) as rolling_sum")
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "rolling_sum"],
        svec!["1", ""],
        svec!["2", ""],
        svec!["3", "6"],
        svec!["4", "9"],
        svec!["5", "12"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_all() {
    let wrk = Workdir::new("window_all");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n", "group"],
            svec!["1", "one"],
            svec!["2", "one"],
            svec!["3", "one"],
            svec!["4", "one"],
            svec!["5", "one"],
            svec!["6", "two"],
            svec!["7", "two"],
            svec!["8", "two"],
            svec!["9", "two"],
            svec!["10", "two"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("row_number(), lead(n), lead(n, 3), lag(n), lag(n, 3), cumsum(n), rolling_sum(3, n), frac(n, 2)")
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        [
            "n",
            "group",
            "row_number()",
            "lead(n)",
            "lead(n, 3)",
            "lag(n)",
            "lag(n, 3)",
            "cumsum(n)",
            "rolling_sum(3, n)",
            "frac(n, 2)",
        ],
        ["1", "one", "1", "2", "4", "", "", "1", "", "0.02"],
        ["2", "one", "2", "3", "5", "1", "", "3", "", "0.04"],
        ["3", "one", "3", "4", "6", "2", "", "6", "6", "0.05"],
        ["4", "one", "4", "5", "7", "3", "1", "10", "9", "0.07"],
        ["5", "one", "5", "6", "8", "4", "2", "15", "12", "0.09"],
        ["6", "two", "6", "7", "9", "5", "3", "21", "15", "0.11"],
        ["7", "two", "7", "8", "10", "6", "4", "28", "18", "0.13"],
        ["8", "two", "8", "9", "", "7", "5", "36", "21", "0.15"],
        ["9", "two", "9", "10", "", "8", "6", "45", "24", "0.16"],
        ["10", "two", "10", "", "", "9", "7", "55", "27", "0.18"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_groupby() {
    let wrk = Workdir::new("window_groupby");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n", "group"],
            svec!["1", "one"],
            svec!["2", "one"],
            svec!["3", "one"],
            svec!["4", "one"],
            svec!["5", "one"],
            svec!["6", "two"],
            svec!["7", "two"],
            svec!["8", "two"],
            svec!["9", "two"],
            svec!["10", "two"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("row_number(), lead(n), lead(n, 3), lag(n), lag(n, 3), cumsum(n), rolling_sum(3, n)")
        .args(["-g", "group"])
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        [
            "n",
            "group",
            "row_number()",
            "lead(n)",
            "lead(n, 3)",
            "lag(n)",
            "lag(n, 3)",
            "cumsum(n)",
            "rolling_sum(3, n)",
        ],
        ["1", "one", "1", "2", "4", "", "", "1", ""],
        ["2", "one", "2", "3", "5", "1", "", "3", ""],
        ["3", "one", "3", "4", "", "2", "", "6", "6"],
        ["4", "one", "4", "5", "", "3", "1", "10", "9"],
        ["5", "one", "5", "", "", "4", "2", "15", "12"],
        ["6", "two", "1", "7", "9", "", "", "6", ""],
        ["7", "two", "2", "8", "10", "6", "", "13", ""],
        ["8", "two", "3", "9", "", "7", "", "21", "21"],
        ["9", "two", "4", "10", "", "8", "6", "30", "24"],
        ["10", "two", "5", "", "", "9", "7", "40", "27"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_frac() {
    let wrk = Workdir::new("window_frac");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec![""],
            svec!["4"],
            svec!["5"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("frac(n, 2) as frac").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "frac"],
        svec!["1", "0.07"],
        svec!["2", "0.13"],
        svec!["3", "0.20"],
        svec!["", ""],
        svec!["4", "0.27"],
        svec!["5", "0.33"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_dense_rank() {
    let wrk = Workdir::new("window_dense_rank");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["20"],
            svec!["10"],
            svec!["30"],
            svec!["10"],
            svec!["20"],
            svec!["20"],
            svec!["20"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("dense_rank(n) as rank").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "rank"],
        svec!["20", "2"],
        svec!["10", "1"],
        svec!["30", "3"],
        svec!["10", "1"],
        svec!["20", "2"],
        svec!["20", "2"],
        svec!["20", "2"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_rank() {
    let wrk = Workdir::new("window_rank");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["20"],
            svec!["10"],
            svec!["30"],
            svec!["10"],
            svec!["20"],
            svec!["20"],
            svec!["20"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("rank(n) as rank").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "rank"],
        svec!["20", "3"],
        svec!["10", "1"],
        svec!["30", "7"],
        svec!["10", "2"],
        svec!["20", "4"],
        svec!["20", "5"],
        svec!["20", "6"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_advanced_ranking() {
    let wrk = Workdir::new("window_advanced_ranking");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["id", "n"],
            svec!["1", "-0.5"],
            svec!["1", "-0.5"],
            svec!["1", "-0.2"],
            svec!["1", "1"],
            svec!["1", "0.5"],
            svec!["2", "-0.3"],
            svec!["2", "-0.2"],
            svec!["2", "0.6"],
            svec!["2", "-0.5"],
            svec!["2", "-0.2"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("cume_dist(n) as cume_dist, ntile(2, n)")
        .args(["-g", "id"])
        .arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["id", "n", "cume_dist", "ntile(2, n)"],
        ["1", "-0.5", "0.4", "1"],
        ["1", "-0.5", "0.4", "1"],
        ["1", "-0.2", "0.6", "1"],
        ["1", "1", "1", "2"],
        ["1", "0.5", "0.8", "2"],
        ["2", "-0.3", "0.4", "1"],
        ["2", "-0.2", "0.8", "1"],
        ["2", "0.6", "1", "2"],
        ["2", "-0.5", "0.2", "1"],
        ["2", "-0.2", "0.8", "2"],
    ];

    assert_eq!(got, expected);
}

#[test]
fn window_generic_agg() {
    let wrk = Workdir::new("window_generic_agg");
    wrk.create(
        "numbers.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );
    let mut cmd = wrk.command("window");
    cmd.arg("mean(n) as mean").arg("numbers.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n", "mean"],
        svec!["1", "3"],
        svec!["2", "3"],
        svec!["3", "3"],
        svec!["4", "3"],
        svec!["5", "3"],
    ];

    assert_eq!(got, expected);
}
