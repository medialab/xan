use crate::workdir::Workdir;

#[test]
fn unpivot() {
    let wrk = Workdir::new("unpivot");
    wrk.create(
        "data.csv",
        vec![
            svec!["dept", "jan", "feb", "mar"],
            svec!["electronics", "1", "2", "3"],
            svec!["clothes", "10", "20", "30"],
            svec!["cars", "100", "200", "300"],
        ],
    );
    let mut cmd = wrk.command("unpivot");
    cmd.arg("jan:")
        .args(["-N", "month"])
        .args(["-V", "count"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["dept", "month", "count"],
        svec!["electronics", "jan", "1"],
        svec!["electronics", "feb", "2"],
        svec!["electronics", "mar", "3"],
        svec!["clothes", "jan", "10"],
        svec!["clothes", "feb", "20"],
        svec!["clothes", "mar", "30"],
        svec!["cars", "jan", "100"],
        svec!["cars", "feb", "200"],
        svec!["cars", "mar", "300"],
    ];
    assert_eq!(got, expected);
}
