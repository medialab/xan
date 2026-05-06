use crate::workdir::Workdir;

#[test]
fn baseline_percentage() {
    let wrk = Workdir::new("baseline_percentage");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "pp32", "pp64"],
            svec!["Q4", "80", "160"],
            svec!["Q8", "100", "200"],
            svec!["BF16", "90", "180"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "pp32", "pp64"],
        svec!["Q4", "80 (-20%)", "160 (-20%)"],
        svec!["Q8", "100", "200"],
        svec!["BF16", "90 (-10%)", "180 (-10%)"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_select_column() {
    let wrk = Workdir::new("baseline_select_column");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "quant", "val"],
            svec!["a", "Q4", "80"],
            svec!["b", "Q8", "100"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").args(["-s", "quant"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "quant", "val"],
        svec!["a", "Q4", "80 (-20%)"],
        svec!["b", "Q8", "100"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_compare_specific_columns() {
    let wrk = Workdir::new("baseline_compare_cols");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "pp32", "pp64", "notes"],
            svec!["Q4", "80", "160", "fast"],
            svec!["Q8", "100", "200", "slow"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").args(["-c", "pp32"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    // only pp32 should be compared, pp64 stays as-is
    let expected = vec![
        svec!["quant", "pp32", "pp64", "notes"],
        svec!["Q4", "80 (-20%)", "160", "fast"],
        svec!["Q8", "100", "200", "slow"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_non_numeric_passthrough() {
    let wrk = Workdir::new("baseline_non_numeric");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val", "label"],
            svec!["Q4", "80", "fast"],
            svec!["Q8", "100", "base"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val", "label"],
        svec!["Q4", "80 (-20%)", "fast"],
        svec!["Q8", "100", "base"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_positive_diff() {
    let wrk = Workdir::new("baseline_positive");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "120"],
            svec!["Q8", "100"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val"],
        svec!["Q4", "120 (+20%)"],
        svec!["Q8", "100"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_case_insensitive() {
    let wrk = Workdir::new("baseline_case_insensitive");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "80"],
            svec!["q8_0", "100"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8_0").arg("-i").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val"],
        svec!["Q4", "80 (-20%)"],
        svec!["q8_0", "100"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_equal_values() {
    let wrk = Workdir::new("baseline_equal");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "100"],
            svec!["Q8", "100"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val"],
        svec!["Q4", "100 (=)"],
        svec!["Q8", "100"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_zero_baseline_positive() {
    let wrk = Workdir::new("baseline_zero_pos");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "50"],
            svec!["Q8", "0"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val"],
        svec!["Q4", "50 (+inf%)"],
        svec!["Q8", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_zero_baseline_negative() {
    let wrk = Workdir::new("baseline_zero_neg");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "-30"],
            svec!["Q8", "0"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val"],
        svec!["Q4", "-30 (-inf%)"],
        svec!["Q8", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_zero_both() {
    let wrk = Workdir::new("baseline_zero_both");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "0"],
            svec!["Q8", "0"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("Q8").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["quant", "val"],
        svec!["Q4", "0 (=)"],
        svec!["Q8", "0"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn baseline_no_match_error() {
    let wrk = Workdir::new("baseline_no_match");
    wrk.create(
        "data.csv",
        vec![
            svec!["quant", "val"],
            svec!["Q4", "80"],
            svec!["Q8", "100"],
        ],
    );
    let mut cmd = wrk.command("baseline");
    cmd.arg("NONEXISTENT").arg("data.csv");

    wrk.assert_err(&mut cmd);
}
