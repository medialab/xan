use std::process;

use workdir::Workdir;

fn setup(name: &str) -> (Workdir, process::Command) {
    let rows = vec![
        svec!["h1", "h2"],
        svec!["1", "c"],
        svec!["3", "d"],
        svec!["2", "z"],
        svec!["1", "y"],
        svec!["", "y"],
    ];

    let wrk = Workdir::new(name);
    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("dist");
    cmd.arg("h1").arg("in.csv");

    (wrk, cmd)
}

#[test]
fn dist_basic() {
    let (wrk, mut cmd) = setup("dist_no_headers");
    cmd.args(&["--screen-size", "80"]);

    let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    got = got.into_iter().collect();
    let expected = vec![
        ["                                                               nb_lines | %     "],
        [" 1.00 - 1.20 ███████████████████▌.............................        2 | 40.00"],
        [" 1.20 - 1.40 .................................................        0 | 0.00"],
        [" 1.40 - 1.60 .................................................        0 | 0.00"],
        [" 1.60 - 1.80 .................................................        0 | 0.00"],
        [" 1.80 - 2.00 █████████▊.......................................        1 | 20.00"],
        [" 2.00 - 2.21 .................................................        0 | 0.00"],
        [" 2.21 - 2.41 .................................................        0 | 0.00"],
        [" 2.41 - 2.62 .................................................        0 | 0.00"],
        [" 2.62 - 2.82 .................................................        0 | 0.00"],
        [" 2.82 - 3.02 █████████▊.......................................        1 | 20.00"],
        ["        NaNs █████████▊.......................................        1 | 20.00"],
        ["             Distribution for 5/5 lines."]
    ];
    assert_eq!(got, expected);
}
