use std::collections::VecDeque;
use std::num::NonZeroUsize;

pub struct ContextBuffer<T> {
    before_buffer: Option<VecDeque<T>>,
    after_window: Option<usize>,
    after_counter: usize,
}

impl<T> ContextBuffer<T> {
    pub fn new(before: Option<NonZeroUsize>, after: Option<NonZeroUsize>) -> Self {
        Self {
            before_buffer: before.map(|capacity| VecDeque::with_capacity(capacity.get())),
            after_window: after.map(|n| n.get()),
            after_counter: 0,
        }
    }
}

impl<T: Clone> ContextBuffer<T> {
    #[inline]
    fn push_owned(&mut self, item: T) {
        if let Some(before) = self.before_buffer.as_mut() {
            if before.len() == before.capacity() {
                before.pop_front();
            }

            before.push_back(item);
        }
    }

    // NOTE: I don't factorize this method with `push_owned` to avoid double
    // Option test and also avoid an unrequired clone when only after context.
    #[inline]
    fn push(&mut self, item: &T) {
        if let Some(before) = self.before_buffer.as_mut() {
            if before.len() == before.capacity() {
                before.pop_front();
            }

            before.push_back(item.clone());
        }
    }

    pub fn try_process<F, E>(&mut self, is_match: bool, item: &T, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&T) -> Result<(), E>,
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

    pub fn try_process_owned<F, E>(
        &mut self,
        is_match: bool,
        item: T,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&T) -> Result<(), E>,
    {
        if is_match {
            if let Some(before) = self.before_buffer.as_mut() {
                for past_item in before.drain(..) {
                    callback(&past_item)?;
                }
            }

            callback(&item)?;

            if let Some(w) = self.after_window {
                self.after_counter = w;
            }
        } else if self.after_counter > 0 {
            self.after_counter -= 1;
            callback(&item)?;
        } else {
            self.push_owned(item);
        }

        Ok(())
    }
}

impl ContextBuffer<Vec<u8>> {
    #[inline]
    fn push_bytes(&mut self, item: &[u8]) {
        if let Some(before) = self.before_buffer.as_mut() {
            if before.len() == before.capacity() {
                before.pop_front();
            }

            before.push_back(item.to_vec());
        }
    }

    pub fn try_process_bytes<F, E>(
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
            self.push_bytes(item);
        }

        Ok(())
    }
}
