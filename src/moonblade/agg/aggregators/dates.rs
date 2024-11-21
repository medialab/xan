use jiff::{Unit, Zoned};

const SECONDS_PER_HOUR: usize = 60 * 60;
const SECONDS_PER_DAY: usize = SECONDS_PER_HOUR * 24;
const SECONDS_PER_YEAR: usize = SECONDS_PER_DAY * 365;

#[derive(Debug, Clone)]
pub struct ZonedExtent {
    extent: Option<(Zoned, Zoned)>,
}

impl ZonedExtent {
    pub fn new() -> Self {
        Self { extent: None }
    }

    pub fn clear(&mut self) {
        self.extent = None;
    }

    pub fn add(&mut self, value: &Zoned) {
        match &mut self.extent {
            None => self.extent = Some((value.clone(), value.clone())),
            Some((min, max)) => {
                if value < *min {
                    *min = value.clone();
                }

                if value > *max {
                    *max = value.clone();
                }
            }
        }
    }

    pub fn earliest(&self) -> Option<Zoned> {
        self.extent.as_ref().map(|(z, _)| z.clone())
    }

    pub fn lastest(&self) -> Option<Zoned> {
        self.extent.as_ref().map(|(_, z)| z.clone())
    }

    pub fn count_seconds(&self) -> Option<usize> {
        self.extent.as_ref().map(|(start, end)| {
            let duration = start.duration_until(end);
            let seconds = duration.as_secs();

            seconds as usize
        })
    }

    pub fn count_hours(&self) -> Option<usize> {
        self.count_seconds()
            .map(|seconds| (seconds as f64 / SECONDS_PER_HOUR as f64).ceil() as usize)
    }

    pub fn count_days(&self) -> Option<usize> {
        self.count_seconds()
            .map(|seconds| (seconds as f64 / SECONDS_PER_DAY as f64).ceil() as usize)
    }

    pub fn count_years(&self) -> Option<usize> {
        self.count_seconds()
            .map(|seconds| (seconds as f64 / SECONDS_PER_YEAR as f64).ceil() as usize)
    }

    pub fn count(&self, unit: Unit) -> Option<usize> {
        match unit {
            Unit::Second => self.count_seconds(),
            Unit::Hour => self.count_hours(),
            Unit::Day => self.count_days(),
            Unit::Year => self.count_years(),
            _ => unimplemented!(),
        }
    }

    pub fn merge(&mut self, other: Self) {
        match self.extent.as_mut() {
            None => {
                self.extent = other.extent;
            }
            Some((min, max)) => {
                if let Some((other_min, other_max)) = other.extent {
                    if other_min < *min {
                        *min = other_min;
                    }
                    if other_max > *max {
                        *max = other_max;
                    }
                }
            }
        }
    }
}
