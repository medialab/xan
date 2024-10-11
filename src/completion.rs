use std::fs::File;

use glob::glob;

static COMMANDS: [&str; 52] = [
    "agg",
    "behead",
    "bins",
    "blank",
    "cat",
    "cluster",
    "count",
    "dedup",
    "enum",
    "eval",
    "explode",
    "foreach",
    "filter",
    "fixlengths",
    "flatmap",
    "flatten",
    "fmt",
    "frequency",
    "from",
    "glob",
    "groupby",
    "headers",
    "help",
    "hist",
    "implode",
    "index",
    "input",
    "join",
    "map",
    "merge",
    "parallel",
    "partition",
    "plot",
    "progress",
    "range",
    "rename",
    "reverse",
    "sample",
    "search",
    "select",
    "shuffle",
    "slice",
    "sort",
    "split",
    "stats",
    "tokenize",
    "top",
    "transform",
    "transpose",
    "union-find",
    "view",
    "vocab",
];

static VOCAB_SUBCOMMANDS: [&str; 5] = ["corpus", "doc", "doc-token", "token", "cooc"];

// bash: complete -C "target/debug/__xan" -o default xan
// zsh:  complete -F "target/debug/__xan" -o default xan
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut to_complete = args[2].as_str();
    let word_before = &args[3];

    if to_complete == "--" {
        to_complete = "";
    }

    // Completing commands
    if word_before == "xan" {
        if !to_complete.starts_with('-') {
            for command in COMMANDS {
                if command.starts_with(to_complete) {
                    println!("{}", command);
                }
            }
        }
    } else if word_before == "vocab" && !to_complete.starts_with('-') {
        for command in VOCAB_SUBCOMMANDS {
            if command.starts_with(to_complete) {
                println!("{}", command);
            }
        }
    }

    // Completing column selectors
    // TODO: escape the selector name
    if word_before == "-s"
        || word_before == "--select"
        || word_before == "-g"
        || word_before == "--groupby"
        || word_before == "select"
        || word_before == "transform"
        || word_before == "explode"
        || word_before == "implode"
        || word_before == "groupby"
        || word_before == "partition"
        || word_before == "plot"
    {
        let mut all_headers = Vec::<String>::new();

        for entry in glob("**/*.csv")
            .unwrap()
            .chain(glob("**/*.csv.gz").unwrap())
            .chain(glob("**/*.tsv").unwrap())
            .chain(glob("**/*.tsv.gz").unwrap())
            .take(15)
        {
            let path = entry.unwrap();
            let file = match File::open(path) {
                Ok(f) => f,
                _ => continue,
            };
            let mut reader = csv::Reader::from_reader(file);

            if let Ok(headers) = reader.headers() {
                for name in headers {
                    let name = name.to_string();

                    if name.starts_with(to_complete) && !all_headers.contains(&name) {
                        all_headers.push(name);
                    }
                }
            }
        }

        for name in all_headers {
            println!("{}", name);
        }
    }
}
