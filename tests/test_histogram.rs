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
    cmd.args(&["--limit", "0"]).args(&["--select", "1"]).arg("--no-headers").args(&["--screen-size", "80"]);

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got = got.into_iter().collect();
    let expected = vec![
        ["                    1                                        \u{200e}  nb_lines | %     "],
        ["                    a\u{200e} ████████████████████....................        3 | 50.00"],
        ["                    b\u{200e} █████████████▎..........................        2 | 33.33"],
        ["                   h1\u{200e} ██████▋.................................        1 | 16.67"],
        ["                      Histogram for 6/6 lines and 3/3 categories."]
    ];
    assert_eq!(got, expected);
}
