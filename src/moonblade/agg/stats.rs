use csv::ByteRecord;

use super::aggregators::{
    ApproxCardinality, ApproxQuantiles, Count, Extent, Frequencies, LexicographicExtent, Numbers,
    NumericExtent, Sum, Types, Welford,
};
use crate::moonblade::types::DynamicNumber;
use crate::util;

fn map_to_field<T: ToString>(opt: Option<T>) -> Vec<u8> {
    opt.map(|m| m.to_string().as_bytes().to_vec())
        .unwrap_or(b"".to_vec())
}

#[derive(Debug)]
pub struct Stats {
    nulls: bool,
    count: Count,
    extent: NumericExtent,
    length_extent: Extent<usize>,
    lexicograhic_extent: LexicographicExtent,
    welford: Welford,
    sum: Sum,
    types: Types,
    frequencies: Option<Frequencies>,
    numbers: Option<Numbers>,
    approx_cardinality: Option<Box<ApproxCardinality>>,
    approx_quantiles: Option<Box<ApproxQuantiles>>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            nulls: false,
            count: Count::new(),
            extent: NumericExtent::new(),
            length_extent: Extent::new(),
            lexicograhic_extent: LexicographicExtent::new(),
            welford: Welford::new(),
            sum: Sum::new(),
            types: Types::new(),
            frequencies: None,
            numbers: None,
            approx_cardinality: None,
            approx_quantiles: None,
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.count.merge(other.count);
        self.extent.merge(other.extent);
        self.length_extent.merge(other.length_extent);
        self.welford.merge(other.welford);
        self.sum.merge(other.sum);
        self.types.merge(other.types);

        if let Some(frequencies) = &mut self.frequencies {
            frequencies.merge(other.frequencies.unwrap());
        }

        if let Some(numbers) = &mut self.numbers {
            numbers.merge(other.numbers.unwrap());
        }

        if let Some(approx_cardinality) = &mut self.approx_cardinality {
            approx_cardinality.merge(*other.approx_cardinality.unwrap());
        }

