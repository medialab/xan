use crate::workdir::Workdir;

#[test]
fn matrix_count() {
    let wrk = Workdir::new("matrix_count");
    wrk.create(
        "data.csv",
        vec![
            svec!["true", "pred", "weight"],
            svec!["true", "true", "1"],
            svec!["true", "true", "1"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["false", "true", "0.5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("count").arg("true").arg("pred").arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "true", "false"],
        ["true", "2", "1"],
        ["false", "4", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_count_weight() {
    let wrk = Workdir::new("matrix_count_weight");
    wrk.create(
        "data.csv",
        vec![
            svec!["true", "pred", "weight"],
            svec!["true", "true", "1"],
            svec!["true", "true", "1"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["true", "false", "0.5"],
            svec!["false", "true", "0.5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("count")
        .arg("true")
        .arg("pred")
        .arg("data.csv")
        .args(["--weight", "weight"]);

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "true", "false"],
        ["true", "2", "0.5"],
        ["false", "2", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_count_rectangular() {
    let wrk = Workdir::new("matrix_count_rectangular");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b", "weight"],
            svec!["one", "deux", "1"],
            svec!["one", "trois", "5"],
            svec!["two", "un", "2"],
            svec!["one", "deux", "7"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("count")
        .arg("a")
        .arg("b")
        .args(["-w", "weight"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "one", "two"],
        ["deux", "8", ""],
        ["trois", "5", ""],
        ["un", "", "2"],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_adj() {
    let wrk = Workdir::new("matrix_adj");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b", "weight"],
            svec!["one", "deux", "1"],
            svec!["one", "trois", "5"],
            svec!["two", "un", "2"],
            svec!["one", "deux", "7"],
            svec!["one", "one", "4"],
            svec!["two", "two", "1"],
            svec!["two", "one", "5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("adj")
        .arg("a")
        .arg("b")
        .args(["-w", "weight"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "one", "deux", "trois", "two", "un"],
        ["one", "4", "", "", "5", ""],
        ["deux", "8", "", "", "", ""],
        ["trois", "5", "", "", "", ""],
        ["two", "", "", "", "1", ""],
        ["un", "", "", "", "2", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_adj_undirected() {
    let wrk = Workdir::new("matrix_adj_undirected");
    wrk.create(
        "data.csv",
        vec![
            svec!["a", "b", "weight"],
            svec!["one", "deux", "1"],
            svec!["one", "trois", "5"],
            svec!["two", "un", "2"],
            svec!["one", "deux", "7"],
            svec!["one", "one", "4"],
            svec!["two", "two", "1"],
            svec!["two", "one", "5"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("adj")
        .arg("a")
        .arg("b")
        .args(["-w", "weight"])
        .arg("-U")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "deux", "one", "trois", "two", "un"],
        ["deux", "", "8", "", "", ""],
        ["one", "8", "4", "5", "5", ""],
        ["trois", "", "5", "", "", ""],
        ["two", "", "5", "", "1", "2"],
        ["un", "", "", "", "2", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_grid() {
    let wrk = Workdir::new("matrix_grid");
    wrk.create(
        "data.csv",
        vec![
            svec!["x", "y"],
            svec!["117058.67","-277609.06"],
            svec!["24227.016","99455.19"],
            svec!["-75575.8","15522.383"],
            svec!["-75575.8","15522.383"],
            svec!["43559.855","28944.082"],
            svec!["12391.966","-279303.53"],
            svec!["-171043.47", "63589.05"],
            svec!["-249256.39", "-3438.3096"],
            svec!["-277263.53", "-38509.46"],
            svec!["33029.5", "156171.39"],
            svec!["43516.703", "-84912.56"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("grid")
        .arg("x")
        .arg("y")
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "[-277,263;-237,831[", "[-237,831;-198,399[", "[-198,399;-158,966[", "[-158,966;-119,534[", "[-119,534;-80,102[", "[-80,102;-40,670[", "[-40,670;-1,237.9[", "[-1,237.9;38,194[", "[38,194;77,626[", "[77,626;117,058]"],
        ["[-279,303;-235,756[", "", "", "", "", "", "1", "1", "", "", ""],
        ["[-235,756;-192,208[", "", "", "", "", "", "", "", "", "", ""],
        ["[-192,208;-148,661[", "", "", "", "", "", "", "", "1", "", ""],
        ["[-148,661;-105,113[", "", "", "", "", "", "", "", "", "", ""],
        ["[-105,113;-61,566[", "", "", "", "", "", "", "", "", "", ""],
        ["[-61,566;-18,018[", "", "", "", "", "", "", "2", "", "", ""],
        ["[-18,018;25,528[", "", "", "", "", "", "", "", "", "", ""],
        ["[25,528;69,076[", "1", "", "", "", "", "", "", "", "1", "1"],
        ["[69,076;112,623[", "", "", "", "", "1", "", "", "1", "", ""],
        ["[112,623;156,171]", "1", "", "", "", "", "", "", "", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_grid_weight() {
    let wrk = Workdir::new("matrix_grid");
    wrk.create(
        "data.csv",
        vec![
            svec!["x", "y", "weight"],
            svec!["117058.67", "-277609.06", "6.178381874268749"],
            svec!["24227.016", "99455.19", "8.256781997701086"],
            svec!["-75575.8", "15522.383", "6.480093564961779"],
            svec!["43559.855", "28944.082", "3.4911930158370597"],
            svec!["12391.966", "-279303.53", "2.5618406796454716"],
            svec!["-171043.47", "63589.05", "8.834227893525213"],
            svec!["-249256.39", "-3438.3096", "7.486790584554081"],
            svec!["-277263.53", "-38509.46", "10.616205472750881"],
            svec!["33029.5", "156171.39", "7.14535053019698"],
            svec!["43516.703", "-84912.56", "7.199900320236468"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("grid")
        .arg("x")
        .arg("y")
        .args(["-w", "weight"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        ["", "[-277,263;-237,831[", "[-237,831;-198,399[", "[-198,399;-158,966[", "[-158,966;-119,534[", "[-119,534;-80,102[", "[-80,102;-40,670[", "[-40,670;-1,237.9[", "[-1,237.9;38,194[", "[38,194;77,626[", "[77,626;117,058]"],
        ["[-279,303;-235,756[", "", "", "", "", "", "10.616205472750881", "7.486790584554081", "", "", ""],
        ["[-235,756;-192,208[", "", "", "", "", "", "", "", "", "", ""],
        ["[-192,208;-148,661[", "", "", "", "", "", "", "", "8.834227893525213", "", ""],
        ["[-148,661;-105,113[", "", "", "", "", "", "", "", "", "", ""],
        ["[-105,113;-61,566[", "", "", "", "", "", "", "", "", "", ""],
        ["[-61,566;-18,018[", "", "", "", "", "", "", "6.480093564961779", "", "", ""],
        ["[-18,018;25,528[", "", "", "", "", "", "", "", "", "", ""],
        ["[25,528;69,076[", "2.5618406796454716", "", "", "", "", "", "", "", "8.256781997701086", "7.14535053019698"],
        ["[69,076;112,623[", "", "", "", "", "7.199900320236468", "", "", "3.4911930158370597", "", ""],
        ["[112,623;156,171]", "6.178381874268749", "", "", "", "", "", "", "", "", ""],
    ];
    assert_eq!(got, expected);
}

#[test]
fn matrix_grid_weight_bins() {
    let wrk = Workdir::new("matrix_grid");
    wrk.create(
        "data.csv",
        vec![
            svec!["x", "y", "weight"],
            svec!["117058.67", "-277609.06", "6.178381874268749"],
            svec!["24227.016", "99455.19", "8.256781997701086"],
            svec!["-75575.8", "15522.383", "6.480093564961779"],
            svec!["43559.855", "28944.082", "3.4911930158370597"],
            svec!["12391.966", "-279303.53", "2.5618406796454716"],
            svec!["-171043.47", "63589.05", "8.834227893525213"],
            svec!["-249256.39", "-3438.3096", "7.486790584554081"],
            svec!["-277263.53", "-38509.46", "10.616205472750881"],
            svec!["33029.5", "156171.39", "7.14535053019698"],
            svec!["43516.703", "-84912.56", "7.199900320236468"],
        ],
    );

    let mut cmd = wrk.command("matrix");
    cmd.arg("grid")
        .arg("x")
        .arg("y")
        .args(["-w", "weight"])
        .args(["--bins-x", "7"])
        .args(["--bins-y", "3"])
        .arg("data.csv");

    let got: Vec<Vec<String>> = wrk.read_stdout(&mut cmd);
    let expected = vec![
        svec!["", "[-277,263;-220,931[", "[-220,931;-164,600[", "[-164,600;-108,268[", "[-108,268;-51,936[", "[-51,936;4,395.1[", "[4,395.1;60,726[", "[60,726;117,058]"],
        svec!["[-279,303;-134,145[", "", "18.102996057304964", "", "", "", "8.834227893525213", ""],
        svec!["[-134,145;11,013[", "", "", "", "", "6.480093564961779", "", ""],
        svec!["[11,013;156,171[", "", "2.5618406796454716", "7.199900320236468", "18.893325543735124", "6.178381874268749", "", ""],
    ];
    assert_eq!(got, expected);
}