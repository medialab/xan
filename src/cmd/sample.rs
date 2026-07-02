use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io;
use std::num::NonZeroUsize;

use rand::{Rng, RngExt};
use simd_csv::ByteRecord;

use crate::CliError;
use crate::CliResult;
use crate::collections::ClusteredInsertHashmap;
use crate::collections::HashMap;
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;

struct GroupReservoir {
    records: Vec<ByteRecord>,
    total: usize,
}

#[derive(PartialEq)]
struct WeightedRow(f64, ByteRecord);

impl WeightedRow {
    fn row(self) -> ByteRecord {
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
    -S, --sorted           Use with -g/--groupby to indicate that input is sorted on group
                           columns so the command can run faster and use memory proportional
                           on sample size rather than group cardinality.
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
    arg_sample_size: NonZeroUsize,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_seed: Option<usize>,
    flag_weight: Option<SelectedColumns>,
    flag_groupby: Option<SelectedColumns>,
    flag_sorted: bool,
    flag_cursed: bool,
}

impl Args {
    fn rconf(&self) -> Config {
        let mut rconfig = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        if let Some(weight_column_selection) = self.flag_weight.clone() {
            rconfig = rconfig.select(weight_column_selection);
        }

        rconfig
    }

    fn wconf(&self) -> Config {
        Config::new(&self.flag_output)
    }

    fn rng(&self) -> Box<dyn Rng> {
        util::acquire_rng(self.flag_seed)
    }

    fn write<R: io::Read>(
        &self,
        rdr: &mut simd_csv::Reader<R>,
        records: impl Iterator<Item = ByteRecord>,
    ) -> CliResult<()> {
        let mut wtr = self.wconf().simd_writer()?;

        if rdr.has_headers() {
            wtr.write_byte_record(rdr.byte_headers()?)?;
        }

        for record in records {
            wtr.write_byte_record(&record)?;
        }

        Ok(wtr.flush()?)
    }

    fn reservoir_sample(&self) -> CliResult<()> {
        let sample_size = self.arg_sample_size.get();
        let mut rng = self.rng();

        let mut rdr = self.rconf().simd_reader()?;
        let mut record = ByteRecord::new();
        let mut i: usize = 0;

        let mut reservoir = Vec::with_capacity(sample_size);

        while rdr.read_byte_record(&mut record)? {
            i += 1;

            if reservoir.len() < sample_size {
                reservoir.push(record.clone());
            } else {
                let random = rng.random_range(0..i);

                if random < sample_size {
                    reservoir[random] = record.clone();
                }
            }
        }

        self.write(&mut rdr, reservoir.into_iter())?;

        Ok(())
    }

    fn weighted_reservoir_sample(&self) -> CliResult<()> {
        let sample_size = self.arg_sample_size.get();
        let mut rng = self.rng();

        let mut rdr = self.rconf().simd_reader()?;
        let has_headers = rdr.has_headers();

        let weight_column_index = self
            .flag_weight
            .as_ref()
            .unwrap()
            .single_selection(rdr.byte_headers()?, has_headers)?;

        // Using algorithm "A-Res" from:
        // 1. Pavlos S. Efraimidis, Paul G. Spirakis. "Weighted random sampling with a reservoir."
        // 2. Pavlos S. Efraimidis. "Weighted Random Sampling over Data Streams."
        let mut reservoir: BinaryHeap<WeightedRow> = BinaryHeap::with_capacity(sample_size);

        for result in rdr.byte_records() {
            let record = result?;

            let weight: f64 = fast_float::parse(&record[weight_column_index])
                .map_err(|_| CliError::Other("could not parse weight as f64".to_string()))?;

            let score = rng.random::<f64>().powf(1.0 / weight);
            let weighted_row = WeightedRow(score, record);

            if reservoir.len() < sample_size {
                reservoir.push(weighted_row);
            } else if &weighted_row < reservoir.peek().unwrap() {
                reservoir.pop();
                reservoir.push(weighted_row);
            }
        }

        self.write(&mut rdr, reservoir.into_iter().map(|record| record.row()))?;

        Ok(())
    }

    fn grouped_reservoir_sample(&self) -> CliResult<()> {
        let sample_size = self.arg_sample_size.get();

        let mut rdr = self.rconf().simd_reader()?;
        let has_headers = rdr.has_headers();

        let group_sel = self
            .flag_groupby
            .as_ref()
            .unwrap()
            .selection(rdr.byte_headers()?, has_headers)?;

        let mut global_reservoir: ClusteredInsertHashmap<ByteRecord, GroupReservoir> =
            ClusteredInsertHashmap::new();

        let mut rng = self.rng();

        for result in rdr.byte_records() {
            let record = result?;
            let group = group_sel.select(&record).collect();

            let reservoir = global_reservoir.insert_with(group, || GroupReservoir {
                records: Vec::with_capacity(1),
                total: 0,
            });

            if reservoir.records.len() < sample_size {
                reservoir.records.push(record);
            } else {
                let random_index = rng.random_range(0..reservoir.total + 1);
                if random_index < sample_size {
                    reservoir.records[random_index] = record;
                }
            }

            reservoir.total += 1;
        }

        self.write(
            &mut rdr,
            global_reservoir.into_values().flat_map(|gr| gr.records),
        )?;

        Ok(())
    }

