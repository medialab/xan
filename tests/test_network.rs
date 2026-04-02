use crate::workdir::Workdir;

#[test]
fn network() {
    let wrk = Workdir::new("network");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .args(["source", "target"])
        .args(["-f", "nodelist"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["node"], ["A"], ["B"], ["C"]];
    assert_eq!(got, expected);
}

#[test]
fn network_simple() {
    let wrk = Workdir::new("network_simple");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .arg("--simple")
        .args(["source", "target"])
        .args(["-f", "nodelist"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![["node"], ["A"], ["B"], ["C"]];
    assert_eq!(got, expected);
}

#[test]
fn network_stats() {
    let wrk = Workdir::new("network_stats");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .arg("-U")
        .args(["source", "target"])
        .args(["-f", "stats"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        [
            "type",
            "nodes",
            "edges",
            "is_multi",
            "has_self_loops",
            "density",
            "connected_components",
            "largest_connected_component",
        ],
        [
            "undirected",
            "3",
            "2",
            "no",
            "no",
            "0.6666666666666666",
            "1",
            "3",
        ],
    ];
    assert_eq!(got, expected);
}

#[test]
fn network_gexf() {
    let wrk = Workdir::new("network_gexf");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .args(["source", "target"])
        .args(["-f", "gexf"])
        .arg("data.csv");

    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(
        got.trim(),
        "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>
<gexf xmlns=\"http://www.gexf.net/1.2draft\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.gexf.net/1.2draft http://www.gexf.net/1.2draft/gexf.xsd\" version=\"1.2\">
  <meta lastmodifieddate=\"2026-04-02\">
    <creator>xan</creator>
  </meta>
  <graph defaultedgetype=\"directed\">
    <attributes class=\"node\">
    </attributes>
    <attributes class=\"edge\">
    </attributes>
    <nodes>
      <node id=\"A\" label=\"A\"/>
      <node id=\"B\" label=\"B\"/>
      <node id=\"C\" label=\"C\"/>
    </nodes>
    <edges>
      <edge source=\"A\" target=\"B\"/>
      <edge source=\"B\" target=\"C\"/>
    </edges>
  </graph>
</gexf>");
}

#[test]
fn network_json() {
    let wrk = Workdir::new("network_json");
    wrk.create(
        "data.csv",
        vec![svec!["source", "target"], svec!["A", "B"], svec!["B", "C"]],
    );
    let mut cmd = wrk.command("network");
    cmd.arg("edgelist")
        .args(["source", "target"])
        .args(["-f", "json"])
        .arg("--minify")
        .arg("data.csv");

    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(
        got.trim(),
        "{\"options\":{\"allowSelfLoops\":false,\"multi\":false,\"type\":\"directed\"},\"nodes\":[{\"key\":\"A\"},{\"key\":\"B\"},{\"key\":\"C\"}],\"edges\":[{\"source\":\"A\",\"target\":\"B\"},{\"source\":\"B\",\"target\":\"C\"}]}"
    );
}
