use std::process;

use workdir::Workdir;

fn setup(name: &str) -> (Workdir, process::Command) {
    let rows = vec![
        svec!["h1", "h2"],
        svec!["b", "c"],
        svec!["b", "d"],
        svec!["a", "z"],
        svec!["a", "y"],
        svec!["a", "y"],
    ];

    let wrk = Workdir::new(name);
    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("histogram");
    cmd.arg("in.csv");

    (wrk, cmd)
}

#[test]
fn histogram_no_headers() {
    let (wrk, mut cmd) = setup("histogram_no_headers");
    cmd.args(&["--limit", "0"]).args(&["--select", "1"]).arg("--no-headers").args(&["--max-size", "56"]);

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got = got.into_iter().skip(1).collect();
    let expected = vec![
        ["a                              ████████████████████████████████████████████████████████ 3 | 100.00"],
        ["b                              █████████████████████████████████████▎.................. 2 | 66.67"],
        ["h1                             ██████████████████▋..................................... 1 | 33.33"],
    ];
    assert_eq!(got, expected);
}
