use enumset::{EnumSet, EnumSetType};

use crate::dates::AnyTemporal;

#[derive(Debug, EnumSetType)]
pub enum Type {
    String,
    Float,
    Int,
    Url,
    Zoned,
    DateTime,
    Date,
    Time,
    Empty,
}

impl Type {
    fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Float => "float",
            Self::Int => "int",
            Self::Url => "url",
            Self::Zoned => "zoned_datetime",
            Self::DateTime => "datetime",
            Self::Date => "date",
            Self::Time => "time",
            Self::Empty => "empty",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Types {
    set: EnumSet<Type>,
}

impl Types {
    pub fn new() -> Self {
        Self {
            set: EnumSet::empty(),
        }
    }

    #[inline(always)]
    pub fn set(&mut self, t: Type) {
        self.set.insert(t);
    }

    pub fn set_from_any_temporal(&mut self, t: &AnyTemporal) {
        self.set.insert(match t {
            AnyTemporal::Zoned(_) => Type::Zoned,
            AnyTemporal::DateTime(_) => Type::DateTime,
            AnyTemporal::Date(_) => Type::Date,
            AnyTemporal::Time(_) => Type::Time,
        });
    }

    pub fn most_likely_type(&self) -> Option<&str> {
        let mut working_set = self.set;

        if working_set.len() > 1 && working_set.contains(Type::Empty) {
            working_set.remove(Type::Empty);
        }

        if working_set.contains(Type::Float) && working_set.contains(Type::Int) {
            working_set.remove(Type::Int);
        }

        if working_set.contains(Type::String) {
            working_set.remove(Type::Url);
            working_set.remove(Type::Date);
        }

        match working_set.len() {
            0 => None,
            1 => working_set.iter().next().map(|s| s.as_str()),
            _ => Some("mixed"),
        }
    }

    pub fn sorted_types(&self) -> Vec<&str> {
        let mut result = Vec::new();

        for t in self.set {
            result.push(t.as_str());
        }

        result
    }

    pub fn clear(&mut self) {
        self.set.clear();
    }

    pub fn merge(&mut self, other: Self) {
        self.set |= other.set;
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

        types.set(Type::Int);

        assert_eq!(types.sorted_types(), vec!["int"]);
        assert_eq!(types.most_likely_type(), Some("int"));

        types.set(Type::Float);

        assert_eq!(types.sorted_types(), vec!["float", "int"]);
        assert_eq!(types.most_likely_type(), Some("float"));

        types.set(Type::String);

        assert_eq!(types.sorted_types(), vec!["string", "float", "int"]);
        assert_eq!(types.most_likely_type(), Some("mixed"));
    }
}
