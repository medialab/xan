use workdir::Workdir;

fn sort_output(data: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut output = Vec::new();
    output.push(data[0].clone());

    let mut rows = data.into_iter().skip(1).collect::<Vec<Vec<String>>>();
    rows.sort_by(|a, b| a[0].cmp(&b[0]));

    output.extend(rows);

    output
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
        svec!["group", "sumA"],
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
        svec!["group", "count()"],
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
        svec!["group", "sum"],
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
        svec!["group", "meanA"],
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
        svec!["group", "maxA", "maxB", "maxC"],
        svec!["x", "1", "2", "3"],
        svec!["y", "2", "3", "4"],
        svec!["z", "3", "6", "7"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn groupby_group_column() {
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
        .arg("sum(value_A) as sumA")
        .arg("--group-column")
        .arg("test")
        .arg("data.csv");

    let got: Vec<Vec<String>> = sort_output(wrk.read_stdout(&mut cmd));
    let expected = vec![
        svec!["test", "sumA"],
        svec!["x", "1"],
        svec!["y", "3"],
        svec!["z", "8"],
    ];
    assert_eq!(got, expected);
}
