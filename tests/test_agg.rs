use workdir::Workdir;

fn test_single_agg_function(wrk: &Workdir, expr: &str, name: &str, value: &str) {
    let mut cmd = wrk.command("agg");
    cmd.arg(expr).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec![name], svec![value]];
    assert_eq!(got, expected);
}

#[test]
fn agg() {
    let wrk = Workdir::new("agg");
    wrk.create(
        "data.csv",
        vec![svec!["n"], svec!["1"], svec!["2"], svec!["3"], svec!["4"]],
    );

    test_single_agg_function(&wrk, "count() as count", "count", "4");
    test_single_agg_function(&wrk, "sum(n) as sum", "sum", "10");
    test_single_agg_function(&wrk, "mean(n) as mean", "mean", "2.5");
}

#[test]
fn agg_multiple_columns() {
    let wrk = Workdir::new("agg");
    wrk.create(
        "data.csv",
        vec![svec!["n"], svec!["1"], svec!["2"], svec!["3"], svec!["4"]],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("count() as count, sum(n) as sum").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["count", "sum"], svec!["4", "10"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_combinator() {
    let wrk = Workdir::new("agg");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b"],
            svec!["1", "2"],
            svec!["2", "0"],
            svec!["3", "6"],
            svec!["4", "2"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("sum(add(a, inc(b))) as sum").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["sum"], svec!["24"]];
    assert_eq!(got, expected);
}
