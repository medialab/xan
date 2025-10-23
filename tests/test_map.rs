use crate::workdir::Workdir;

#[test]
fn map() {
    let wrk = Workdir::new("map");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b) as c").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_multi() {
    let wrk = Workdir::new("map_multi");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b) as c, mul(a, b) as d").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c", "d"],
        svec!["1", "2", "3", "2"],
        svec!["2", "3", "5", "6"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_index() {
    let wrk = Workdir::new("map_index");
    wrk.create("data.csv", vec![svec!["n"], svec!["10"], svec!["15"]]);

    let mut cmd = wrk.command("map");
    cmd.arg("index() as r").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n", "r"], svec!["10", "0"], svec!["15", "1"]];
    assert_eq!(got, expected);
}

#[test]
fn map_parallel() {
    let wrk = Workdir::new("map_parallel");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b) as c").arg("-p").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_threads() {
    let wrk = Workdir::new("map_threads");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "2"], svec!["2", "3"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("add(a, b) as c").args(["-t", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "2", "3"],
        svec!["2", "3", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_overwrite() {
    let wrk = Workdir::new("map_overwrite");
    wrk.create(
        "data.csv",
        vec![svec!["a", "b"], svec!["1", "4"], svec!["5", "2"]],
    );
    let mut cmd = wrk.command("map");
    cmd.arg("-O").arg("b * 10 as b, a * b as c").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "c"],
        svec!["1", "40", "4"],
        svec!["5", "20", "10"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_filter() {
    let wrk = Workdir::new("map_filter");
    wrk.create(
        "data.csv",
        vec![
            svec!["full_name"],
            svec!["john landis"],
            svec!["béatrice babka"],
        ],
    );
    let mut cmd = wrk.command("map");

    cmd.arg("if(full_name.startswith('j'), full_name.split(' ')[0]) as first_name")
        .arg("--filter")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["full_name", "first_name"],
        svec!["john landis", "john"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn map_plural_clause() {
    let wrk = Workdir::new("map_plural_clause");
    wrk.create(
        "data.csv",
        vec![
            svec!["full_name"],
            svec!["john landis"],
            svec!["béatrice babka"],
        ],
    );
    let mut cmd = wrk.command("map");

    cmd.arg("full_name.split(' ') as (first_name, last_name)")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["full_name", "first_name", "last_name"],
        svec!["john landis", "john", "landis"],
        svec!["béatrice babka", "béatrice", "babka"],
    ];
    assert_eq!(got, expected);
}
