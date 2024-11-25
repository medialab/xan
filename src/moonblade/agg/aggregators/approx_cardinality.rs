use std::collections::hash_map::RandomState;

use hyperloglogplus::{HyperLogLog, HyperLogLogPlus};

#[derive(Debug, Clone)]
pub struct ApproxCardinality {
    register: HyperLogLogPlus<String, RandomState>,
    count: Option<usize>,
}

impl ApproxCardinality {
    pub fn new() -> Self {
        Self {
            register: HyperLogLogPlus::new(16, RandomState::new()).unwrap(),
            count: None,
        }
    }

    pub fn clear(&mut self) {
        self.register = HyperLogLogPlus::new(16, RandomState::new()).unwrap();
        self.count = None;
    }

    pub fn add(&mut self, string: &str) {
        self.register.insert(string);
    }

    pub fn finalize(&mut self) {
        self.count = Some(self.register.count().trunc() as usize);
    }

    pub fn get(&self) -> usize {
        self.count.expect("not finalized!")
    }

    pub fn merge(&mut self, other: Self) {
        self.register.merge(&other.register).unwrap();
    }
}
