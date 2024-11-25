use crate::workdir::Workdir;

#[test]
fn to_json() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", ""]];

    let wrk = Workdir::new("to_json");
    wrk.create("in1.csv", rows1);

    let mut cmd = wrk.command("to");
    cmd.arg("json").arg("in1.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "[
  {
    \"h1\": \"a\",
    \"h2\": \"\"
  }
]";
    assert_eq!(got, expected);
}

#[test]
fn to_json_nulls() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", ""]];

    let wrk = Workdir::new("to_json_nulls");
    wrk.create("in1.csv", rows1);

    let mut cmd = wrk.command("to");
    cmd.arg("json").arg("--nulls").arg("in1.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "[
  {
    \"h1\": \"a\",
    \"h2\": null
  }
]";
    assert_eq!(got, expected);
}

#[test]
fn to_json_omit() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", ""], svec!["c", "d"]];

    let wrk = Workdir::new("to_json_omit");
    wrk.create("in1.csv", rows1);

    let mut cmd = wrk.command("to");
    cmd.arg("json").arg("--omit").arg("in1.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "[
  {
    \"h1\": \"a\"
  },
  {
    \"h1\": \"c\",
    \"h2\": \"d\"
  }
]";
    assert_eq!(got, expected);
}

#[test]
fn to_ndjson() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", ""], svec!["c", "d"]];

    let wrk = Workdir::new("to_ndjson");
    wrk.create("in1.csv", rows1);

    let mut cmd = wrk.command("to");
    cmd.arg("ndjson").arg("in1.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "{\"h1\":\"a\",\"h2\":\"\"}\n{\"h1\":\"c\",\"h2\":\"d\"}";
    assert_eq!(got, expected);
}

#[test]
fn to_ndjson_nulls() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", ""], svec!["c", "d"]];

    let wrk = Workdir::new("to_ndjson_nulls");
    wrk.create("in1.csv", rows1);

    let mut cmd = wrk.command("to");
    cmd.arg("ndjson").arg("--nulls").arg("in1.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "{\"h1\":\"a\",\"h2\":null}\n{\"h1\":\"c\",\"h2\":\"d\"}";
    assert_eq!(got, expected);
}

#[test]
fn to_ndjson_omit() {
    let rows1 = vec![svec!["h1", "h2"], svec!["a", ""], svec!["c", "d"]];

    let wrk = Workdir::new("to_ndjson_omit");
    wrk.create("in1.csv", rows1);

    let mut cmd = wrk.command("to");
    cmd.arg("ndjson").arg("--omit").arg("in1.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "{\"h1\":\"a\"}\n{\"h1\":\"c\",\"h2\":\"d\"}";
    assert_eq!(got, expected);
}
