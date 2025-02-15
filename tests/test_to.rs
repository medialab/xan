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

#[test]
fn to_html() {
    let wrk = Workdir::new("to_html");

    let rows = vec![
        svec!["name", "age"],
        svec!["John", "12"],
        svec!["Lucy", "15"],
    ];

    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("to");
    cmd.arg("html").arg("in.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "<table>
  <thead>
    <tr>
      <th>name</th>
      <th>age</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>John</td>
      <td>12</td>
    </tr>
    <tr>
      <td>Lucy</td>
      <td>15</td>
    </tr>
  </tbody>
</table>";
    assert_eq!(got, expected);
}

#[test]
fn to_md() {
    let wrk = Workdir::new("to_md");

    let rows = vec![
        svec!["name", "age"],
        svec!["John", "12"],
        svec!["Lucy", "15"],
    ];

    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("to");
    cmd.arg("md").arg("in.csv");

    let got: String = wrk.stdout(&mut cmd);
    let expected = "| name | age |
| ---- | --- |
| John | 12  |
| Lucy | 15  |";
    assert_eq!(got, expected);
}
