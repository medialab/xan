#[derive(Debug, Clone)]
pub struct AllAny {
    all: bool,
    any: bool,
}

impl AllAny {
    pub fn new() -> Self {
        Self {
            all: true,
            any: false,
        }
    }

    pub fn clear(&mut self) {
        self.all = true;
        self.any = false;
    }

    pub fn add(&mut self, new_bool: bool) {
        self.all = self.all && new_bool;
        self.any = self.any || new_bool;
    }

    pub fn all(&self) -> bool {
        self.all
    }

    pub fn any(&self) -> bool {
        self.any
    }

    pub fn merge(&mut self, other: Self) {
        self.all = self.all && other.all;
        self.any = self.any || other.any;
    }
}
