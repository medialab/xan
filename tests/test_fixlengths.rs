use workdir::Workdir;
use CsvRecord;

fn trim_trailing_empty(it: &CsvRecord) -> Vec<String> {
    let mut cloned = it.clone().unwrap();
    while cloned.len() > 1 && cloned.last().unwrap().is_empty() {
        cloned.pop();
    }
    cloned
}

#[test]
fn fixlengths_all_maxlen_trims() {
    let rows = vec![
        svec!["h1", "h2"],
        svec!["abcdef", "ghijkl", "", ""],
        svec!["mnopqr", "stuvwx", "", ""],
    ];

    let wrk = Workdir::new("fixlengths_all_maxlen_trims").flexible(true);
    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("fixlengths");
    cmd.arg("in.csv");

    let got: Vec<CsvRecord> = wrk.read_stdout(&mut cmd);
    for r in got.iter() {
        assert_eq!(r.len(), 2)
    }
}

#[test]
fn fixlengths_all_maxlen_trims_at_least_1() {
    let rows = vec![svec![""], svec!["", ""], svec!["", "", ""]];

    let wrk = Workdir::new("fixlengths_all_maxlen_trims_at_least_1").flexible(true);
    wrk.create("in.csv", rows);

    let mut cmd = wrk.command("fixlengths");
    cmd.arg("in.csv");

    let got: Vec<CsvRecord> = wrk.read_stdout(&mut cmd);
    for r in got.iter() {
        assert_eq!(r.len(), 1)
    }
}
