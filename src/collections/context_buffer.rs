use std::collections::VecDeque;
use std::num::NonZeroUsize;

pub struct ContextBuffer {
    before_buffer: Option<VecDeque<Vec<u8>>>,
    after_window: Option<usize>,
    after_counter: usize,
}

impl ContextBuffer {
    pub fn new(before: Option<NonZeroUsize>, after: Option<NonZeroUsize>) -> Self {
        Self {
            before_buffer: before.map(|capacity| VecDeque::with_capacity(capacity.get())),
            after_window: after.map(|n| n.get()),
            after_counter: 0,
        }
    }

    #[inline]
    fn push(&mut self, item: &[u8]) {
        if let Some(before) = self.before_buffer.as_mut() {
            if before.len() == before.capacity() {
                before.pop_front();
            }

            before.push_back(item.to_vec());
        }
    }

    pub fn try_process<F, E>(
        &mut self,
        is_match: bool,
        item: &[u8],
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&[u8]) -> Result<(), E>,
    {
        if is_match {
            if let Some(before) = self.before_buffer.as_mut() {
                for past_item in before.drain(..) {
                    callback(&past_item)?;
                }
            }

            callback(item)?;

            if let Some(w) = self.after_window {
                self.after_counter = w;
            }
        } else if self.after_counter > 0 {
            self.after_counter -= 1;
            callback(item)?;
        } else {
            self.push(item);
        }

        Ok(())
    }
}
