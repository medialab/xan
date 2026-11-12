use std::borrow::ToOwned;
use std::process;

use crate::workdir::Workdir;

macro_rules! slice_tests {
    ($name:ident, $start:expr, $end:expr, $expected:expr) => {
        mod $name {
            use super::test_slice;

            #[test]
            fn headers() {
                let name = concat!(stringify!($name), "headers");
                test_slice(name, $start, $end, $expected, true, false);
            }

            #[test]
            fn no_headers() {
                let name = concat!(stringify!($name), "no_headers");
                test_slice(name, $start, $end, $expected, false, false);
            }

            #[test]
            fn headers_len() {
                let name = concat!(stringify!($name), "headers_len");
                test_slice(name, $start, $end, $expected, true, true);
            }

            #[test]
            fn no_headers_len() {
                let name = concat!(stringify!($name), "no_headers_len");
                test_slice(name, $start, $end, $expected, false, true);
            }
        }
    };
}

fn setup(name: &str, headers: bool) -> (Workdir, process::Command) {
    let wrk = Workdir::new(name);
    let mut data = vec![svec!["a"], svec!["b"], svec!["c"], svec!["d"], svec!["e"]];
    if headers {
        data.insert(0, svec!["header"]);
    }

    wrk.create("in.csv", data);

    let mut cmd = wrk.command("slice");
    cmd.arg("in.csv");

    (wrk, cmd)
}

fn test_slice(
    name: &str,
    start: Option<usize>,
    end: Option<usize>,
    expected: &[&str],
    headers: bool,
    as_len: bool,
) {
    let (wrk, mut cmd) = setup(name, headers);
    if let Some(start) = start {
        cmd.arg("--start").arg(start.to_string());
    }
    if let Some(end) = end {
        if as_len {
            let start = start.unwrap_or(0);
            cmd.arg("--len").arg((end - start).to_string());
        } else {
            cmd.arg("--end").arg(end.to_string());
        }
    }
    if !headers {
        cmd.arg("--no-headers");
    }

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let mut expected = expected
        .iter()
        .map(|&s| vec![s.to_owned()])
        .collect::<Vec<Vec<String>>>();
    if headers {
        expected.insert(0, svec!["header"]);
    }
    assert_eq!(got, expected);
}

fn test_index(name: &str, idx: usize, expected: &str, headers: bool) {
    let (wrk, mut cmd) = setup(name, headers);
    cmd.arg("--index").arg(idx.to_string());
    if !headers {
        cmd.arg("--no-headers");
    }

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let mut expected = vec![vec![expected.to_owned()]];
    if headers {
        expected.insert(0, svec!["header"]);
    }
    assert_eq!(got, expected);
}

slice_tests!(slice_simple, Some(0), Some(1), &["a"]);
slice_tests!(slice_simple_2, Some(1), Some(3), &["b", "c"]);
slice_tests!(slice_no_start, None, Some(1), &["a"]);
slice_tests!(slice_no_end, Some(3), None, &["d", "e"]);
slice_tests!(slice_all, None, None, &["a", "b", "c", "d", "e"]);

#[test]
fn slice_index() {
    test_index("slice_index", 1, "b", true);
}
#[test]
fn slice_index_no_headers() {
    test_index("slice_index_no_headers", 1, "b", false);
}

#[test]
fn slice_indices() {
    let wrk = Workdir::new("slice_indices");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec!["zero"],
            svec!["one"],
            svec!["two"],
            svec!["three"],
            svec!["four"],
            svec!["five"],
        ],
    );
    let mut cmd = wrk.command("slice");
    cmd.args(["-I", "1,5,4"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["one"], svec!["four"], svec!["five"]];
    assert_eq!(got, expected);
}

