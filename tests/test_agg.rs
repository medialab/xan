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
    test_single_agg_function(&wrk, "avg(n) as mean", "mean", "2.5");
    test_single_agg_function(&wrk, "min(n) as min", "min", "1");
    test_single_agg_function(&wrk, "max(n) as max", "max", "4");
    test_single_agg_function(&wrk, "median(n) as median", "median", "2.5");
    test_single_agg_function(&wrk, "median_low(n) as median", "median", "2");
    test_single_agg_function(&wrk, "median_high(n) as median", "median", "3");
    test_single_agg_function(&wrk, "var(n) as variance", "variance", "1.25");
    test_single_agg_function(&wrk, "var_pop(n) as variance", "variance", "1.25");
    test_single_agg_function(
        &wrk,
        "var_sample(n) as variance",
        "variance",
        "1.6666666666666667",
    );
    test_single_agg_function(&wrk, "stddev(n) as stddev", "stddev", "1.118033988749895");
    test_single_agg_function(
        &wrk,
        "stddev_pop(n) as stddev",
        "stddev",
        "1.118033988749895",
    );
    test_single_agg_function(
        &wrk,
        "stddev_sample(n) as stddev",
        "stddev",
        "1.2909944487358056",
    );
    test_single_agg_function(&wrk, "all(gte(n,2)) as all", "all", "false");
    test_single_agg_function(&wrk, "all(gte(n,1)) as all", "all", "true");
    test_single_agg_function(&wrk, "any(gte(n,1)) as any", "any", "true");
    test_single_agg_function(&wrk, "any(gte(n,5)) as any", "any", "false");
}

#[test]
fn agg_mode_cardinality() {
    let wrk = Workdir::new("agg_mode_cardinality");
    wrk.create(
        "data.csv",
        vec![
            svec!["color"],
            svec!["red"],
            svec!["blue"],
            svec!["yellow"],
            svec!["red"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("mode(color) as mode, cardinality(color) as cardinality")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["mode", "cardinality"], svec!["red", "3"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_sqlish_count() {
    let wrk = Workdir::new("agg_sqlish_count");
    wrk.create(
        "data.csv",
        vec![svec!["n"], svec!["1"], svec!["2"], svec![""], svec!["4"]],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("count() as count_with_nulls, count(n) as count_without_nulls")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["count_with_nulls", "count_without_nulls"],
        svec!["4", "3"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn agg_multiple_columns() {
    let wrk = Workdir::new("agg_multiple_columns");
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
    let wrk = Workdir::new("agg_combinator");
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

#[test]
fn agg_min_max_strings() {
    let wrk = Workdir::new("agg_min_max_strings");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["test"],
            svec!["5"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("lex_first(n) as min, lex_last(n) as max")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["min", "max"], svec!["1", "test"]];
    assert_eq!(got, expected);
}
