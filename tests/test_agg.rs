use crate::workdir::Workdir;

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
    test_single_agg_function(&wrk, "count(n > 2) as count", "count", "2");
    test_single_agg_function(&wrk, "ratio(n > 2) as count", "count", "0.5");
    test_single_agg_function(&wrk, "percentage(n > 2) as count", "count", "50%");
    test_single_agg_function(&wrk, "count(n eq '') as count", "count", "0");
    test_single_agg_function(&wrk, "sum(n) as sum", "sum", "10");
    test_single_agg_function(&wrk, "mean(n) as mean", "mean", "2.5");
    test_single_agg_function(&wrk, "avg(n) as mean", "mean", "2.5");
    test_single_agg_function(&wrk, "min(n) as min", "min", "1");
    test_single_agg_function(&wrk, "max(n) as max", "max", "4");
    test_single_agg_function(&wrk, "median(n) as median", "median", "2.5");
    test_single_agg_function(&wrk, "q2(n) as q2", "q2", "2.5");
    test_single_agg_function(&wrk, "quantile(n, 0.5) as q2", "q2", "2.5");
    test_single_agg_function(&wrk, "q1(n) as q1", "q1", "1.75");
    test_single_agg_function(&wrk, "q3(n) as q3", "q3", "3.25");
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
    test_single_agg_function(&wrk, "all(n >= 2) as all", "all", "false");
    test_single_agg_function(&wrk, "all(n >= 1) as all", "all", "true");
    test_single_agg_function(&wrk, "any(n >= 1) as any", "any", "true");
    test_single_agg_function(&wrk, "any(n >= 5) as any", "any", "false");
    test_single_agg_function(&wrk, "first(n) as first", "first", "1");
    test_single_agg_function(&wrk, "last(n) as last", "last", "4");
}

#[test]
fn agg_first_last() {
    let wrk = Workdir::new("agg_first_last");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec![""],
            svec!["1"],
            svec![""],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec![""],
            svec!["6"],
            svec![""],
        ],
    );

    test_single_agg_function(&wrk, "first(n) as first", "first", "1");
    test_single_agg_function(&wrk, "last(n) as last", "last", "6");
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
    cmd.arg(
        "count() as total_count, count(n ne '') as count_without_nulls, count(n eq '') as count_nulls",
    )
    .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["total_count", "count_without_nulls", "count_nulls"],
        svec!["4", "3", "1"],
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
    cmd.arg("sum(add(a, b + 1)) as sum").arg("data.csv");

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

