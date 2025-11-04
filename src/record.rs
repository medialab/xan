use std::ops::Index;

pub trait Record: Clone + Index<usize, Output = [u8]> {
    fn new() -> Self;
    // fn len(&self) -> usize;
    fn iter(&self) -> impl DoubleEndedIterator<Item = &[u8]> + ExactSizeIterator;
    // fn get(&self, index: usize) -> Option<&[u8]>;
    // fn clear(&mut self);
    // fn truncate(&mut self, len: usize);
    fn push_field(&mut self, cell: &[u8]);

    // #[inline(always)]
    // fn is_empty(&self) -> bool {
    //     self.len() == 0
    // }

    #[must_use]
    fn replace_at<'a>(&'a self, column_index: usize, new_value: &'a [u8]) -> Self
    where
        Self: FromIterator<&'a [u8]>,
    {
        self.iter()
            .enumerate()
            .map(|(i, v)| if i == column_index { new_value } else { v })
            .collect()
    }

    #[must_use]
    fn prepend<'a>(&'a self, cell_value: &[u8]) -> Self
    where
        Self: Extend<&'a [u8]>,
    {
        let mut new_record = Self::new();
        new_record.push_field(cell_value);
        new_record.extend(self.iter());

        new_record
    }

    #[must_use]
    fn append(&self, cell_value: &[u8]) -> Self {
        let mut new_record = self.clone();
        new_record.push_field(cell_value);
        new_record
    }

    #[must_use]
    fn remove<'a>(&'a self, column_index: usize) -> Self
    where
        Self: FromIterator<&'a [u8]>,
    {
        self.iter()
            .enumerate()
            .filter_map(|(i, c)| if i == column_index { None } else { Some(c) })
            .collect()
    }
}

impl Record for csv::ByteRecord {
    #[inline(always)]
    fn new() -> Self {
        Self::new()
    }

    // #[inline(always)]
    // fn len(&self) -> usize {
    //     self.len()
    // }

    #[inline(always)]
    fn iter(&self) -> impl DoubleEndedIterator<Item = &[u8]> + ExactSizeIterator {
        self.iter()
    }

    // #[inline(always)]
    // fn get(&self, index: usize) -> Option<&[u8]> {
    //     self.get(index)
    // }

    // #[inline(always)]
    // fn clear(&mut self) {
    //     self.clear();
    // }

    // #[inline(always)]
    // fn truncate(&mut self, len: usize) {
    //     self.truncate(len);
    // }

    #[inline(always)]
    fn push_field(&mut self, cell: &[u8]) {
        self.push_field(cell);
    }
}

impl Record for simd_csv::ByteRecord {
    #[inline(always)]
    fn new() -> Self {
        Self::new()
    }

    // #[inline(always)]
    // fn len(&self) -> usize {
    //     self.len()
    // }

    #[inline(always)]
    fn iter(&self) -> impl DoubleEndedIterator<Item = &[u8]> + ExactSizeIterator {
        self.iter()
    }

    // #[inline(always)]
    // fn get(&self, index: usize) -> Option<&[u8]> {
    //     self.get(index)
    // }

    // #[inline(always)]
    // fn clear(&mut self) {
    //     self.clear();
    // }

    // #[inline(always)]
    // fn truncate(&mut self, len: usize) {
    //     self.truncate(len);
    // }

    #[inline(always)]
    fn push_field(&mut self, cell: &[u8]) {
        self.push_field(cell);
    }
}
