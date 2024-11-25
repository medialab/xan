#[derive(Debug, Clone)]
pub struct Values {
    values: Vec<String>,
}

impl Values {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.values.clear()
    }

    pub fn add(&mut self, string: String) {
        self.values.push(string);
    }

    pub fn join(&self, separator: &str) -> String {
        self.values.join(separator)
    }

    pub fn merge(&mut self, other: Self) {
        self.values.extend(other.values);
    }
}
