use crate::workdir::Workdir;

// This macro takes *two* identifiers: one for the test with headers
// and another for the test without headers.
macro_rules! join_test {
    ($name:ident, $fun:expr) => {
        mod $name {
            use std::process;

            use super::{make_rows, setup};
            use crate::workdir::Workdir;

            #[test]
            fn headers() {
                let wrk = setup(stringify!($name), true);
                let mut cmd = wrk.command("join");
                cmd.args(["-D", "none"])
                    .args(&["city", "cities.csv", "city", "places.csv"]);
                $fun(wrk, cmd, true);
            }

            #[test]
            fn no_headers() {
                let n = stringify!(concat_idents!($name, _no_headers));
                let wrk = setup(n, false);
                let mut cmd = wrk.command("join");
                cmd.args(["-D", "none"]).arg("--no-headers").args(&[
                    "0",
                    "cities.csv",
                    "0",
                    "places.csv",
                ]);
                $fun(wrk, cmd, false);
            }
        }
    };
}

fn setup(name: &str, headers: bool) -> Workdir {
    let mut cities = vec![
        svec!["Boston", "MA"],
        svec!["New York", "NY"],
        svec!["San Francisco", "CA"],
        svec!["Buffalo", "NY"],
    ];
    let mut places = vec![
        svec!["Boston", "Logan Airport"],
        svec!["Boston", "Boston Garden"],
        svec!["Buffalo", "Ralph Wilson Stadium"],
        svec!["Orlando", "Disney World"],
    ];
    if headers {
        cities.insert(0, svec!["city", "state"]);
    }
    if headers {
        places.insert(0, svec!["city", "place"]);
    }

    let wrk = Workdir::new(name);
    wrk.create("cities.csv", cities);
    wrk.create("places.csv", places);
    wrk
}

fn make_rows(headers: bool, rows: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut all_rows = vec![];
    if headers {
        all_rows.push(svec!["city", "state", "city", "place"]);
    }
    all_rows.extend(rows.into_iter());
    all_rows
}

join_test!(join_inner, |wrk: Workdir,
                        mut cmd: process::Command,
                        headers: bool| {
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = make_rows(
        headers,
        vec![
            svec!["Boston", "MA", "Boston", "Logan Airport"],
            svec!["Boston", "MA", "Boston", "Boston Garden"],
            svec!["Buffalo", "NY", "Buffalo", "Ralph Wilson Stadium"],
        ],
    );
    assert_eq!(got, expected);
});

join_test!(
    join_outer_left,
    |wrk: Workdir, mut cmd: process::Command, headers: bool| {
        cmd.arg("--left");
        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = make_rows(
            headers,
            vec![
                svec!["Boston", "MA", "Boston", "Logan Airport"],
                svec!["Boston", "MA", "Boston", "Boston Garden"],
                svec!["New York", "NY", "", ""],
                svec!["San Francisco", "CA", "", ""],
                svec!["Buffalo", "NY", "Buffalo", "Ralph Wilson Stadium"],
            ],
        );
        assert_eq!(got, expected);
    }
);

join_test!(
    join_outer_right,
    |wrk: Workdir, mut cmd: process::Command, headers: bool| {
        cmd.arg("--right");
        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = make_rows(
            headers,
            vec![
                svec!["Boston", "MA", "Boston", "Logan Airport"],
                svec!["Boston", "MA", "Boston", "Boston Garden"],
                svec!["Buffalo", "NY", "Buffalo", "Ralph Wilson Stadium"],
                svec!["", "", "Orlando", "Disney World"],
            ],
        );
        assert_eq!(got, expected);
    }
);

join_test!(
    join_outer_full,
    |wrk: Workdir, mut cmd: process::Command, headers: bool| {
        cmd.arg("--full");
        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = make_rows(
            headers,
            vec![
                svec!["Boston", "MA", "Boston", "Logan Airport"],
                svec!["Boston", "MA", "Boston", "Boston Garden"],
                svec!["Buffalo", "NY", "Buffalo", "Ralph Wilson Stadium"],
                svec!["", "", "Orlando", "Disney World"],
                svec!["New York", "NY", "", ""],
                svec!["San Francisco", "CA", "", ""],
            ],
        );
        assert_eq!(got, expected);
    }
);

