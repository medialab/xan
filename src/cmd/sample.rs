use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io;

use rand::Rng;

use crate::collections::ClusteredInsertHashmap;
use crate::collections::HashMap;
use crate::config::{Config, Delimiter};
use crate::read::{find_next_record_offset_from_random_position, sample_initial_records};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliError;
use crate::CliResult;

type GroupKey = Vec<Vec<u8>>;

static USAGE: &str = "
Randomly samples CSV data uniformly using memory proportional to the size of
the sample.

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
    -§, --cursed           Return a c̵̱̝͆̓ṳ̷̔r̶̡͇͓̍̇š̷̠̎e̶̜̝̿́d̸͔̈́̀ sample from a Lovecraftian kinda-uniform
                           distribution (source: trust me), without requiring to read
                           the whole file. Instead, we will randomly jump through it
                           like a dark wizard. This means the sampled file must
                           be large enough and seekable, so no stdin nor gzipped files.
                           Rows at the very end of the file might be discriminated against
                           because they are not cool enough. If desired sample size is
                           deemed too large for the estimated total number of rows, the
                           c̵̱̝͆̓ṳ̷̔r̶̡͇͓̍̇š̷̠̎e̶̜̝̿́d̸͔̈́̀  routine will fallback to normal reservoir sampling to
                           sidestep the pain of learning O(∞) is actually a thing.
                           Does not work with -w/--weight nor -g/--groupby.

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
    flag_cursed: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_cursed && (args.flag_groupby.is_some() || args.flag_weight.is_some()) {
        Err("-§/--cursed does not work with -g/--groubpy nor -w/--weight!")?;
    }

    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if let Some(weight_column_selection) = args.flag_weight.clone() {
        rconfig = rconfig.select(weight_column_selection);
    }

    let sample_size = args.arg_sample_size;

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconfig.reader()?;

    let byte_headers = rdr.byte_headers()?;

    let group_sel_opt = args
        .flag_groupby
        .map(|s| s.selection(byte_headers, !args.flag_no_headers))
        .transpose()?;

    let sampled = if args.flag_cursed {
        sample_cursed(&rconfig, sample_size, args.flag_seed)?
    } else if args.flag_weight.is_some() {
        let weight_column_index = rconfig.single_selection(byte_headers)?;

        if let Some(group_sel) = group_sel_opt {
            sample_weighted_reservoir_grouped(
                &mut rdr,
                sample_size,
                args.flag_seed,
                weight_column_index,
                group_sel,
            )?
        } else {
            sample_weighted_reservoir(&mut rdr, sample_size, args.flag_seed, weight_column_index)?
        }
    } else if let Some(group_sel) = group_sel_opt {
        sample_reservoir_grouped(&mut rdr, sample_size, args.flag_seed, group_sel)?
    } else {
        sample_reservoir(&mut rdr, sample_size, args.flag_seed)?
    };

    rconfig.write_headers(&mut rdr, &mut wtr)?;

    for row in sampled.into_iter() {
        wtr.write_byte_record(&row)?;
    }

    Ok(wtr.flush()?)
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
        let random = rng.random_range(0..i + 1);
        if random < sample_size as usize {
            reservoir[random] = row?;
        }
    }
    Ok(reservoir)
}

struct GroupReservoir {
    records: Vec<csv::ByteRecord>,
    count: usize,
}

fn sample_reservoir_grouped<R: io::Read>(
    rdr: &mut csv::Reader<R>,
    sample_size: u64,
    seed: Option<usize>,
    group_sel: Selection,
) -> CliResult<Vec<csv::ByteRecord>> {
    let mut global_reservoir: ClusteredInsertHashmap<GroupKey, GroupReservoir> =
        ClusteredInsertHashmap::new();

    let mut rng = util::acquire_rng(seed);

    for result in rdr.byte_records() {
        let record = result?;
        let group = group_sel.collect(&record);

        let reservoir = global_reservoir.insert_with(group, || GroupReservoir {
            records: Vec::with_capacity(1),
            count: 0,
        });

        if reservoir.records.len() < sample_size as usize {
            reservoir.records.push(record);
        } else {
            let random_index = rng.random_range(0..reservoir.count + 1);
            if random_index < sample_size as usize {
                reservoir.records[random_index] = record;
            }
        }

        reservoir.count += 1;
    }

    Ok(global_reservoir
        .into_values()
        .flat_map(|gr| gr.records)
        .collect())
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

        let weight: f64 = fast_float::parse(&record[weight_column_index])
            .map_err(|_| CliError::Other("could not parse weight as f64".to_string()))?;

        let score = rng.random::<f64>().powf(1.0 / weight);
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

fn sample_weighted_reservoir_grouped<R: io::Read>(
    rdr: &mut csv::Reader<R>,
    sample_size: u64,
    seed: Option<usize>,
    weight_column_index: usize,
    group_sel: Selection,
) -> CliResult<Vec<csv::ByteRecord>> {
    let mut rng = util::acquire_rng(seed);

    let mut global_reservoir: ClusteredInsertHashmap<GroupKey, BinaryHeap<WeightedRow>> =
        ClusteredInsertHashmap::new();

    for result in rdr.byte_records() {
        let record = result?;

        let group_key = group_sel.collect(&record);

        let weight: f64 = fast_float::parse(&record[weight_column_index])
            .map_err(|_| CliError::Other("could not parse weight as f64".to_string()))?;

        let reservoir = global_reservoir.insert_with(group_key, || BinaryHeap::with_capacity(1));

        let score = rng.random::<f64>().powf(1.0 / weight);
        let weighted_row = WeightedRow(score, record);

        if reservoir.len() < sample_size as usize {
            reservoir.push(weighted_row);
        } else if &weighted_row < reservoir.peek().unwrap() {
            reservoir.pop();
            reservoir.push(weighted_row);
        }
    }

    Ok(global_reservoir
        .into_values()
        .flatten()
        .map(|record| record.row())
        .collect())
}

fn sample_cursed(
    config: &Config,
    sample_size: u64,
    seed: Option<usize>,
) -> CliResult<Vec<csv::ByteRecord>> {
    let mut reader = config.seekable_reader()?;
    let sample = sample_initial_records(&mut reader, 64)?.ok_or("file is empty!")?;

    // If sample size is too large wrt whole file approximated number of records we fall back to
    // traditional reservoir sampling:
    if sample_size > (sample.exact_or_approx_count() as f64 * 0.1).ceil() as u64 {
        return sample_reservoir(&mut config.reader()?, sample_size, seed);
    }

    // NOTE: we don't need to avoid headers because next record found will always be not the headers
    let range = 0..(sample.file_len - sample.max_record_size * 8);

    let mut rng = util::acquire_rng(seed);
    let mut records: HashMap<u64, csv::ByteRecord> = HashMap::with_capacity(sample_size as usize);

    'outer: while records.len() < sample_size as usize {
        // NOTE: we only attempt 5 times to find a not yet sampled record
        for _ in 0..5 {
            let random_byte_offset = rng.random_range(range.clone());

            if let Some((record, offset)) = find_next_record_offset_from_random_position(
                &mut reader,
                random_byte_offset,
                &sample,
                8,
            )?
            .into_record_with_offset()
            {
                records.insert(offset, record);
                continue 'outer;
            } else {
                continue;
            }
        }

        Err("Your data is not cursed enough!")?;
    }

    Ok(records.into_values().collect())
}
