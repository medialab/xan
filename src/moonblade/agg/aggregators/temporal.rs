use jiff::{Error, Unit};

use crate::temporal::AnyTemporal;

#[derive(Debug, Clone)]
pub struct TemporalExtent {
    extent: Option<(AnyTemporal, AnyTemporal)>,
}

impl TemporalExtent {
    pub fn new() -> Self {
        Self { extent: None }
    }

    pub fn clear(&mut self) {
        self.extent = None;
    }

    pub fn add(&mut self, value: AnyTemporal) -> Result<(), String> {
        match &mut self.extent {
            None => self.extent = Some((value.clone(), value)),
            Some((min, max)) => {
                if !min.has_same_type(&value) {
                    return Err(format!("temporal extent attempting to handle incompatible types: {} and {} (current min: {:?}, given value: {:?})", min.kind_as_str(), value.kind_as_str(), min, value));
                }

                if value < *min {
                    *min = value.clone();
                } else if value > *max {
                    *max = value.clone();
                }
            }
        };

        Ok(())
    }

    pub fn earliest(&self) -> Option<AnyTemporal> {
        self.extent.as_ref().map(|(z, _)| z.clone())
    }

    pub fn lastest(&self) -> Option<AnyTemporal> {
        self.extent.as_ref().map(|(_, z)| z.clone())
    }

    pub fn count(&self, unit: Unit) -> Result<Option<f64>, Error> {
        match &self.extent {
            None => Ok(None),
            Some((earliest, latest)) => earliest.relative_total(latest, unit).map(Some),
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
