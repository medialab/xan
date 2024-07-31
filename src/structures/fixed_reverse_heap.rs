use std::cmp::Reverse;
use std::collections::BinaryHeap;

// A specialized heap handy to compute top-k in O(log n) time
// but only O(k) memory.
// It is a max-heap by default to fit rust's standard library
// choices.
pub struct FixedReverseHeap<T> {
    heap: BinaryHeap<Reverse<T>>,
}

impl<T: Ord> FixedReverseHeap<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, item: T) {
        let heap = &mut self.heap;

        if heap.len() < heap.capacity() {
            heap.push(Reverse(item));
        } else {
            let worst_item = heap.peek().unwrap();

            if item > worst_item.0 {
                heap.pop();
                heap.push(Reverse(item));
            }
        }
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
}
