use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io;

use csv;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::config::{Config, Delimiter};
use crate::index::Indexed;
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliError;
use crate::CliResult;

type GroupKey = Vec<Vec<u8>>;

static USAGE: &str = "
Randomly samples CSV data uniformly using memory proportional to the size of
the sample.

When an index is present, this command will use random indexing if the sample
size is less than 10% of the total number of records. This allows for efficient
sampling such that the entire CSV file is not parsed.

This command is intended to provide a means to sample from a CSV data set that
is too big to fit into memory (for example, for use with commands like 'xan freq'
or 'xan stats'). It will however visit every CSV record exactly
once, which is necessary to provide a uniform random sample. If you wish to
limit the number of records visited, use the 'xan slice' command to pipe into
'xan sample'.

The command can also extract a biased sample based on a numeric column representing
row weights, using the --weight flag.

Usage:
    xan sample [options] <sample-size> [<input>]
    xan sample --help

sample options:
    --seed <number>        RNG seed.
    -w, --weight <column>  Column containing weights to bias the sample.
    -g, --groupby <cols>   Return a sample per group.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will be consider as part of
                           the population to sample from. (When not set, the
                           first row is the header row and will always appear
                           in the output.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_sample_size: u64,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_seed: Option<usize>,
    flag_weight: Option<SelectColumns>,
    flag_groupby: Option<SelectColumns>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if let Some(weight_column_selection) = args.flag_weight.clone() {
        rconfig = rconfig.select(weight_column_selection);
    }

    let sample_size = args.arg_sample_size;

    let mut wtr = Config::new(&args.flag_output).writer()?;
    let sampled = match rconfig.indexed()? {
        Some(mut idx) => {
            // TODO: crash if -g

            if args.flag_weight.is_some() {
                let mut rdr = rconfig.reader()?;
                rconfig.write_headers(&mut rdr, &mut wtr)?;

                let weight_column_index = rconfig.single_selection(rdr.byte_headers()?)?;

                sample_weighted_reservoir(
                    &mut rdr,
                    sample_size,
                    args.flag_seed,
                    weight_column_index,
                )?
            } else if do_random_access(sample_size, idx.count()) {
                rconfig.write_headers(&mut *idx, &mut wtr)?;
                sample_random_access(&mut idx, sample_size)?
            } else {
                let mut rdr = rconfig.reader()?;
                rconfig.write_headers(&mut rdr, &mut wtr)?;
                sample_reservoir(&mut rdr, sample_size, args.flag_seed)?
            }
        }
        _ => {
            let mut rdr = rconfig.reader()?;
            rconfig.write_headers(&mut rdr, &mut wtr)?;
            let byte_headers = rdr.byte_headers()?;

            let group_sel_opt = args
                .flag_groupby
                .map(|s| Config::new(&None).select(s).selection(byte_headers))
                .transpose()?;

            if args.flag_weight.is_some() {
                let weight_column_index = rconfig.single_selection(byte_headers)?;

                // TODO: deal with -g
                sample_weighted_reservoir(
                    &mut rdr,
                    sample_size,
                    args.flag_seed,
                    weight_column_index,
                )?
            } else {
                if let Some(group_sel) = group_sel_opt {
                    sample_reservoir_grouped(&mut rdr, sample_size, args.flag_seed, group_sel)?
                } else {
                    sample_reservoir(&mut rdr, sample_size, args.flag_seed)?
                }
            }
        }
    };
    for row in sampled.into_iter() {
        wtr.write_byte_record(&row)?;
    }
    Ok(wtr.flush()?)
}

fn sample_random_access<R, I>(
    idx: &mut Indexed<R, I>,
    sample_size: u64,
) -> CliResult<Vec<csv::ByteRecord>>
where
    R: io::Read + io::Seek,
    I: io::Read + io::Seek,
{
    let mut all_indices = (0..idx.count()).collect::<Vec<_>>();
    let mut rng = rand::thread_rng();
    all_indices.shuffle(&mut rng);

    let mut sampled = Vec::with_capacity(sample_size as usize);
    for i in all_indices.into_iter().take(sample_size as usize) {
        idx.seek(i)?;
        sampled.push(idx.byte_records().next().unwrap()?);
    }
    Ok(sampled)
}

fn sample_reservoir<R: io::Read>(
    rdr: &mut csv::Reader<R>,
    sample_size: u64,
    seed: Option<usize>,
) -> CliResult<Vec<csv::ByteRecord>> {
    // The following algorithm has been adapted from:
    // https://en.wikipedia.org/wiki/Reservoir_sampling
    let mut reservoir = Vec::with_capacity(sample_size as usize);
    let mut records = rdr.byte_records().enumerate();
    for (_, row) in records.by_ref().take(sample_size as usize) {
        reservoir.push(row?);
    }

    // Seeding rng
    let mut rng = util::acquire_rng(seed);

    // Now do the sampling.
    for (i, row) in records {
        let random = rng.gen_range(0..i + 1);
        if random < sample_size as usize {
            reservoir[random] = row?;
        }
    }
    Ok(reservoir)
}

fn sample_reservoir_grouped<R: io::Read>(
    rdr: &mut csv::Reader<R>,
    sample_size: u64,
    seed: Option<usize>,
    group_sel: Selection,
) -> CliResult<Vec<csv::ByteRecord>> {
    Ok(vec![])
}

#[derive(PartialEq)]
struct WeightedRow(f64, csv::ByteRecord);

impl WeightedRow {
    fn row(self) -> csv::ByteRecord {
        self.1
    }
}

impl Eq for WeightedRow {}

impl PartialOrd for WeightedRow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeightedRow {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap().reverse()
    }
}

fn sample_weighted_reservoir<R: io::Read>(
    rdr: &mut csv::Reader<R>,
    sample_size: u64,
    seed: Option<usize>,
    weight_column_index: usize,
) -> CliResult<Vec<csv::ByteRecord>> {
    // Seeding rng
    let mut rng = util::acquire_rng(seed);

    // Using algorithm "A-Res" from:
    // 1. Pavlos S. Efraimidis, Paul G. Spirakis. "Weighted random sampling with a reservoir."
    // 2. Pavlos S. Efraimidis. "Weighted Random Sampling over Data Streams."
    let mut reservoir: BinaryHeap<WeightedRow> = BinaryHeap::with_capacity(sample_size as usize);

    for result in rdr.byte_records() {
        let record = result?;

        let weight: f64 = String::from_utf8_lossy(&record[weight_column_index])
            .parse()
            .map_err(|_| CliError::Other("could not parse weight as f64".to_string()))?;

        let score = rng.gen::<f64>().powf(1.0 / weight);
        let weighted_row = WeightedRow(score, record);

        if reservoir.len() < sample_size as usize {
            reservoir.push(weighted_row);
        } else if &weighted_row < reservoir.peek().unwrap() {
            reservoir.pop();
            reservoir.push(weighted_row);
        }
    }

    Ok(reservoir.into_iter().map(|record| record.row()).collect())
}

fn do_random_access(sample_size: u64, total: u64) -> bool {
    sample_size <= (total / 10)
}
