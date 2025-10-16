use crate::workdir::Workdir;

#[test]
fn separate() {
    let wrk = Workdir::new("separate");
    wrk.create(
        "data.csv",
        vec![
            svec!["locution"],
            svec!["a priori"],
            svec!["de facto"],
            svec![""],
            svec!["au cas où"],
            svec![" "],
            svec!["ex-æquo"],
        ],
    );
    let mut cmd = wrk.command("separate");
    cmd.arg("locution").arg(" ").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["locution", "untitled1", "untitled2", "untitled3"],
        svec!["a priori", "a", "priori", ""],
        svec!["de facto", "de", "facto", ""],
        svec!["", "", "", ""],
        svec!["au cas où", "au", "cas", "où"],
        svec![" ", "", "", ""],
        svec!["ex-æquo", "ex-æquo", "", ""],
    ];
    assert_eq!(got, expected);
}