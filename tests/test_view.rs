use crate::workdir::Workdir;

#[test]
fn view_tiny_cols() {
    let wrk = Workdir::new("view_tiny_cols");
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "score"],
            svec!["john", "1"],
            svec!["mary", "2"],
        ],
    );

    for cols in ["0", "1", "5", "10", "12"] {
        let mut cmd = wrk.command("view");
        cmd.arg("data.csv")
            .args(["--cols", cols])
            .env_remove("XAN_VIEW_ARGS");
        wrk.assert_success(&mut cmd);
    }
}