#[test]
fn agg_parallel() {
    let wrk = Workdir::new("agg_parallel");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["6"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("sum(n) as sum").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["sum"], svec!["16"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_types() {
    let wrk = Workdir::new("agg_types");
    wrk.create(
        "data.csv",
        vec![
            svec!["I", "E", "S", "M", "F"],
            svec!["1", "", "test1", "", "2"],
            svec!["2", "", "test2", "3", "2.5"],
            svec!["", "", "test3", "3.5", "1"],
            svec!["4", "", "4", "test", ""],
            svec!["5", "", "test5", "string", "5.6"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("type(I) as I, type(E) as E, type(S) as S, type(M) as M, type(F) as F")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["I", "E", "S", "M", "F"],
        svec!["int", "empty", "string", "string", "float"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("agg");
    cmd.arg("types(I) as I, types(E) as E, types(S) as S, types(M) as M, types(F) as F")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["I", "E", "S", "M", "F"],
        svec![
            "int|empty",
            "empty",
            "int|string",
            "int|float|string|empty",
            "int|float|empty"
        ],
    ];
    assert_eq!(got, expected);
}

#[test]
fn agg_values() {
    let wrk = Workdir::new("agg_values");
    wrk.create(
        "data.csv",
        vec![svec!["name"], svec!["John"], svec!["Mary"], svec!["Lucas"]],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("values(name) as V").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["V"], svec!["John|Mary|Lucas"]];
    assert_eq!(got, expected);

    // Custom separator
    let mut cmd = wrk.command("agg");
    cmd.arg("values(name, '~') as V").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["V"], svec!["John~Mary~Lucas"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_values_identity() {
    let wrk = Workdir::new("agg_values_identity");
    wrk.create(
        "data.csv",
        vec![svec!["name"], svec!["John"], svec!["Mary"], svec!["Lucas"]],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("values(name) as V1, values(name, '-') as V2")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["V1", "V2"],
        svec!["John|Mary|Lucas", "John-Mary-Lucas"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn agg_distinct_values() {
    let wrk = Workdir::new("agg_distinct_values");
    wrk.create(
        "data.csv",
        vec![
            svec!["name"],
            svec!["John"],
            svec!["Mary"],
            svec!["Lucas"],
            svec!["Mary"],
            svec!["Lucas"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("distinct_values(name) as V").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["V"], svec!["John|Lucas|Mary"]];
    assert_eq!(got, expected);

    // Custom separator
    let mut cmd = wrk.command("agg");
    cmd.arg("distinct_values(name, '~') as V").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["V"], svec!["John~Lucas~Mary"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_arg_extent() {
    let wrk = Workdir::new("agg_arg_extent");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "n"],
            svec!["John", "3"],
            svec!["Mary", "1"],
            svec!["Lucas", "2"],
        ],
    );

    // Implicit index
    let mut cmd = wrk.command("agg");
    cmd.arg("argmin(n) as argmin, argmax(n) as argmax")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["argmin", "argmax"], svec!["1", "0"]];
    assert_eq!(got, expected);

    // Expression
    let mut cmd = wrk.command("agg");
    cmd.arg("argmin(n, name) as argmin, argmax(n, name) as argmax")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["argmin", "argmax"], svec!["Mary", "John"]];
    assert_eq!(got, expected);

    // Testing interference with min/max (in both orders)
    let mut cmd = wrk.command("agg");
    cmd.arg("argmin(n, name) as argmin, min(n) as min")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["argmin", "min"], svec!["Mary", "1"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("agg");
    cmd.arg("min(n) as min, argmin(n, name) as argmin")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["min", "argmin"], svec!["1", "Mary"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_most_common() {
    let wrk = Workdir::new("agg_most_common");
    wrk.create(
        "data.csv",
        vec![
            svec!["color"],
            svec!["blue"],
            svec!["blue"],
            svec!["red"],
            svec!["blue"],
            svec!["yellow"],
            svec!["red"],
            svec!["purple"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("most_common(3, color) as top, most_common_counts(3, color) as top_counts")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["top", "top_counts"],
        svec!["blue|red|purple", "3|2|1"],
    ];
    assert_eq!(got, expected);

    // Custom separator
    let mut cmd = wrk.command("agg");
    cmd.arg("most_common(3, color, '§') as top, most_common_counts(3, color, '§') as top_counts")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["top", "top_counts"],
        svec!["blue§red§purple", "3§2§1"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn agg_top() {
    let wrk = Workdir::new("agg_top");
    wrk.create(
        "data.csv",
        vec![
            svec!["score", "color"],
            svec!["1", "blue"],
            svec!["3", "red"],
            svec!["2", "yellow"],
            svec!["2", "purple"],
            svec!["4", "ochre"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("top(3, score) as top, argtop(3, score, color) as argtop")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["top", "argtop"], svec!["4|3|2", "ochre|red|yellow"]];
    assert_eq!(got, expected);

    // Custom separator
    let mut cmd = wrk.command("agg");
    cmd.arg("top(3, score, ',') as top, argtop(3, score, color, ',') as argtop")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["top", "argtop"], svec!["4,3,2", "ochre,red,yellow"]];
    assert_eq!(got, expected);
}

#[test]
fn agg_argtop() {
    let wrk = Workdir::new("agg_argtop");
    wrk.create(
        "data.csv",
        vec![
            svec!["score", "color"],
            svec!["1", "blue"],
            svec!["3", "red"],
            svec!["2", "yellow"],
            svec!["2", "purple"],
            svec!["4", "ochre"],
        ],
    );

    let mut cmd = wrk.command("agg");
    cmd.arg("argtop(3, score) as top").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["top"], svec!["4|1|2"]];
    assert_eq!(got, expected);

    // Auxilliary expr
    let mut cmd = wrk.command("agg");
    cmd.arg("argtop(3, score, color) as top").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["top"], svec!["ochre|red|yellow"]];
    assert_eq!(got, expected);

    // Custom separator
    let mut cmd = wrk.command("agg");
    cmd.arg("argtop(3, score, index(), ',') as top")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["top"], svec!["4,1,2"]];
    assert_eq!(got, expected);

    // Custom separator with auxilliary expr
    let mut cmd = wrk.command("agg");
    cmd.arg("argtop(3, score, color, ',') as top")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["top"], svec!["ochre,red,yellow"]];
    assert_eq!(got, expected);
}
