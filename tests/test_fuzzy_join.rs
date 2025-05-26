use crate::workdir::Workdir;

#[test]
fn fuzzy_join() {
    let wrk = Workdir::new("fuzzy_join_regex");
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

    let mut cmd = wrk.command("fuzzy-join");
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
fn fuzzy_join_regex() {
    let wrk = Workdir::new("fuzzy_join_regex");
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

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("--regex")
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
fn fuzzy_join_regex_multiselect() {
    let wrk = Workdir::new("fuzzy_join_regex_multiselect");
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

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("--regex")
        .args(["person,surname", "colors.csv", "pattern", "people.csv"]);
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
fn fuzzy_join_regex_parallel() {
    let wrk = Workdir::new("fuzzy_join_regex_parallel");
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

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("--regex")
        .arg("-p")
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
fn fuzzy_join_regex_left() {
    let wrk = Workdir::new("fuzzy_join_regex_left");
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

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("--regex")
        .arg("--left")
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
fn fuzzy_join_regex_ignore_case() {
    let wrk = Workdir::new("fuzzy_join_regex_ignore_case");
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

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("--regex")
        .arg("-i")
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

#[test]
fn fuzzy_join_url() {
    let wrk = Workdir::new("fuzzy_join_url");
    wrk.create(
        "medias.csv",
        vec![
            svec!["name", "url"],
            svec!["Le Monde", "lemonde.fr"],
            svec!["Pixels", "lemonde.fr/pixels"],
            svec!["Le Figaro", "lefigaro.fr"],
        ],
    );
    wrk.create(
        "links.csv",
        vec![
            svec!["link"],
            svec!["liberation.fr/article.html"],
            svec!["lemonde.fr/article.html"],
            svec!["lemonde.fr/pixels/article.html"],
            svec!["lefigaro.fr"],
        ],
    );

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("-u")
        .args(["link", "links.csv", "url", "medias.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["link", "name", "url"],
        svec!["lemonde.fr/article.html", "Le Monde", "lemonde.fr"],
        svec![
            "lemonde.fr/pixels/article.html",
            "Pixels",
            "lemonde.fr/pixels"
        ],
        svec!["lefigaro.fr", "Le Figaro", "lefigaro.fr"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn fuzzy_join_url_left() {
    let wrk = Workdir::new("fuzzy_join_url_left");
    wrk.create(
        "medias.csv",
        vec![
            svec!["name", "url"],
            svec!["Le Monde", "lemonde.fr"],
            svec!["Pixels", "lemonde.fr/pixels"],
            svec!["Le Figaro", "lefigaro.fr"],
        ],
    );
    wrk.create(
        "links.csv",
        vec![
            svec!["link"],
            svec!["liberation.fr/article.html"],
            svec!["lemonde.fr/article.html"],
            svec!["lemonde.fr/pixels/article.html"],
            svec!["lefigaro.fr"],
        ],
    );

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("-u")
        .args(["link", "links.csv", "url", "medias.csv"])
        .arg("--left");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["link", "name", "url"],
        svec!["liberation.fr/article.html", "", ""],
        svec!["lemonde.fr/article.html", "Le Monde", "lemonde.fr"],
        svec![
            "lemonde.fr/pixels/article.html",
            "Pixels",
            "lemonde.fr/pixels"
        ],
        svec!["lefigaro.fr", "Le Figaro", "lefigaro.fr"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn fuzzy_join_url_prefix() {
    let wrk = Workdir::new("fuzzy_join_url_prefix");
    wrk.create(
        "medias.csv",
        vec![
            svec!["name", "url"],
            svec!["Le Monde", "lemonde.fr"],
            svec!["Pixels", "lemonde.fr/pixels"],
            svec!["Le Figaro", "lefigaro.fr"],
        ],
    );
    wrk.create(
        "links.csv",
        vec![
            svec!["link"],
            svec!["liberation.fr/article.html"],
            svec!["lemonde.fr/article.html"],
            svec!["lemonde.fr/pixels/article.html"],
            svec!["lefigaro.fr"],
        ],
    );

    let mut cmd = wrk.command("fuzzy-join");
    cmd.arg("-u")
        .args(["link", "links.csv", "url", "medias.csv"])
        .args(["-L", "left_"])
        .args(["-R", "right_"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["left_link", "right_name", "right_url"],
        svec!["lemonde.fr/article.html", "Le Monde", "lemonde.fr"],
        svec![
            "lemonde.fr/pixels/article.html",
            "Pixels",
            "lemonde.fr/pixels"
        ],
        svec!["lefigaro.fr", "Le Figaro", "lefigaro.fr"],
    ];
    assert_eq!(got, expected);
}
