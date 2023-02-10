#[cfg(feature = "fullsearch")]
mod test {
    use std::process;

    use workdir::Workdir;

    fn setup(name: &str, keywords: &str) -> (Workdir, process::Command) {
        let rows = vec![
            svec!["english", "french"],
            svec!["eat", "mange"],
            svec!["eat", "mangeons"],
            svec!["the cats eat the mouse", "les chats mangent la souri"],
            svec!["the cat eats the mouse", "le chat mange la souri"],
            svec!["the cat is eating the mouse", "le chat est en train de manger la souri"],
            svec!["dog", "chien"],
            svec!["", "manger se dit eat en anglais"],
            svec!["les chats mangent la souri", "the cats eat the mouse"],
        ];

        let wrk = Workdir::new(name);
        wrk.create("in.csv", rows);

        let mut cmd = wrk.command("fullsearch");
        cmd.arg(keywords).arg("in.csv");

        (wrk, cmd)
    }

    #[test]
    fn fullsearch_basic() {
        let (wrk, mut cmd) = setup("fullsearch_limit", "eat");

        let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        got.sort();
        let expected = vec![
            svec!["", "manger se dit eat en anglais"],
            svec!["eat", "mange"],
            svec!["eat", "mangeons"],
            svec!["english", "french"],
            svec!["les chats mangent la souri", "the cats eat the mouse"],
            svec!["the cat eats the mouse", "le chat mange la souri"],
            svec!["the cat is eating the mouse", "le chat est en train de manger la souri"],
            svec!["the cats eat the mouse", "les chats mangent la souri"]
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn fullsearch_limit() {
        let (wrk, mut cmd) = setup("fullsearch_limit", "eat");
        cmd.args(&["--limit", "2"]);

        let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        got.sort();
        let expected = vec![
            svec!["", "manger se dit eat en anglais"],
            svec!["english", "french"],
            svec!["les chats mangent la souri", "the cats eat the mouse"]
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn fullsearch_select() {
        let (wrk, mut cmd) = setup("frequency_select", "eat");
        cmd.args(&["--limit", "0"]).args(&["--select", "french"]);

        let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        got.sort();
        let expected = vec![
            svec!["", "manger se dit eat en anglais"],
            svec!["english", "french"],
            svec!["les chats mangent la souri", "the cats eat the mouse"]
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn fullsearch_lang() {
        let (wrk, mut cmd) = setup("frequency_select", "mange");
        cmd.args(&["--limit", "0"]).args(&["--lang", "french"]);

        let mut got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
        got.sort();
        let expected = vec![
            svec!["", "manger se dit eat en anglais"],
            svec!["eat", "mange"],
            svec!["english", "french"],
            svec!["the cat eats the mouse", "le chat mange la souri"],
            svec!["the cat is eating the mouse", "le chat est en train de manger la souri"]
        ];
        assert_eq!(got, expected);
    }
}