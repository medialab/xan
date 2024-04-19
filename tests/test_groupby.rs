use crate::workdir::Workdir;

fn sort_output_on_n_first(data: Vec<Vec<String>>, n: usize) -> Vec<Vec<String>> {
    let mut output = Vec::new();
    output.push(data[0].clone());

    let mut rows = data.into_iter().skip(1).collect::<Vec<Vec<String>>>();
    rows.sort_by(|a, b| a[0..n].cmp(&b[0..n]));

    output.extend(rows);

    output
}

fn sort_output(data: Vec<Vec<String>>) -> Vec<Vec<String>> {
    sort_output_on_n_first(data, 1)
}

#[test]
fn groupby() {
    let wrk = Workdir::new("groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["y", "2", "3", "4"],
            svec!["z", "3", "4", "5"],
            svec!["y", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["z", "3", "6", "7"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id").arg("sum(value_A) as sumA").arg("data.csv");

    let got: Vec<Vec<String>> = sort_output(wrk.read_stdout(&mut cmd));
    let expected = vec![
        svec!["id", "sumA"],
        svec!["x", "1"],
        svec!["y", "3"],
        svec!["z", "8"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_count() {
    let wrk = Workdir::new("groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["y", "2", "3", "4"],
            svec!["z", "3", "4", "5"],
            svec!["y", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["z", "3", "6", "7"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id").arg("count()").arg("data.csv");

    let got: Vec<Vec<String>> = sort_output(wrk.read_stdout(&mut cmd));
    let expected = vec![
        svec!["id", "count()"],
        svec!["x", "1"],
        svec!["y", "2"],
        svec!["z", "3"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_sum() {
    let wrk = Workdir::new("groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["y", "2", "3", "4"],
            svec!["z", "3", "4", "5"],
            svec!["y", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["z", "3", "6", "7"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id")
        .arg("sum(add(value_A,add(value_B,value_C))) as sum")
        .arg("data.csv");

    let got: Vec<Vec<String>> = sort_output(wrk.read_stdout(&mut cmd));
    let expected = vec![
        svec!["id", "sum"],
        svec!["x", "6"],
        svec!["y", "15"],
        svec!["z", "38"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_mean() {
    let wrk = Workdir::new("groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["y", "2", "3", "4"],
            svec!["z", "3", "4", "5"],
            svec!["y", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["z", "3", "6", "7"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id").arg("mean(value_A) as meanA").arg("data.csv");

    let got: Vec<Vec<String>> = sort_output(wrk.read_stdout(&mut cmd));
    let expected = vec![
        svec!["id", "meanA"],
        svec!["x", "1"],
        svec!["y", "1.5"],
        svec!["z", "2.6666666666666665"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_max() {
    let wrk = Workdir::new("groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["y", "2", "3", "4"],
            svec!["z", "3", "4", "5"],
            svec!["y", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["z", "3", "6", "7"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id")
        .arg("max(value_A) as maxA, max(value_B) as maxB,max(value_C) as maxC")
        .arg("data.csv");

    let got: Vec<Vec<String>> = sort_output(wrk.read_stdout(&mut cmd));
    let expected = vec![
        svec!["id", "maxA", "maxB", "maxC"],
        svec!["x", "1", "2", "3"],
        svec!["y", "2", "3", "4"],
        svec!["z", "3", "6", "7"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_sorted() {
    let wrk = Workdir::new("groupby");
    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["y", "2", "3", "4"],
            svec!["y", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["z", "3", "6", "7"],
            svec!["z", "3", "4", "5"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id")
        .arg("sum(value_A) as sumA")
        .arg("--sorted")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "sumA"],
        svec!["x", "1"],
        svec!["y", "3"],
        svec!["z", "8"],
    ];
    assert_eq!(got, expected);

    wrk.create(
        "data.csv",
        vec![svec!["id", "value_A", "value_B", "value_C"]],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id")
        .arg("sum(value_A) as sumA")
        .arg("--sorted")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["id", "sumA"]];
    assert_eq!(got, expected);

    wrk.create(
        "data.csv",
        vec![
            svec!["id", "value_A", "value_B", "value_C"],
            svec!["x", "1", "2", "3"],
            svec!["z", "2", "3", "5"],
            svec!["y", "2", "3", "4"],
            svec!["z", "3", "4", "5"],
            svec!["y", "1", "2", "3"],
            svec!["x", "3", "6", "7"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("id")
        .arg("sum(value_A) as sumA")
        .arg("--sorted")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "sumA"],
        svec!["x", "1"],
        svec!["z", "2"],
        svec!["y", "2"],
        svec!["z", "3"],
        svec!["y", "1"],
        svec!["x", "3"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_complex_selection() {
    let wrk = Workdir::new("groupby_complex_selection");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "color", "count"],
            svec!["john", "blue", "1"],
            svec!["mary", "orange", "3"],
            svec!["mary", "orange", "2"],
            svec!["john", "yellow", "9"],
            svec!["john", "blue", "2"],
        ],
    );

    let mut cmd = wrk.command("groupby");
    cmd.arg("name,color")
        .arg("sum(count) as sum")
        .arg("data.csv");

    let got: Vec<Vec<String>> = sort_output_on_n_first(wrk.read_stdout(&mut cmd), 2);
    let expected = vec![
        svec!["name", "color", "sum"],
        svec!["john", "blue", "3"],
        svec!["john", "yellow", "9"],
        svec!["mary", "orange", "5"],
    ];
    assert_eq!(got, expected);
}
