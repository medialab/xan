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
