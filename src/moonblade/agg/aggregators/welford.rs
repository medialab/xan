// NOTE: this is an implementation of Welford's online algorithm
// Ref: https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance
// Ref: https://en.wikipedia.org/wiki/Standard_deviation
#[derive(Debug, Clone)]
pub struct Welford {
    count: usize,
    mean: f64,
    m2: f64,
}

impl Welford {
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }

    pub fn add(&mut self, value: f64) {
        let (mut count, mut mean, mut m2) = (self.count, self.mean, self.m2);
        count += 1;
        let delta = value - mean;
        mean += delta / count as f64;
        let delta2 = value - mean;
        m2 += delta * delta2;

        self.count = count;
        self.mean = mean;
        self.m2 = m2;
    }

    pub fn mean(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }

        Some(self.mean)
    }

    pub fn variance(&self) -> Option<f64> {
        if self.count < 1 {
            return None;
        }

        Some(self.m2 / self.count as f64)
    }

    pub fn sample_variance(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }

        Some(self.m2 / (self.count - 1) as f64)
    }

    pub fn stdev(&self) -> Option<f64> {
        self.variance().map(|v| v.sqrt())
    }

    pub fn sample_stdev(&self) -> Option<f64> {
        self.sample_variance().map(|v| v.sqrt())
    }

    pub fn merge(&mut self, other: Self) {
        self.count += other.count;

        let count1 = self.count as f64;
        let count2 = self.count as f64;

        let total = count1 + count2;

        let mean_diff_squared = (self.mean - other.mean).powi(2);
        self.mean = ((count1 * self.mean) + (count2 * other.mean)) / total;

        self.m2 = (((count1 * self.m2) + (count2 * other.m2)) / total)
            + ((count1 * count2 * mean_diff_squared) / (total * total));
    }
}
