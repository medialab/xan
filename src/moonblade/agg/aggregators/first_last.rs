use crate::moonblade::types::DynamicValue;

// NOTE: I am splitting first and last because first can be more efficient
// This is typically not the case for extents where the amount of copying
// is mostly arbitrary
#[derive(Debug, Clone)]
pub struct First {
    item: Option<(usize, DynamicValue)>,
}

impl First {
    pub fn new() -> Self {
        Self { item: None }
    }

    pub fn clear(&mut self) {
        self.item = None;
    }

    pub fn add(&mut self, index: usize, next_value: &DynamicValue) {
        if self.item.is_none() {
            self.item = Some((index, next_value.clone()));
        }
    }

    pub fn first(&self) -> Option<DynamicValue> {
        self.item.as_ref().map(|p| p.1.clone())
    }

    pub fn merge(&mut self, other: Self) {
        match self.item.as_ref() {
            None => self.item = other.item,
            Some((i, _)) => {
                if let Some((j, _)) = other.item.as_ref() {
                    if i > j {
                        self.item = other.item;
                    }
                }
            }
        };
    }
}

#[derive(Debug, Clone)]
pub struct Last {
    item: Option<(usize, DynamicValue)>,
}

impl Last {
    pub fn new() -> Self {
        Self { item: None }
    }

    pub fn clear(&mut self) {
        self.item = None;
    }

    pub fn add(&mut self, index: usize, next_value: &DynamicValue) {
        self.item = Some((index, next_value.clone()));
    }

    pub fn last(&self) -> Option<DynamicValue> {
        self.item.as_ref().map(|p| p.1.clone())
    }

    pub fn merge(&mut self, other: Self) {
        match self.item.as_ref() {
            None => self.item = other.item,
            Some((i, _)) => {
                if let Some((j, _)) = other.item.as_ref() {
                    if i < j {
                        self.item = other.item;
                    }
                }
            }
        };
    }
}
