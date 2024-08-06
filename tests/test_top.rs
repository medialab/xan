use crate::workdir::Workdir;

#[test]
fn top() {
    let wrk = Workdir::new("top");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "age"],
            svec!["Sven", "34"],
            svec!["Harold", "12"],
            svec!["Mary", "29"],
        ],
    );
    let mut cmd = wrk.command("top");
    cmd.arg("age").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "age"],
        svec!["Sven", "34"],
        svec!["Mary", "29"],
        svec!["Harold", "12"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("top");
    cmd.arg("age").args(["-l", "2"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "age"],
        svec!["Sven", "34"],
        svec!["Mary", "29"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn top_reverse() {
    let wrk = Workdir::new("top_reverse");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "age"],
            svec!["Sven", "34"],
            svec!["Harold", "12"],
            svec!["Mary", "29"],
        ],
    );
    let mut cmd = wrk.command("top");
    cmd.arg("age").arg("-R").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "age"],
        svec!["Harold", "12"],
        svec!["Mary", "29"],
        svec!["Sven", "34"],
    ];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("top");
    cmd.arg("age").arg("-R").args(["-l", "2"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "age"],
        svec!["Harold", "12"],
        svec!["Mary", "29"],
    ];
    assert_eq!(got, expected);
}
