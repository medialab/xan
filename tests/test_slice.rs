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
        cmd.arg("--start").arg(&start.to_string());
    }
    if let Some(end) = end {
        if as_len {
            let start = start.unwrap_or(0);
            cmd.arg("--len").arg(&(end - start).to_string());
        } else {
            cmd.arg("--end").arg(&end.to_string());
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
    cmd.arg("--index").arg(&idx.to_string());
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
    cmd.args(["-i", "1,5,4"]).arg("data.csv");

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
}
