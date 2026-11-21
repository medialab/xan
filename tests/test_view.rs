use crate::workdir::Workdir;

fn create_data(wrk: &Workdir) {
    wrk.create(
        "data.csv",
        vec![
            svec!["name", "score"],
            svec!["john", "1"],
            svec!["mary", "2"],
        ],
    );
}

#[test]
fn view_tiny_cols() {
    let wrk = Workdir::new("view_tiny_cols");
    create_data(&wrk);

    for cols in ["0", "1", "5", "10", "12"] {
        let mut cmd = wrk.command("view");
        cmd.arg("data.csv")
            .args(["--cols", cols])
            .env_remove("XAN_VIEW_ARGS");
        wrk.assert_success(&mut cmd);
    }
}

// NOTE: stdout is never a terminal when running tests
#[test]
fn view_pager_ignored_when_not_a_terminal() {
    let wrk = Workdir::new("view_pager_ignored_when_not_a_terminal");
    create_data(&wrk);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").env_remove("XAN_VIEW_ARGS");
    let expected: String = wrk.stdout(&mut cmd);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").arg("-p").env_remove("XAN_VIEW_ARGS");
    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got, expected);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").env("XAN_VIEW_ARGS", "-p");
    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
fn view_tee_with_pager_from_env() {
    let wrk = Workdir::new("view_tee_with_pager_from_env");
    create_data(&wrk);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").arg("--tee").env("XAN_VIEW_ARGS", "-p");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["name", "score"],
        svec!["john", "1"],
        svec!["mary", "2"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn view_no_pager() {
    let wrk = Workdir::new("view_no_pager");
    create_data(&wrk);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").env_remove("XAN_VIEW_ARGS");
    let expected: String = wrk.stdout(&mut cmd);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv")
        .arg("--no-pager")
        .env("XAN_VIEW_ARGS", "-p");
    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got, expected);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv")
        .args(["-p", "--no-pager"])
        .env_remove("XAN_VIEW_ARGS");
    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got, expected);
}

#[test]
fn view_ignore_env() {
    let wrk = Workdir::new("view_ignore_env");
    create_data(&wrk);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").env_remove("XAN_VIEW_ARGS");
    let expected: String = wrk.stdout(&mut cmd);

    let defaults = "-t borderless -I -M -S 2 --color always";

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv").env("XAN_VIEW_ARGS", defaults);
    let got: String = wrk.stdout(&mut cmd);
    assert_ne!(got, expected);

    let mut cmd = wrk.command("view");
    cmd.arg("data.csv")
        .arg("--ignore-env")
        .env("XAN_VIEW_ARGS", defaults);
    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got, expected);

    // Invalid defaults should not make the command fail when ignored
    let mut cmd = wrk.command("view");
    cmd.arg("data.csv")
        .arg("--ignore-env")
        .env("XAN_VIEW_ARGS", "--not-a-flag");
    let got: String = wrk.stdout(&mut cmd);
    assert_eq!(got, expected);

    for cols in ["0", "1", "5", "10", "12"] {
        let mut cmd = wrk.command("view");
        cmd.arg("data.csv")
            .args(["--cols", cols])
            .env_remove("XAN_VIEW_ARGS");
        wrk.assert_success(&mut cmd);
    }
}

#[test]
fn view_plain_aligned() {
    let wrk = Workdir::new("view_plain_aligned");
    let filename = "Nomino (2020) S1.E1.1080p.mkv";
    wrk.create(
        "data.csv",
        vec![svec!["key", "value"], svec![filename, "0101.mkv"]],
    );

    let mut cmd = wrk.command("view");
    cmd.args(["--plain", "--cols", "80", "data.csv"])
        .env_remove("XAN_VIEW_ARGS");
    let stdout = String::from_utf8(wrk.output(&mut cmd).stdout).unwrap();

    assert!(!stdout.starts_with('\n'));
    assert!(!stdout.ends_with("\n\n"));
    assert!(!stdout.contains('\x1b'));
    let lines: Vec<_> = stdout.lines().collect();
    assert!(lines.iter().all(|line| !line.trim().is_empty()));
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("key") && lines[0].contains("value"));
    assert!(lines[1].contains(filename) && lines[1].contains("0101.mkv"));
    assert_eq!(
        lines[0].find("value").unwrap(),
        lines[1].find("0101.mkv").unwrap()
    );
}
