use crate::workdir::Workdir;

#[test]
fn regex_join() {
    let wrk = Workdir::new("regex_join");
    wrk.create(
        "people.csv",
        vec![
            svec!["pattern", "name"],
            svec!["john", "John"],
            svec!["lisa", "Lisa"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["person", "color"],
            svec!["jack laurel", "brown"],
            svec!["john cannon", "blue"],
            svec!["lisa eckart", "purple"],
            svec!["lil john", "red"],
            svec!["mina harker", "yellow"],
        ],
    );

    let mut cmd = wrk.command("regex-join");
    cmd.args(["person", "colors.csv", "pattern", "people.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "pattern", "name"],
        svec!["john cannon", "blue", "john", "John"],
        svec!["lisa eckart", "purple", "lisa", "Lisa"],
        svec!["lil john", "red", "john", "John"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn regex_join_multiselect() {
    let wrk = Workdir::new("regex_join_multiselect");
    wrk.create(
        "people.csv",
        vec![
            svec!["pattern", "name"],
            svec!["john", "John"],
            svec!["lisa", "Lisa"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["person", "color", "surname"],
            svec!["jack laurel", "brown", "bear"],
            svec!["john cannon", "blue", "gladys"],
            svec!["lisa eckart", "purple", "john"],
            svec!["lil john", "red", "bear"],
            svec!["mina harker", "yellow", "lisa"],
        ],
    );

    let mut cmd = wrk.command("regex-join");
    cmd.args(["person,surname", "colors.csv", "pattern", "people.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "surname", "pattern", "name"],
        svec!["john cannon", "blue", "gladys", "john", "John"],
        svec!["lisa eckart", "purple", "john", "john", "John"],
        svec!["lisa eckart", "purple", "john", "lisa", "Lisa"],
        svec!["lil john", "red", "bear", "john", "John"],
        svec!["mina harker", "yellow", "lisa", "lisa", "Lisa"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn regex_join_parallel() {
    let wrk = Workdir::new("regex_join_parallel");
    wrk.create(
        "people.csv",
        vec![
            svec!["pattern", "name"],
            svec!["john", "John"],
            svec!["lisa", "Lisa"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["person", "color"],
            svec!["jack laurel", "brown"],
            svec!["john cannon", "blue"],
            svec!["lisa eckart", "purple"],
            svec!["lil john", "red"],
            svec!["mina harker", "yellow"],
        ],
    );

    let mut cmd = wrk.command("regex-join");
    cmd.arg("-p")
        .args(["person", "colors.csv", "pattern", "people.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "pattern", "name"],
        svec!["john cannon", "blue", "john", "John"],
        svec!["lisa eckart", "purple", "lisa", "Lisa"],
        svec!["lil john", "red", "john", "John"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn regex_join_left() {
    let wrk = Workdir::new("regex_join_left");
    wrk.create(
        "people.csv",
        vec![
            svec!["pattern", "name", "age"],
            svec!["john", "John", "4"],
            svec!["lisa", "Lisa", "5"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["person", "color"],
            svec!["jack laurel", "brown"],
            svec!["john cannon", "blue"],
            svec!["lisa eckart", "purple"],
            svec!["lil john", "red"],
            svec!["mina harker", "yellow"],
        ],
    );

    let mut cmd = wrk.command("regex-join");
    cmd.arg("--left")
        .args(["person", "colors.csv", "pattern", "people.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "pattern", "name", "age"],
        svec!["jack laurel", "brown", "", "", ""],
        svec!["john cannon", "blue", "john", "John", "4"],
        svec!["lisa eckart", "purple", "lisa", "Lisa", "5"],
        svec!["lil john", "red", "john", "John", "4"],
        svec!["mina harker", "yellow", "", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn regex_join_ignore_case() {
    let wrk = Workdir::new("regex_join_ignore_case");
    wrk.create(
        "people.csv",
        vec![
            svec!["pattern", "name"],
            svec!["john", "John"],
            svec!["lisa", "Lisa"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["person", "color"],
            svec!["jack laurel", "brown"],
            svec!["JOHN cannon", "blue"],
            svec!["LiSa eckart", "purple"],
            svec!["lil jOHn", "red"],
            svec!["mina harker", "yellow"],
        ],
    );

    let mut cmd = wrk.command("regex-join");
    cmd.arg("-i")
        .args(["person", "colors.csv", "pattern", "people.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "pattern", "name"],
        svec!["JOHN cannon", "blue", "john", "John"],
        svec!["LiSa eckart", "purple", "lisa", "Lisa"],
        svec!["lil jOHn", "red", "john", "John"],
    ];
    assert_eq!(got, expected);
}
