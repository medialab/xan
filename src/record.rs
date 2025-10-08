use std::ops::Index;

pub trait Record: Clone + Index<usize, Output = [u8]> {
    // fn new() -> Self;
    fn len(&self) -> usize;
    fn iter(&self) -> impl DoubleEndedIterator<Item = &[u8]> + ExactSizeIterator;
    // fn get(&self, index: usize) -> Option<&[u8]>;
    // fn clear(&mut self);
    // fn truncate(&mut self, len: usize);
    // fn push_field(&mut self, cell: &[u8]);

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Record for csv::ByteRecord {
    // #[inline(always)]
    // fn new() -> Self {
    //     Self::new()
    // }

    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

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

    // #[inline(always)]
    // fn push_field(&mut self, cell: &[u8]) {
    //     self.push_field(cell);
    // }
}

impl Record for simd_csv::ByteRecord {
    // #[inline(always)]
    // fn new() -> Self {
    //     Self::new()
    // }

    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

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

    // #[inline(always)]
    // fn push_field(&mut self, cell: &[u8]) {
    //     self.push_field(cell);
    // }
}
