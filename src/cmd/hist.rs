use std::collections::BTreeMap;

use colored;
use colored::Colorize;
use csv;
use indexmap::{map::Entry, IndexMap};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

const SMALL_BAR_CHARS: [&str; 2] = ["╸", "━"]; // "╾"
const MEDIUM_BAR_CHARS: [&str; 1] = ["■"];
const LARGE_BAR_CHARS: [&str; 8] = ["▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

// TODO: log scales etc.

static USAGE: &str = "
Print a horizontal histogram for the given CSV file with each line
representing a bar in the resulting graph.

This command is very useful when used in conjunction with the `frequency` or `bins`
command.

Usage:
    xan hist [options] [<input>]
    xan hist --help

hist options:
    --name <name>            Name of the represented field when no field column is
                             present. [default: unknown].
    -f, --field <name>       Name of the field column. I.e. the one containing
                             the represented value (remember this command can
                             print several histograms). [default: field].
    -l, --label <name>       Name of the label column. I.e. the one containing the
                             label for a single bar of the histogram. [default: value].
    -v, --value <name>       Name of the count column. I.e. the one containing the value
                             for each bar. [default: count].
    -B, --bar-size <size>    Size of the bar characters between \"small\", \"medium\"
                             and \"large\". [default: medium].
    --cols <num>             Width of the graph in terminal columns, i.e. characters.
                             Defaults to using all your terminal's width or 80 if
                             terminal's size cannot be found (i.e. when piping to file).
                             Can also be given as a ratio of the terminal's width e.g. \"0.5\".
    -R, --rainbow            Alternating colors for the bars.
    -m, --domain-max <type>  If \"max\" max bar length will be scaled to the
                             max bar value. If \"sum\", max bar length will be scaled to
                             the sum of bar values (i.e. sum of bar lengths will be 100%).
                             Can also be an absolute numerical value, to clamp the bars
                             or make sure different histograms are represented using the
                             same scale.
                             [default: max]
    -c, --category <col>     Name of the categorical column that will be used to
                             assign distinct colors per category.
                             Incompatible with -R, --rainbow.
    -C, --force-colors       Force colors even if output is not supposed to be able to
                             handle them.
    -P, --hide-percent       Don't show percentages.
    -u, --unit <unit>        Value unit.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_field: SelectColumns,
    flag_label: SelectColumns,
    flag_value: SelectColumns,
    flag_cols: Option<String>,
    flag_force_colors: bool,
    flag_domain_max: String,
    flag_rainbow: bool,
    flag_name: String,
    flag_hide_percent: bool,
    flag_unit: Option<String>,
    flag_category: Option<SelectColumns>,
    flag_bar_size: String,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if args.flag_force_colors {
        colored::control::set_override(true);
    }

    if args.flag_category.is_some() && args.flag_rainbow {
        Err("-c, --category cannot work with -R, --rainbow")?;
    }

    let mut rdr = conf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let label_pos = args
        .flag_label
        .single_selection(&headers, !args.flag_no_headers)?;
    let value_pos = args
        .flag_value
        .single_selection(&headers, !args.flag_no_headers)?;
    let field_pos_option = args
        .flag_field
        .single_selection(&headers, !args.flag_no_headers)
        .ok();

    let mut histograms = Histograms::new();

    let mut record = csv::StringRecord::new();

    let category_column_index = args
        .flag_category
        .as_ref()
        .map(|name| name.single_selection(&headers, !args.flag_no_headers))
        .transpose()?;

    let mut category_colors: IndexMap<String, usize> = IndexMap::new();
    let mut categories_overflow: Vec<String> = Vec::new();

    while rdr.read_record(&mut record)? {
        let field = match field_pos_option {
            Some(field_pos) => record[field_pos].to_string(),
            None => args.flag_name.clone(),
        };
        let label = record[label_pos].to_string();
        let value = record[value_pos]
            .parse::<f64>()
            .map_err(|_| "could not parse value")?;

        if let Some(category_col) = category_column_index {
            let category = record[category_col].to_string();

            if !category.is_empty() {
                let next_index = category_colors.len();

                match category_colors.entry(category.clone()) {
                    Entry::Vacant(entry) => {
                        if next_index < 7 {
                            entry.insert(next_index);
                            histograms.add(field, label, value, Some(next_index));
                        } else {
                            // NOTE: beware O(n) lol
                            if !categories_overflow.contains(&category) {
                                categories_overflow.push(category);
                            }

                            histograms.add(field, label, value, None);
                        }
                    }
                    Entry::Occupied(entry) => {
                        histograms.add(field, label, value, Some(*entry.get()));
                    }
                };
            } else {
                histograms.add(field, label, value, None);
            }
        } else {
            histograms.add(field, label, value, None);
        }
    }

    let mut cols = util::acquire_term_cols(&None);

    if let Some(spec) = &args.flag_cols {
        if spec.contains('.') {
            let ratio = spec.parse::<f64>().map_err(|_| "--cols is invalid! ")?;

            cols = (cols as f64 * ratio).trunc().abs() as usize;
        } else {
            cols = spec.parse::<usize>().map_err(|_| "--cols is invalid! ")?;
        }
    }

    let unit = args.flag_unit.as_deref().unwrap_or("");

    for histogram in histograms.iter_mut() {
        if histogram.len() == 0 {
            continue;
        }

        if args.flag_category.is_some() {
            histogram.collapse();
        }

        let sum = histogram.sum();

        let domain_max = match args.flag_domain_max.as_str() {
            "max" => histogram.max().unwrap(),
            "sum" => sum,
            d => match d.parse::<f64>() {
                Ok(f) => f,
                _ => return fail!("unknown --domain-max. Should be one of \"sum\", \"max\"."),
            },
        };

        println!(
            "\nHistogram for {} (bars: {}, sum: {}{}, max: {}{}):\n",
            histogram.field.green(),
            util::format_number(histogram.len()).cyan(),
            util::format_number(sum).cyan(),
            unit.cyan(),
            util::format_number(histogram.max().unwrap()).cyan(),
            unit.cyan(),
        );

        let pct_cols: usize = if args.flag_hide_percent { 0 } else { 8 };

        if cols < 30 {
            return fail!("You did not provide enough --cols to print anything!");
        }

        let value_max_width_unit_addendum = match &args.flag_unit {
            None => 0,
            Some(unit) => unit.width(),
        };

        let remaining_cols = cols - pct_cols;
        let count_cols = histogram.value_max_width().unwrap();
        let label_cols = usize::min(
            (remaining_cols as f64 * 0.4).floor() as usize,
            histogram.label_max_width().unwrap(),
        );
        let bar_cols =
            remaining_cols - (count_cols + value_max_width_unit_addendum) - label_cols - 4;

        let mut odd = false;

        let chars: &[&str] = match args.flag_bar_size.as_str() {
            "small" => &SMALL_BAR_CHARS,
            "medium" => &MEDIUM_BAR_CHARS,
            "large" => &LARGE_BAR_CHARS,
            _ => {
                return fail!(
                    "unknown -B, --bar-size. Should be one of \"small\", \"medium\", \"large\"."
                )
            }
        };

        for (i, bar) in histogram.bars().enumerate() {
            let bar_width =
                from_domain_to_range(bar.value, (0.0, domain_max), (0.0, bar_cols as f64));

            let mut bar_as_chars =
                util::unicode_aware_rpad(&create_bar(chars, bar_width), bar_cols, " ").clear();

            // Categorical colors
            if category_column_index.is_some() {
                if let Some(color_index) = bar.category {
                    bar_as_chars = util::colorize(
                        &util::colorizer_by_rainbow(color_index, &bar_as_chars),
                        &bar_as_chars,
                    );
                } else {
                    bar_as_chars = bar_as_chars.dimmed();
                }
            }
            // Rainbow
            else if args.flag_rainbow {
                bar_as_chars =
                    util::colorize(&util::colorizer_by_rainbow(i, &bar_as_chars), &bar_as_chars);
            }
            // Stripes
            else {
                if odd {
                    bar_as_chars = match args.flag_bar_size.as_str() {
                        "large" => bar_as_chars.dimmed(),
                        _ => bar_as_chars.bright_black(),
                    };
                }

                odd = !odd;
            }

            let label = util::unicode_aware_rpad_with_ellipsis(&bar.label, label_cols, " ");
            let label = match bar.label.as_str() {
                "<rest>" | "<null>" | "<NaN>" | "<empty>" => label.dimmed(),
                _ => label.normal(),
            };

            println!(
                "{} |{}{}{}|{}|",
                label,
                util::unicode_aware_lpad_with_ellipsis(
                    &util::format_number(bar.value),
                    count_cols,
                    " "
                )
                .cyan(),
                unit.cyan(),
                if args.flag_hide_percent {
                    "".to_string().normal()
                } else {
                    format!(" {:>6.2}%", bar.value / sum * 100.0).purple()
                },
                bar_as_chars
            );
        }

        // Printing the categorical legend
        if let Some(category_col) = category_column_index {
            let category_column_name =
                std::str::from_utf8(&headers[category_col]).expect("could not decode header");

            println!("\nColors by {}:", category_column_name.green());

            for (category, color_index) in &category_colors {
                println!(
                    " {}  {}",
                    util::colorize(&util::colorizer_by_rainbow(*color_index, "■"), "■"),
                    util::colorize(
                        &util::colorizer_by_rainbow(*color_index, category),
                        category
                    ),
                );
            }

            if !categories_overflow.is_empty() {
                let others = categories_overflow
                    .iter()
                    .map(|label| label.dimmed().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                println!(" {}  {}", "■".dimmed(), &others);
            }
        }
    }

    println!();

    Ok(())
}

fn from_domain_to_range(x: f64, domain: (f64, f64), range: (f64, f64)) -> f64 {
    let domain_width = (domain.1 - domain.0).abs();
    let pct = (x - domain.0) / domain_width;

    let range_widht = (range.1 - range.0).abs();

    pct * range_widht + range.0
}

fn create_bar(chars: &[&str], width: f64) -> String {
    let f = width.fract();

    if f < f64::EPSILON {
        chars[chars.len() - 1].repeat(width as usize)
    } else {
        let mut string = chars[chars.len() - 1].repeat(width.floor() as usize);

        let padding = chars[((chars.len() - 1) as f64 * f).floor() as usize];
        string.push_str(padding);

        string
    }
}

#[derive(Debug)]
struct Bar {
    label: String,
    value: f64,
    category: Option<usize>,
}

#[derive(Debug)]
struct Histogram {
    field: String,
    bars: Vec<Bar>,
}

impl Histogram {
    fn len(&self) -> usize {
        self.bars.len()
    }

    fn max(&self) -> Option<f64> {
        let mut max: Option<f64> = None;

        for bar in self.bars.iter() {
            let n = bar.value;

            max = match max {
                None => Some(n),
                Some(m) => Some(f64::max(n, m)),
            };
        }

        max
    }

    fn sum(&self) -> f64 {
        self.bars.iter().map(|bar| bar.value).sum()
    }

    fn bars(&self) -> impl Iterator<Item = &Bar> {
        self.bars.iter()
    }

    fn label_max_width(&self) -> Option<usize> {
        self.bars.iter().map(|bar| bar.label.width()).max()
    }

    fn value_max_width(&self) -> Option<usize> {
        self.bars
            .iter()
            .map(|bar| util::format_number(bar.value).len())
            .max()
    }

    fn collapse(&mut self) {
        let mut last_label_opt: Option<&str> = None;

        for bar in self.bars.iter_mut() {
            match last_label_opt {
                None => last_label_opt = Some(&bar.label),
                Some(last_label) => {
                    if last_label == bar.label {
                        bar.label = "".to_string();
                    } else {
                        last_label_opt = Some(&bar.label);
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
struct Histograms {
    histograms: BTreeMap<String, Histogram>,
}

impl Histograms {
    pub fn new() -> Self {
        Histograms {
            histograms: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, field: String, label: String, value: f64, category: Option<usize>) {
        self.histograms
            .entry(field.clone())
            .and_modify(|h| {
                h.bars.push(Bar {
                    label: label.clone(),
                    value,
                    category,
                })
            })
            .or_insert_with(|| Histogram {
                field,
                bars: vec![Bar {
                    label,
                    value,
                    category,
                }],
            });
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Histogram> {
        self.histograms.values_mut()
    }
}
