use crate::workdir::Workdir;

#[test]
fn pivot() {
    let wrk = Workdir::new("pivot");
    wrk.create(
        "data.csv",
        vec![
            svec!["country", "name", "year", "population"],
            svec!["NL", "Amsterdam", "2000", "1005"],
            svec!["NL", "Amsterdam", "2010", "1065"],
            svec!["NL", "Amsterdam", "2020", "1158"],
            svec!["US", "Seattle", "2000", "564"],
            svec!["US", "Seattle", "2010", "608"],
            svec!["US", "Seattle", "2020", "738"],
            svec!["US", "New York City", "2000", "8015"],
            svec!["US", "New York City", "2010", "8175"],
            svec!["US", "New York City", "2020", "8772"],
        ],
    );

    let mut cmd = wrk.command("pivot");
    cmd.arg("year").arg("first(population)").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["country", "name", "2000", "2010", "2020"],
        svec!["NL", "Amsterdam", "1005", "1065", "1158"],
        svec!["US", "Seattle", "564", "608", "738"],
        svec!["US", "New York City", "8015", "8175", "8772"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn pivot_groupby() {
    let wrk = Workdir::new("pivot_groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["country", "name", "year", "population"],
            svec!["NL", "Amsterdam", "2000", "1005"],
            svec!["NL", "Amsterdam", "2010", "1065"],
            svec!["NL", "Amsterdam", "2020", "1158"],
            svec!["US", "Seattle", "2000", "564"],
            svec!["US", "Seattle", "2010", "608"],
            svec!["US", "Seattle", "2020", "738"],
            svec!["US", "New York City", "2000", "8015"],
            svec!["US", "New York City", "2010", "8175"],
            svec!["US", "New York City", "2020", "8772"],
        ],
    );

    let mut cmd = wrk.command("pivot");
    cmd.arg("year")
        .arg("sum(population)")
        .args(["-g", "country"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["country", "2000", "2010", "2020"],
        svec!["NL", "1005", "1065", "1158"],
        svec!["US", "8579", "8783", "9510"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn pivot_missing() {
    let wrk = Workdir::new("pivot_missing");
    wrk.create(
        "data.csv",
        vec![
            svec!["country", "name", "year", "population"],
            svec!["NL", "Amsterdam", "2010", "1065"],
            svec!["NL", "Amsterdam", "2020", "1158"],
            svec!["US", "Seattle", "2000", "564"],
            svec!["US", "Seattle", "2020", "738"],
            svec!["US", "New York City", "2000", "8015"],
            svec!["US", "New York City", "2010", "8175"],
        ],
    );

    let mut cmd = wrk.command("pivot");
    cmd.arg("year").arg("first(population)").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["country", "name", "2010", "2020", "2000"],
        svec!["NL", "Amsterdam", "1065", "1158", ""],
        svec!["US", "Seattle", "", "738", "564"],
        svec!["US", "New York City", "8175", "", "8015"],
    ];
    assert_eq!(got, expected);
}