#[test]
fn join_inner_issue11() {
    let a = vec![svec!["1", "2"], svec!["3", "4"], svec!["5", "6"]];
    let b = vec![svec!["2", "1"], svec!["4", "3"], svec!["6", "5"]];

    let wrk = Workdir::new("join_inner_issue11");
    wrk.create("a.csv", a);
    wrk.create("b.csv", b);

    let mut cmd = wrk.command("join");
    cmd.arg("-n").args(["0,1", "a.csv", "1,0", "b.csv"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["1", "2", "2", "1"],
        svec!["3", "4", "4", "3"],
        svec!["5", "6", "6", "5"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_cross() {
    let wrk = Workdir::new("join_cross");
    wrk.create(
        "letters.csv",
        vec![svec!["h1", "h2"], svec!["a", "b"], svec!["c", "d"]],
    );
    wrk.create(
        "numbers.csv",
        vec![svec!["h3", "h4"], svec!["1", "2"], svec!["3", "4"]],
    );

    let mut cmd = wrk.command("join");
    cmd.arg("--cross").args(["letters.csv", "numbers.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["h1", "h2", "h3", "h4"],
        svec!["a", "b", "1", "2"],
        svec!["a", "b", "3", "4"],
        svec!["c", "d", "1", "2"],
        svec!["c", "d", "3", "4"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_cross_no_headers() {
    let wrk = Workdir::new("join_cross_no_headers");
    wrk.create("letters.csv", vec![svec!["a", "b"], svec!["c", "d"]]);
    wrk.create("numbers.csv", vec![svec!["1", "2"], svec!["3", "4"]]);

    let mut cmd = wrk.command("join");
    cmd.arg("--cross")
        .arg("--no-headers")
        .args(["", "letters.csv", "", "numbers.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["a", "b", "1", "2"],
        svec!["a", "b", "3", "4"],
        svec!["c", "d", "1", "2"],
        svec!["c", "d", "3", "4"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_prefix() {
    let wrk = Workdir::new("join_prefix");
    wrk.create(
        "fruits.csv",
        vec![
            svec!["idx", "fruit"],
            svec!["1", "apple"],
            svec!["2", "mango"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["idx", "color"],
            svec!["1", "blue"],
            svec!["2", "purple"],
        ],
    );

    let mut cmd = wrk.command("join");
    cmd.arg("--prefix-left")
        .arg("left_")
        .arg("--prefix-right")
        .arg("right_")
        .args(["-D", "none"])
        .args(["idx", "fruits.csv", "idx", "colors.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["left_idx", "left_fruit", "right_idx", "right_color"],
        svec!["1", "apple", "1", "blue"],
        svec!["2", "mango", "2", "purple"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_semi() {
    let wrk = Workdir::new("join_semi");
    wrk.create(
        "fruits.csv",
        vec![
            svec!["id", "fruit"],
            svec!["1", "mango"],
            svec!["2", "orange"],
            svec!["3", "apple"],
            svec!["4", "cherry"],
        ],
    );
    wrk.create(
        "index.csv",
        vec![svec!["fruit"], svec!["mango"], svec!["cherry"]],
    );

    let mut cmd = wrk.command("join");
    cmd.arg("--semi")
        .args(["fruit", "fruits.csv", "fruit", "index.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "fruit"],
        svec!["1", "mango"],
        svec!["4", "cherry"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_anti() {
    let wrk = Workdir::new("join_anti");
    wrk.create(
        "fruits.csv",
        vec![
            svec!["id", "fruit"],
            svec!["1", "mango"],
            svec!["2", "orange"],
            svec!["3", "apple"],
            svec!["4", "cherry"],
        ],
    );
    wrk.create(
        "index.csv",
        vec![svec!["fruit"], svec!["mango"], svec!["cherry"]],
    );

    let mut cmd = wrk.command("join");
    cmd.arg("--anti")
        .args(["fruit", "fruits.csv", "fruit", "index.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["id", "fruit"],
        svec!["2", "orange"],
        svec!["3", "apple"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_combined_arity() {
    let wrk = Workdir::new("join_combined_arity");
    wrk.create(
        "fruits.csv",
        vec![
            svec!["idx", "fruit"],
            svec!["1", "apple"],
            svec!["2", "mango"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["idx", "color"],
            svec!["1", "blue"],
            svec!["2", "purple"],
        ],
    );

    let mut cmd = wrk.command("join");
    cmd.arg("--prefix-left")
        .arg("left_")
        .arg("--prefix-right")
        .arg("right_")
        .args(["idx", "fruits.csv", "colors.csv"]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["left_idx", "left_fruit", "right_color"],
        svec!["1", "apple", "blue"],
        svec!["2", "mango", "purple"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn join_drop_key() {
    let wrk = Workdir::new("join_drop_key");
    wrk.create(
        "fruits.csv",
        vec![
            svec!["idx", "fruit"],
            svec!["1", "apple"],
            svec!["2", "mango"],
        ],
    );
    wrk.create(
        "colors.csv",
        vec![
            svec!["idx", "color"],
            svec!["1", "blue"],
            svec!["2", "purple"],
        ],
    );

    for flag in ["--inner", "--left", "--right", "--full"] {
        // --drop-key=none
        let mut cmd = wrk.command("join");
        cmd.args(["--drop-key", "none"])
            .args(["idx", "fruits.csv", "colors.csv"])
            .arg(flag);

        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = vec![
            svec!["idx", "fruit", "idx", "color"],
            svec!["1", "apple", "1", "blue"],
            svec!["2", "mango", "2", "purple"],
        ];
        assert_eq!(got, expected);

        // --drop-key=left
        let mut cmd = wrk.command("join");
        cmd.args(["--drop-key", "left"])
            .args(["idx", "fruits.csv", "colors.csv"])
            .arg(flag);

        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = vec![
            svec!["fruit", "idx", "color"],
            svec!["apple", "1", "blue"],
            svec!["mango", "2", "purple"],
        ];
        assert_eq!(got, expected);

        // --drop-key=right
        let mut cmd = wrk.command("join");
        cmd.args(["--drop-key", "right"])
            .args(["idx", "fruits.csv", "colors.csv"])
            .arg(flag);

        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = vec![
            svec!["idx", "fruit", "color"],
            svec!["1", "apple", "blue"],
            svec!["2", "mango", "purple"],
        ];
        assert_eq!(got, expected);

        // --drop-key=both
        let mut cmd = wrk.command("join");
        cmd.args(["--drop-key", "both"])
            .args(["idx", "fruits.csv", "colors.csv"])
            .arg(flag);

        let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        let expected = vec![
            svec!["fruit", "color"],
            svec!["apple", "blue"],
            svec!["mango", "purple"],
        ];
        assert_eq!(got, expected);
    }
}

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

    let mut cmd = wrk.command("join");
    cmd.arg("-c")
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

    let mut cmd = wrk.command("join");
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

#[test]
fn fuzzy_join_drop_key() {
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

    // --drop-key=none
    let mut cmd = wrk.command("join");
    cmd.arg("-c").args(["--drop-key", "none"]).args([
        "person",
        "colors.csv",
        "pattern",
        "people.csv",
    ]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "pattern", "name"],
        svec!["john cannon", "blue", "john", "John"],
        svec!["lisa eckart", "purple", "lisa", "Lisa"],
        svec!["lil john", "red", "john", "John"],
    ];
    assert_eq!(got, expected);

    // --drop-key=left
    let mut cmd = wrk.command("join");
    cmd.arg("-c").args(["--drop-key", "left"]).args([
        "person",
        "colors.csv",
        "pattern",
        "people.csv",
    ]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["color", "pattern", "name"],
        svec!["blue", "john", "John"],
        svec!["purple", "lisa", "Lisa"],
        svec!["red", "john", "John"],
    ];
    assert_eq!(got, expected);

    // --drop-key=right
    let mut cmd = wrk.command("join");
    cmd.arg("-c").args(["--drop-key", "right"]).args([
        "person",
        "colors.csv",
        "pattern",
        "people.csv",
    ]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "name"],
        svec!["john cannon", "blue", "John"],
        svec!["lisa eckart", "purple", "Lisa"],
        svec!["lil john", "red", "John"],
    ];
    assert_eq!(got, expected);

    // --drop-key=both
    let mut cmd = wrk.command("join");
    cmd.arg("-c").args(["--drop-key", "right"]).args([
        "person",
        "colors.csv",
        "pattern",
        "people.csv",
    ]);
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["person", "color", "name"],
        svec!["john cannon", "blue", "John"],
        svec!["lisa eckart", "purple", "Lisa"],
        svec!["lil john", "red", "John"],
    ];
    assert_eq!(got, expected);
}