#[test]
fn slice_byte_offset() {
    let wrk = Workdir::new("slice_byte_offset");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec!["zero"],
            svec!["one"],
            svec!["two"],
            svec!["three"],
            svec!["four"],
            svec!["five"],
        ],
    );

    let mut cmd = wrk.command("slice");
    cmd.args(["-B", "10"]).args(["-l", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["two"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("slice");
    cmd.args(["-B", "10"])
        .args(["--end-byte", "18"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["two"], svec!["thr"]];
    assert_eq!(got, expected);
}

#[test]
fn slice_conditions() {
    let wrk = Workdir::new("slice_conditions");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec!["0"],
            svec!["1"],
            svec!["2"],
            svec!["3"],
            svec!["4"],
            svec!["5"],
        ],
    );

    let mut cmd = wrk.command("slice");
    cmd.args(["-S", "n > 3"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["4"], svec!["5"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("slice");
    cmd.args(["-S", "n > 3"]).args(["-l", "1"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["4"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("slice");
    cmd.args(["-E", "n >= 2"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["0"], svec!["1"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("slice");
    cmd.args(["-S", "n > 1"])
        .args(["-E", "n >= 3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["2"]];
    assert_eq!(got, expected);
}

#[test]
fn slice_last() {
    let wrk = Workdir::new("slice_last");
    wrk.create(
        "data.csv",
        vec![
            svec!["n"],
            svec!["zero"],
            svec!["one"],
            svec!["two"],
            svec!["three"],
            svec!["four"],
            svec!["five"],
        ],
    );
    let mut cmd = wrk.command("slice");
    cmd.args(["-L", "3"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![svec!["n"], svec!["three"], svec!["four"], svec!["five"]];
    assert_eq!(got, expected);

    let mut cmd = wrk.command("slice");
    cmd.args(["-L", "300"]).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["n"],
        svec!["zero"],
        svec!["one"],
        svec!["two"],
        svec!["three"],
        svec!["four"],
        svec!["five"],
    ];
    assert_eq!(got, expected);
}

fn assert_predicate_slice(name: &str, input: &str, args: &[&str], expected: &[&str]) {
    let wrk = Workdir::new(name);
    wrk.write("data.csv", input);
    let mut cmd = wrk.command("slice");
    cmd.args(args).arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected: Vec<Vec<String>> = expected.iter().map(|s| vec![(*s).to_owned()]).collect();
    assert_eq!(got, expected);
}

const PREDICATE_DATA: &str = "n\n0\n1\n2\n3\n4\n5\n";

#[test]
fn slice_predicate_indices_conditions() {
    assert_predicate_slice(
        "slice_predicate_indices_conditions",
        PREDICATE_DATA,
        &["-S", "n >= 2", "-E", "n >= 5", "-I", "0,2"],
        &["n", "2", "4"],
    );
}

#[test]
fn slice_predicate_indices_start_latches_and_preserves_order() {
    assert_predicate_slice(
        "slice_predicate_indices_start_latches_and_preserves_order",
        "n\n0\n2\n0\n3\n4\n",
        &["-S", "n >= 2", "-I", "3,1,1"],
        &["n", "0", "4"],
    );
}

#[test]
fn slice_predicate_indices_end_before_selected_row() {
    assert_predicate_slice(
        "slice_predicate_indices_end_before_selected_row",
        PREDICATE_DATA,
        &["-E", "n >= 3", "-I", "1,3,5"],
        &["n", "1"],
    );
}

#[test]
fn slice_predicate_range_relative_to_start() {
    assert_predicate_slice(
        "slice_predicate_range_relative_to_start",
        PREDICATE_DATA,
        &["-S", "n >= 2", "-s", "1", "-e", "3"],
        &["n", "3", "4"],
    );
}

#[test]
fn slice_predicate_row_index_before_start() {
    assert_predicate_slice(
        "slice_predicate_row_index_before_start",
        PREDICATE_DATA,
        &["-S", "row_index() >= 2", "-l", "2"],
        &["n", "2", "3"],
    );
}

#[test]
fn slice_predicate_row_index_after_numeric_skips() {
    assert_predicate_slice(
        "slice_predicate_row_index_after_numeric_skips",
        PREDICATE_DATA,
        &["-s", "2", "-E", "row_index() >= 4"],
        &["n", "2", "3"],
    );
}

#[test]
fn slice_predicate_row_index_after_sparse_skips() {
    assert_predicate_slice(
        "slice_predicate_row_index_after_sparse_skips",
        PREDICATE_DATA,
        &[
            "-S",
            "row_index() >= 1",
            "-E",
            "row_index() >= 4",
            "-I",
            "1,3",
        ],
        &["n", "2"],
    );
}

#[test]
fn slice_predicate_no_headers() {
    assert_predicate_slice(
        "slice_predicate_no_headers",
        "0\n1\n2\n3\n4\n5\n",
        &["-n", "-S", "col(0) >= 2", "-E", "col(0) >= 5", "-I", "0,2"],
        &["2", "4"],
    );
}

#[test]
fn slice_predicate_byte_offset() {
    assert_predicate_slice(
        "slice_predicate_byte_offset",
        PREDICATE_DATA,
        &[
            "-B",
            "6",
            "--end-byte",
            "12",
            "-S",
            "row_index() >= 1",
            "-I",
            "0,1",
        ],
        &["n", "3", "4"],
    );
}

#[test]
fn slice_predicate_zero_length_without_evaluating_conditions() {
    assert_predicate_slice(
        "slice_predicate_zero_length_without_evaluating_conditions",
        "n\ninvalid\n",
        &["-l", "0", "-S", "add(n, 1) > 0"],
        &["n"],
    );
}

#[test]
fn slice_predicate_empty_range_no_headers() {
    assert_predicate_slice(
        "slice_predicate_empty_range_no_headers",
        "invalid\n",
        &["-n", "-s", "2", "-e", "2", "-E", "add(col(0), 1) > 0"],
        &[],
    );
}

#[test]
fn slice_predicate_indices_stop_before_following_error() {
    assert_predicate_slice(
        "slice_predicate_indices_stop_before_following_error",
        "n\n0\ninvalid\n",
        &["-I", "0", "-E", "add(n, 1) > 2"],
        &["n", "0"],
    );
}

#[test]
fn slice_predicate_range_stop_before_following_error() {
    assert_predicate_slice(
        "slice_predicate_range_stop_before_following_error",
        "n\n0\ninvalid\n",
        &["-l", "1", "-E", "add(n, 1) > 2"],
        &["n", "0"],
    );
}

#[test]
fn slice_predicate_indices_keep_numeric_precedence() {
    assert_predicate_slice(
        "slice_predicate_indices_keep_numeric_precedence",
        PREDICATE_DATA,
        &["-I", "1,3", "-s", "5", "-e", "2", "-l", "0", "-i", "4"],
        &["n", "1", "3"],
    );
}

#[test]
fn slice_predicate_invalid_selection_and_runtime_errors() {
    let wrk = Workdir::new("slice_predicate_invalid_selection_and_runtime_errors");
    wrk.write("data.csv", "n\ninvalid\n");
    for args in [
        vec!["-I", "1,nope"],
        vec!["-s", "3", "-e", "2"],
        vec!["-i", "1", "-l", "1"],
        vec!["-S", "add(n, 1) > 0"],
        vec!["-E", "add(n, 1) > 0"],
    ] {
        let mut cmd = wrk.command("slice");
        cmd.args(args).arg("data.csv");
        wrk.assert_err(&mut cmd);
    }
}

#[test]
fn slice_predicate_quoted_records() {
    assert_predicate_slice(
        "slice_predicate_quoted_records",
        "n\n\"zero,0\"\n\"one\n1\"\n\"two,2\"\n",
        &["-S", "row_index() >= 1", "-I", "0,1"],
        &["n", "one\n1", "two,2"],
    );
}

#[test]
fn slice_predicate_stdin() {
    use std::io::Write;

    let wrk = Workdir::new("slice_predicate_stdin");
    let mut cmd = wrk.command("slice");
    cmd.args(["-S", "n >= 2", "-E", "n >= 5", "-I", "0,2"])
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(PREDICATE_DATA.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        "n\n2\n4\n"
    );
}

#[test]
fn slice_predicate_gzip() {
    use std::io::Write;

    let wrk = Workdir::new("slice_predicate_gzip");
    let file = std::fs::File::create(wrk.path("data.csv.gz")).unwrap();
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    encoder.write_all(PREDICATE_DATA.as_bytes()).unwrap();
    encoder.finish().unwrap();
    let mut cmd = wrk.command("slice");
    cmd.args(["-S", "n >= 2", "-I", "0,2"]).arg("data.csv.gz");
    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    assert_eq!(got, vec![svec!["n"], svec!["2"], svec!["4"]]);
}

#[test]
fn slice_predicate_raw_control() {
    assert_predicate_slice(
        "slice_predicate_raw_control",
        PREDICATE_DATA,
        &["--raw", "-B", "6", "--end-byte", "10"],
        &["n", "2", "3"],
    );
}