        if let Some(approx_quantiles) = &mut self.approx_quantiles {
            approx_quantiles.merge(*other.approx_quantiles.unwrap());
        }
    }

    pub fn include_nulls(&mut self) {
        self.nulls = true;
    }

    pub fn compute_frequencies(&mut self) {
        self.frequencies = Some(Frequencies::new());
    }

    pub fn compute_numbers(&mut self) {
        self.numbers = Some(Numbers::new());
    }

    pub fn compute_approx(&mut self) {
        self.approx_cardinality = Some(Box::new(ApproxCardinality::new()));
        self.approx_quantiles = Some(Box::new(ApproxQuantiles::new()));
    }

    pub fn headers(&self) -> ByteRecord {
        let mut headers = ByteRecord::new();

        headers.push_field(b"field");
        headers.push_field(b"count");
        headers.push_field(b"count_empty");
        headers.push_field(b"type");
        headers.push_field(b"types");
        headers.push_field(b"sum");
        headers.push_field(b"mean");

        if self.numbers.is_some() {
            headers.push_field(b"q1");
            headers.push_field(b"median");
            headers.push_field(b"q3");
        }

        headers.push_field(b"variance");
        headers.push_field(b"stddev");
        headers.push_field(b"min");
        headers.push_field(b"max");

        if self.approx_cardinality.is_some() {
            headers.push_field(b"approx_cardinality");
            headers.push_field(b"approx_q1");
            headers.push_field(b"approx_median");
            headers.push_field(b"approx_q3");
        }

        if self.frequencies.is_some() {
            headers.push_field(b"cardinality");
            headers.push_field(b"mode");
            headers.push_field(b"tied_for_mode");
        }

        headers.push_field(b"lex_first");
        headers.push_field(b"lex_last");
        headers.push_field(b"min_length");
        headers.push_field(b"max_length");

        headers
    }

    pub fn results(self, name: &[u8]) -> ByteRecord {
        let mut record = ByteRecord::new();

        record.push_field(name);
        record.push_field(self.count.get_truthy().to_string().as_bytes());
        record.push_field(self.count.get_falsey().to_string().as_bytes());
        record.push_field(
            self.types
                .most_likely_type()
                .map(|t| t.as_bytes())
                .unwrap_or(b""),
        );
        record.push_field(self.types.sorted_types().join("|").as_bytes());
        record.push_field(&map_to_field(self.sum.get()));
        record.push_field(&map_to_field(self.welford.mean()));

        if let Some(mut numbers) = self.numbers {
            numbers.finalize(false);

            match numbers.quartiles() {
                Some(quartiles) => {
                    for quartile in quartiles {
                        record.push_field(quartile.to_string().as_bytes());
                    }
                }
                None => {
                    for _ in 0..3 {
                        record.push_field(b"");
                    }
                }
            }
        }

        record.push_field(&map_to_field(self.welford.variance()));
        record.push_field(&map_to_field(self.welford.stdev()));
        record.push_field(&map_to_field(self.extent.min()));
        record.push_field(&map_to_field(self.extent.max()));

        if let Some(mut approx_cardinality) = self.approx_cardinality {
            approx_cardinality.finalize();
            record.push_field(approx_cardinality.get().to_string().as_bytes());
        }

        if let Some(mut approx_quantiles) = self.approx_quantiles {
            approx_quantiles.finalize();
            record.push_field(approx_quantiles.get(0.25).to_string().as_bytes());
            record.push_field(approx_quantiles.get(0.5).to_string().as_bytes());
            record.push_field(approx_quantiles.get(0.75).to_string().as_bytes());
        }

        if let Some(frequencies) = self.frequencies.as_ref() {
            record.push_field(frequencies.cardinality().to_string().as_bytes());

            let modes = frequencies.modes();

            record.push_field(&map_to_field(modes.as_ref().map(|m| m[0].clone())));
            record.push_field(&map_to_field(modes.map(|m| m.len())));
        }

        record.push_field(&map_to_field(self.lexicograhic_extent.first()));
        record.push_field(&map_to_field(self.lexicograhic_extent.last()));
        record.push_field(&map_to_field(self.length_extent.min()));
        record.push_field(&map_to_field(self.length_extent.max()));

        record
    }

    pub fn process(&mut self, cell: &[u8]) {
        self.length_extent.add(cell.len());

        if cell.is_empty() {
            self.types.set_empty();
            self.count.add_falsey();

            if self.nulls {
                self.welford.add(0.0);

                if let Some(numbers) = self.numbers.as_mut() {
                    numbers.add(DynamicNumber::Float(0.0));
                }
            }

            return;
        }

        self.count.add_truthy();

        let cell = std::str::from_utf8(cell).expect("could not decode as utf-8");

        if let Ok(number) = cell.parse::<DynamicNumber>() {
            let float = number.as_float();

            self.sum.add(number);
            self.welford.add(float);
            self.extent.add(number);

            match number {
                DynamicNumber::Float(_) => self.types.set_float(),
                DynamicNumber::Integer(_) => self.types.set_int(),
            };

            if let Some(numbers) = self.numbers.as_mut() {
                numbers.add(number);
            }

            if let Some(approx_quantiles) = self.approx_quantiles.as_mut() {
                approx_quantiles.add(float);
            }
        } else if util::is_potentially_date(cell) {
            self.types.set_date();
        } else if util::is_potentially_url(cell) {
            self.types.set_url();
        } else {
            self.types.set_string();
        }

        if let Some(frequencies) = self.frequencies.as_mut() {
            frequencies.add(cell.to_string());
        }

        if let Some(approx_cardinality) = self.approx_cardinality.as_mut() {
            approx_cardinality.add(cell);
        }

        self.lexicograhic_extent.add(cell);
    }
}
