#[derive(Debug, Clone)]
pub struct Count {
    truthy: usize,
    falsey: usize,
}

impl Count {
    pub fn new() -> Self {
        Self {
            truthy: 0,
            falsey: 0,
        }
    }

    pub fn clear(&mut self) {
        self.truthy = 0;
        self.falsey = 0;
    }

    pub fn add(&mut self, truthy: bool) {
        if truthy {
            self.truthy += 1;
        } else {
            self.falsey += 1;
        }
    }

    pub fn add_truthy(&mut self) {
        self.truthy += 1;
    }

    pub fn add_falsey(&mut self) {
        self.falsey += 1
    }

    pub fn get_truthy(&self) -> usize {
        self.truthy
    }

    pub fn get_falsey(&self) -> usize {
        self.falsey
    }

    pub fn get_total(&self) -> usize {
        self.truthy + self.falsey
    }

    pub fn ratio(&self) -> f64 {
        self.truthy as f64 / self.get_total() as f64
    }

    pub fn percentage(&self) -> String {
        format!("{}%", ((self.ratio() * 100.0) as usize))
    }

    pub fn merge(&mut self, other: Self) {
        self.truthy += other.truthy;
        self.falsey += other.falsey;
    }
}
