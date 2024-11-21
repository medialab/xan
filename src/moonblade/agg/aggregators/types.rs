const TYPE_EMPTY: u8 = 0;
const TYPE_STRING: u8 = 1;
const TYPE_FLOAT: u8 = 2;
const TYPE_INT: u8 = 3;
const TYPE_DATE: u8 = 4;
const TYPE_URL: u8 = 5;

#[derive(Debug, Clone)]
pub struct Types {
    bitset: u8,
}

impl Types {
    pub fn new() -> Self {
        Self { bitset: 0 }
    }

    pub fn set(&mut self, pos: u8) {
        self.bitset |= 1 << pos;
    }

    pub fn set_empty(&mut self) {
        self.set(TYPE_EMPTY);
    }

    pub fn set_string(&mut self) {
        self.set(TYPE_STRING);
    }

    pub fn set_float(&mut self) {
        self.set(TYPE_FLOAT);
    }

    pub fn set_int(&mut self) {
        self.set(TYPE_INT);
    }

    pub fn set_date(&mut self) {
        self.set(TYPE_DATE);
    }

    pub fn set_url(&mut self) {
        self.set(TYPE_URL);
    }

    pub fn has(&self, pos: u8) -> bool {
        ((self.bitset >> pos) & 1) == 1
    }

    pub fn has_empty(&self) -> bool {
        self.has(TYPE_EMPTY)
    }

    pub fn has_string(&self) -> bool {
        self.has(TYPE_STRING)
    }

    pub fn has_float(&self) -> bool {
        self.has(TYPE_FLOAT)
    }

    pub fn has_int(&self) -> bool {
        self.has(TYPE_INT)
    }

    pub fn has_date(&self) -> bool {
        self.has(TYPE_DATE)
    }

    pub fn has_url(&self) -> bool {
        self.has(TYPE_URL)
    }

    pub fn most_likely_type(&self) -> Option<&str> {
        Some(if self.has_string() {
            "string"
        } else if self.has_float() {
            "float"
        } else if self.has_int() {
            "int"
        } else if self.has_url() {
            "url"
        } else if self.has_date() {
            "date"
        } else if self.has_empty() {
            "empty"
        } else {
            return None;
        })
    }

    pub fn sorted_types(&self) -> Vec<&str> {
        let mut result = Vec::new();

        if self.has_int() {
            result.push("int");
        }
        if self.has_float() {
            result.push("float");
        }
        if self.has_string() {
            result.push("string");
        }
        if self.has_date() {
            result.push("date");
        }
        if self.has_url() {
            result.push("url");
        }
        if self.has_empty() {
            result.push("empty");
        }

        result
    }

    pub fn clear(&mut self) {
        self.bitset = 0;
        self.set_empty();
    }

    pub fn merge(&mut self, other: Self) {
        self.bitset |= other.bitset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_types_aggregator() {
        let mut types = Types::new();

        assert_eq!(types.sorted_types(), Vec::<&str>::new());
        assert_eq!(types.most_likely_type(), None);

        types.set_int();

        assert_eq!(types.sorted_types(), vec!["int"]);
        assert_eq!(types.most_likely_type(), Some("int"));

        types.set_float();

        assert_eq!(types.sorted_types(), vec!["int", "float"]);
        assert_eq!(types.most_likely_type(), Some("float"));

        types.set_string();

        assert_eq!(types.sorted_types(), vec!["int", "float", "string"]);
        assert_eq!(types.most_likely_type(), Some("string"));
    }
}
