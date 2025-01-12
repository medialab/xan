// NOTE: this is an implementation of Welford's online algorithm
// Ref: https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance
// Ref: https://en.wikipedia.org/wiki/Standard_deviation
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Welford {
    count: usize,
    mean: f64,
    m2: f64,
}

impl Welford {
    pub fn new() -> Self {
        Self::default()
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
        if other.count == 0 {
            return;
        }

        if self.count == 0 {
            other.clone_into(self);
        }

        let count1 = self.count as f64;
        let count2 = other.count as f64;

        let total = count1 + count2;

        let mean_diff_squared = (self.mean - other.mean).powi(2);
        self.mean = ((count1 * self.mean) + (count2 * other.mean)) / total;

        self.m2 = self.m2 + other.m2 + ((count1 * count2 * mean_diff_squared) / total);

        self.count += other.count;
    }
}

// NOTE: https://stackoverflow.com/questions/45773857/merging-covariance-from-two-sets-to-create-new-covariance
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CovarianceWelford {
    count: usize,
    mean_x: f64,
    mean_y: f64,
    m2_x: f64,
    m2_y: f64,
    c: f64,
}

impl CovarianceWelford {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.mean_x = 0.0;
        self.mean_y = 0.0;
        self.m2_x = 0.0;
        self.m2_y = 0.0;
        self.c = 0.0;
    }

    pub fn add(&mut self, x: f64, y: f64) {
        let mut count = self.count;
        let (mut mean_x, mut mean_y) = (self.mean_x, self.mean_y);
        let (mut m2_x, mut m2_y) = (self.m2_x, self.m2_y);

        count += 1;

        let delta_x = x - mean_x;
        let delta_y = y - mean_y;

        mean_x += delta_x / count as f64;
        mean_y += delta_y / count as f64;

        let delta2_x = x - mean_x;
        let delta2_y = y - mean_y;

        m2_x += delta_x * delta2_x;
        m2_y += delta_y * delta2_y;

        self.count = count;

        self.mean_x = mean_x;
        self.mean_y = mean_y;

        self.m2_x = m2_x;
        self.m2_y = m2_y;

        self.c += delta_x * (y - mean_y);
    }

    pub fn covariance(&self) -> Option<f64> {
        if self.count < 1 {
            return None;
        }

        Some(self.c / self.count as f64)
    }

    pub fn sample_covariance(&self) -> Option<f64> {
        if self.count < 2 {
            return None;
        }

        Some(self.c / (self.count - 1) as f64)
    }

    pub fn correlation(&self) -> Option<f64> {
        if self.count < 1 {
            return None;
        }

        if self.m2_x == self.m2_y && self.mean_x == self.mean_y && self.m2_x == self.c {
            return Some(1.0);
        }

        let count = self.count as f64;

        let stdev_x = (self.m2_x / count).sqrt();
        let stdev_y = (self.m2_y / count).sqrt();

        let covariance = self.c / count;

        Some(covariance / (stdev_x * stdev_y))
    }

    pub fn merge(&mut self, other: Self) {
        if other.count == 0 {
            return;
        }

        if self.count == 0 {
            other.clone_into(self);
        }

        let count1 = self.count as f64;
        let count2 = other.count as f64;

        let total = count1 + count2;

        let mean_diff_squared_x = (self.mean_x - other.mean_x).powi(2);
        let new_mean_x = ((count1 * self.mean_x) + (count2 * other.mean_x)) / total;

        self.m2_x = self.m2_x + other.m2_x + ((count1 * count2 * mean_diff_squared_x) / total);

        let mean_diff_squared_y = (self.mean_y - other.mean_y).powi(2);
        let new_mean_y = ((count1 * self.mean_y) + (count2 * other.mean_y)) / total;

        self.m2_y = self.m2_y + other.m2_y + ((count1 * count2 * mean_diff_squared_y) / total);

        self.c = self.c
            + count1 * (self.mean_x - new_mean_x) * (self.mean_y - new_mean_y)
            + other.c
            + count2 * (other.mean_x - new_mean_x) * (other.mean_y - new_mean_y);

        self.mean_x = new_mean_x;
        self.mean_y = new_mean_y;

        self.count += other.count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_covariance_correctness() {
        // Welford equivalence
        let numbers = [1.0, 2.0, 3.0, 4.0, 5.0];

        let mut welford = Welford::new();
        let mut covariance_welford = CovarianceWelford::new();

        for n in numbers {
            welford.add(n);
            covariance_welford.add(n, n);
        }

        assert_eq!(welford.count, covariance_welford.count);
        assert_eq!(welford.mean, covariance_welford.mean_x);
        assert_eq!(welford.mean, covariance_welford.mean_y);
        assert_eq!(welford.m2, covariance_welford.m2_x);
        assert_eq!(welford.m2, covariance_welford.m2_y);

        welford.merge(welford.clone());
        covariance_welford.merge(covariance_welford.clone());

        assert_eq!(welford.count, covariance_welford.count);
        assert_eq!(welford.mean, covariance_welford.mean_x);
        assert_eq!(welford.mean, covariance_welford.mean_y);
        assert_eq!(welford.m2, covariance_welford.m2_x);
        assert_eq!(welford.m2, covariance_welford.m2_y);

        // Proper covariance results
        let xs = [1.0, 4.0, 5.0, 7.0, 9.0];
        let ys = [0.0, 6.0, 7.0, 9.0, 3.0];

        let mut covariance_welford = CovarianceWelford::new();

        for (x, y) in xs.iter().copied().zip(ys.iter().copied()) {
            covariance_welford.add(x, y);
        }

        assert_eq!(covariance_welford.covariance(), Some(3.8));
        assert_eq!(covariance_welford.sample_covariance(), Some(4.75));
        assert_eq!(covariance_welford.correlation(), Some(0.442939783914149));

        // Complete correlation (and test clearing)
        covariance_welford.clear();

        for x in xs.iter().copied() {
            covariance_welford.add(x, x);
        }

        assert_eq!(covariance_welford.covariance(), Some(7.359999999999999));
        assert_eq!(covariance_welford.sample_covariance(), Some(9.2));
        assert_eq!(covariance_welford.correlation(), Some(1.0));

        // Merging correctness
        let mut welford_left = Welford::new();
        let mut welford_right = Welford::new();
        let mut covariance_left = CovarianceWelford::new();
        let mut covariance_right = CovarianceWelford::new();
        welford.clear();
        covariance_welford.clear();

        for (x, y) in xs[..2].iter().copied().zip(ys[..2].iter().copied()) {
            welford_left.add(x);
            welford.add(x);
            covariance_left.add(x, y);
            covariance_welford.add(x, y);
        }

        for (x, y) in xs[2..].iter().copied().zip(ys[2..].iter().copied()) {
            welford_right.add(x);
            welford.add(x);
            covariance_right.add(x, y);
            covariance_welford.add(x, y);
        }

        welford_left.merge(welford_right);
        covariance_left.merge(covariance_right);

        assert_eq!(welford, welford_left);
        assert_eq!(covariance_welford, covariance_left);
    }
}
