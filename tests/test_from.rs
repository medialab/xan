use crate::workdir::Workdir;

#[test]
fn from_ndjson() {
    let wrk = Workdir::new("from_ndjson");
    wrk.write(
        "data.ndjson",
        "{\"name\": \"john\", \"age\": 34}\n{\"age\": 56, \"surname\": \"landis\"}",
    );

    let mut cmd = wrk.command("from");
    cmd.arg("data.ndjson");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name", "age", "surname"],
        ["john", "34", ""],
        ["", "56", "landis"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn from_ndjson_model() {
    let wrk = Workdir::new("from_ndjson_model");
    wrk.write(
        "data.ndjson",
        "{\"name\": \"john\", \"age\": 34}\n{\"age\": 56, \"surname\": \"landis\"}",
    );

    let mut cmd = wrk.command("from");
    cmd.arg("data.ndjson")
        .args(["--model", "{\"name\": \"name\"}"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], [""]];
    assert_eq!(got, expected);
}

#[test]
fn from_json_model() {
    let wrk = Workdir::new("from_json_model");
    wrk.write(
        "data.json",
        "[{\"name\": \"john\", \"age\": 34}, {\"age\": 56, \"surname\": \"landis\"}]",
    );

    let mut cmd = wrk.command("from");
    cmd.arg("data.json")
        .args(["--model", "{\"name\": \"name\"}"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["name"], ["john"], [""]];
    assert_eq!(got, expected);
}

#[test]
fn from_json_root() {
    let wrk = Workdir::new("from_json_root");
    wrk.write(
        "data.json",
        "{\"data\":[{\"name\": \"john\", \"age\": 34}, {\"age\": 56, \"surname\": \"landis\"}]}",
    );

    let mut cmd = wrk.command("from");
    cmd.arg("data.json").args(["--root", "data"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["name", "age", "surname"],
        ["john", "34", ""],
        ["", "56", "landis"],
    ];
    assert_eq!(got, expected);
}