    fn sorted_grouped_reservoir_sample(&self) -> CliResult<()> {
        let sample_size = self.arg_sample_size.get();
        let mut rng = self.rng();

        let mut rdr = self.rconf().simd_reader()?;
        let has_headers = rdr.has_headers();

        let mut record = ByteRecord::new();

        let mut wtr = self.wconf().simd_writer()?;

        if has_headers {
            wtr.write_byte_record(rdr.byte_headers()?)?;
        }

        let group_sel = self
            .flag_groupby
            .as_ref()
            .unwrap()
            .selection(rdr.byte_headers()?, has_headers)?;

        let mut reservoir: Vec<ByteRecord> = Vec::with_capacity(sample_size);

        let mut current_group_opt: Option<ByteRecord> = None;
        let mut i: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            i += 1;

            let group = group_sel.select(&record).collect();

            if current_group_opt.is_none()
                || matches!(&current_group_opt, Some(current_group) if &group == current_group )
            {
                if reservoir.len() < sample_size {
                    reservoir.push(record.clone());
                } else {
                    let random = rng.random_range(0..i);

                    if random < sample_size {
                        reservoir[random] = record.clone();
                    }
                }
            } else {
                for record in reservoir.iter() {
                    wtr.write_byte_record(record)?;
                }

                reservoir.clear();
                reservoir.push(record.clone());
                i = 1;
            }

            current_group_opt = Some(group);
        }

        for record in reservoir.iter() {
            wtr.write_byte_record(record)?;
        }

        Ok(wtr.flush()?)
    }

    fn weighted_grouped_reservoir_sample(&self) -> CliResult<()> {
        let mut rng = self.rng();

        let sample_size = self.arg_sample_size.get();

        let mut rdr = self.rconf().simd_reader()?;
        let has_headers = rdr.has_headers();

        let group_sel = self
            .flag_groupby
            .as_ref()
            .unwrap()
            .selection(rdr.byte_headers()?, has_headers)?;

        let weight_column_index = self
            .flag_weight
            .as_ref()
            .unwrap()
            .single_selection(rdr.byte_headers()?, has_headers)?;

        let mut global_reservoir: ClusteredInsertHashmap<ByteRecord, BinaryHeap<WeightedRow>> =
            ClusteredInsertHashmap::new();

        for result in rdr.byte_records() {
            let record = result?;

            let group_key = group_sel.select(&record).collect();

            let weight: f64 = fast_float::parse(&record[weight_column_index])
                .map_err(|_| CliError::Other("could not parse weight as f64".to_string()))?;

            let reservoir =
                global_reservoir.insert_with(group_key, || BinaryHeap::with_capacity(1));

            let score = rng.random::<f64>().powf(1.0 / weight);
            let weighted_row = WeightedRow(score, record);

            if reservoir.len() < sample_size {
                reservoir.push(weighted_row);
            } else if &weighted_row < reservoir.peek().unwrap() {
                reservoir.pop();
                reservoir.push(weighted_row);
            }
        }

        self.write(
            &mut rdr,
            global_reservoir
                .into_values()
                .flatten()
                .map(|record| record.row()),
        )?;

        Ok(())
    }

    fn cursed_sample(&self) -> CliResult<()> {
        let mut rng = self.rng();
        let config = self.rconf();
        let sample_size = self.arg_sample_size.get();

        let mut seeker = config.simd_seeker()?.ok_or("Could not sample the file!")?;

        // If sample size is too large wrt whole file approximated number of records we fall back to
        // traditional reservoir sampling:
        if sample_size as u64 > (seeker.approx_count() as f64 * 0.1).ceil() as u64 {
            return self.reservoir_sample();
        }

        let mut records: HashMap<u64, ByteRecord> = HashMap::with_capacity(sample_size);

        'outer: while records.len() < sample_size {
            // NOTE: we only attempt 5 times to find a not yet sampled record
            for _ in 0..5 {
                let random_byte_offset = rng.random_range(seeker.range());

                if let Some((pos, record)) = seeker.find_record_after(random_byte_offset)? {
                    records.insert(pos, record);
                    continue 'outer;
                } else {
                    continue;
                }
            }

            Err("Your data is not cursed enough!")?;
        }

        let mut wtr = self.wconf().simd_writer()?;

        if seeker.has_headers() {
            wtr.write_byte_record(seeker.byte_headers())?;
        }

        for record in records.values() {
            wtr.write_byte_record(record)?;
        }

        Ok(wtr.flush()?)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_cursed && (args.flag_groupby.is_some() || args.flag_weight.is_some()) {
        Err("-§/--cursed does not work with -g/--groubpy nor -w/--weight!")?;
    }

    if args.flag_cursed {
        return args.cursed_sample();
    }

    if args.flag_groupby.is_none() {
        if args.flag_weight.is_none() {
            args.reservoir_sample()
        } else {
            args.weighted_reservoir_sample()
        }
    } else if args.flag_sorted {
        if args.flag_weight.is_some() {
            Err("-S/--sorted is not yet implemented for -w/--weight!".into())
        } else {
            args.sorted_grouped_reservoir_sample()
        }
    } else if args.flag_weight.is_none() {
        args.grouped_reservoir_sample()
    } else {
        args.weighted_grouped_reservoir_sample()
    }
}
