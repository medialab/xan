use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

#[derive(Clone, Debug)]
pub struct Arbitrary<T>(pub T);

impl<T> PartialEq for Arbitrary<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for Arbitrary<T> {}

impl<T> PartialOrd for Arbitrary<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Arbitrary<T> {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}

// A specialized heap handy to compute top-k in O(log n) time
// but only O(k) memory.
// It is a max-heap by default to fit rust's standard library
// choices.
#[derive(Debug, Clone)]
pub struct FixedReverseHeap<T> {
    capacity: usize,
    heap: BinaryHeap<Reverse<T>>,
}

impl<T: Ord> FixedReverseHeap<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            heap: BinaryHeap::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, item: T) -> bool {
        let heap = &mut self.heap;

        if heap.len() < self.capacity {
            heap.push(Reverse(item));

            return true;
        } else {
            let worst_item = heap.peek().unwrap();

            if item > worst_item.0 {
                heap.pop();
                heap.push(Reverse(item));
                return true;
            }
        }

        false
    }

    pub fn into_sorted_vec(mut self) -> Vec<T> {
        let l = self.heap.len();

        let mut items = Vec::with_capacity(l);
        let uninit = items.spare_capacity_mut();

        let mut i: usize = l;

        while let Some(Reverse(item)) = self.heap.pop() {
            i -= 1;
            uninit[i].write(item);
        }

        unsafe {
            items.set_len(l);
        }

        items
    }
}

impl<T: Ord> Extend<T> for FixedReverseHeap<T> {
    // Required method
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        for item in iter {
            self.push(item);
        }
    }
}

#[derive(Clone, Debug)]
pub struct FixedReverseHeapMap<T, V> {
    capacity: usize,
    heap: BinaryHeap<(Reverse<T>, Arbitrary<V>)>,
}

impl<T: Ord, V> FixedReverseHeapMap<T, V> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            heap: BinaryHeap::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.heap.clear();
    }

    pub fn into_unordered_iter(self) -> impl Iterator<Item = (T, V)> {
        self.heap
            .into_iter()
            .map(|(Reverse(k), Arbitrary(v))| (k, v))
    }

    pub fn push_with<F>(&mut self, item: T, callback: F) -> bool
    where
        F: FnOnce() -> V,
    {
        let heap = &mut self.heap;

        if heap.len() < self.capacity {
            heap.push((Reverse(item), Arbitrary(callback())));

            return true;
        } else {
            let worst_item = heap.peek().unwrap();

            if item > worst_item.0 .0 {
                heap.pop();
                heap.push((Reverse(item), Arbitrary(callback())));
                return true;
            }
        }

        false
    }

    pub fn into_sorted_vec(mut self) -> Vec<(T, V)> {
        let l = self.heap.len();

        let mut items = Vec::with_capacity(l);
        let uninit = items.spare_capacity_mut();

        let mut i: usize = l;

        while let Some((Reverse(item), Arbitrary(value))) = self.heap.pop() {
            i -= 1;
            uninit[i].write((item, value));
        }

        unsafe {
            items.set_len(l);
        }

        items
    }
}

impl<T: Ord + Clone, V: Clone> FixedReverseHeapMap<T, V> {
    pub fn to_sorted_vec(&self) -> Vec<(T, V)> {
        self.clone().into_sorted_vec()
    }
}

#[derive(Clone, Debug)]
pub struct FixedReverseHeapMapWithTies<T, V> {
    capacity: usize,
    heap: BinaryHeap<(Reverse<T>, Arbitrary<V>)>,
    ties: Vec<(T, V)>,
}

impl<T: Ord, V> FixedReverseHeapMapWithTies<T, V> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            heap: BinaryHeap::with_capacity(capacity),
            ties: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.heap.len() + self.ties.len()
    }

    pub fn push_with<F>(&mut self, item: T, callback: F) -> bool
    where
        F: FnOnce() -> V,
    {
        let heap = &mut self.heap;

        if heap.len() < self.capacity {
            heap.push((Reverse(item), Arbitrary(callback())));

            return true;
        } else {
            let worst_item = heap.peek().unwrap();

            match item.cmp(&worst_item.0 .0) {
                Ordering::Greater => {
                    heap.pop();
                    heap.push((Reverse(item), Arbitrary(callback())));
                    self.ties.clear();
                    return true;
                }
                Ordering::Equal => {
                    self.ties.push((item, callback()));
                    return true;
                }
                _ => (),
            };
        }

        false
    }

    pub fn into_sorted_vec(mut self) -> Vec<(T, V)> {
        let l = self.len();
        let hl = self.heap.len();

        let mut items = Vec::with_capacity(l);
        let uninit = items.spare_capacity_mut();

        let mut i: usize = hl;

        while let Some((Reverse(item), Arbitrary(value))) = self.heap.pop() {
            i -= 1;
            uninit[i].write((item, value));
        }

        i = hl;

        for pair in self.ties {
            uninit[i].write(pair);
            i += 1;
        }

        unsafe {
            items.set_len(l);
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_numbers() {
        let mut heap = FixedReverseHeap::with_capacity(3);
        heap.extend([1, 2, 3, 4, 5, 6]);

        assert_eq!(heap.into_sorted_vec(), vec![6, 5, 4]);
    }

    #[test]
    fn test_reverse_numbers() {
        let mut heap = FixedReverseHeap::with_capacity(3);
        heap.extend([1, 2, 3, 4, 5, 6].into_iter().map(Reverse));

        assert_eq!(
            heap.into_sorted_vec()
                .iter()
                .map(|n| n.0)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn test_map() {
        let mut heap = FixedReverseHeapMap::with_capacity(2);
        heap.push_with(1, || "one");
        heap.push_with(2, || "two");
        heap.push_with(3, || "three");

        assert_eq!(
            heap.clone().into_sorted_vec(),
            vec![(3, "three"), (2, "two")]
        );
    }

    #[test]
    fn test_map_with_ties() {
        let mut heap = FixedReverseHeapMapWithTies::with_capacity(2);
        heap.push_with(1, || "one");
        heap.push_with(2, || "two");
        heap.push_with(3, || "three");

        assert_eq!(
            heap.clone().into_sorted_vec(),
            vec![(3, "three"), (2, "two")]
        );

        // Final ties
        let mut heap = FixedReverseHeapMapWithTies::with_capacity(2);
        heap.push_with(1, || "one");
        heap.push_with(2, || "two");
        heap.push_with(3, || "three");
        heap.push_with(2, || "four");
        heap.push_with(2, || "five");

        assert_eq!(
            heap.clone().into_sorted_vec(),
            vec![(3, "three"), (2, "two"), (2, "four"), (2, "five")]
        );
    }
}
