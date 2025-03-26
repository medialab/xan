use crate::workdir::Workdir;

#[test]
fn url_join() {
    let wrk = Workdir::new("url_join");
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

    let mut cmd = wrk.command("url-join");
    cmd.args(["link", "links.csv", "url", "medias.csv"]);

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
fn url_join_left() {
    let wrk = Workdir::new("url_join_left");
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

    let mut cmd = wrk.command("url-join");
    cmd.args(["link", "links.csv", "url", "medias.csv"])
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
fn url_join_prefix() {
    let wrk = Workdir::new("url_join_prefix");
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

    let mut cmd = wrk.command("url-join");
    cmd.args(["link", "links.csv", "url", "medias.csv"])
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
