use std::ptr;
use wide::{u8x32, CmpEq};

const STEP: usize = 32;

fn get_for_offset(mask: u32) -> u32 {
    #[cfg(target_endian = "big")]
    {
        mask.swap_bytes()
    }
    #[cfg(target_endian = "little")]
    {
        mask
    }
}

pub struct SimdSplit<'i, 's> {
    input: &'i [u8],
    splitter: &'s SimdSplitter,
    offset: usize,
    bitmask: Option<u32>,
}

impl<'i, 's> Iterator for SimdSplit<'i, 's> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        'main: loop {
            if let Some(bitmask) = &mut self.bitmask {
                let m = *bitmask;

                let actual_offset = self.offset + get_for_offset(m).trailing_zeros() as usize;

                let new_bitmask = m & (m - 1);

                if new_bitmask == 0 {
                    self.bitmask = None;
                    self.offset += STEP;
                } else {
                    *bitmask = new_bitmask;
                }

                return Some(actual_offset);
            }

            let len = self.input.len();

            if self.offset >= len {
                return None;
            }

            loop {
                if self.offset + STEP <= len {
                    let chunk = unsafe {
                        u8x32::new(ptr::read(
                            self.input[self.offset..].as_ptr() as *const [u8; 32]
                        ))
                    };

                    // Compare with each delimiter
                    let mut mask = chunk.cmp_eq(self.splitter.v1);
                    mask |= chunk.cmp_eq(self.splitter.v2);
                    mask |= chunk.cmp_eq(self.splitter.v3);

                    let bitmask = mask.move_mask() as u32;

                    if bitmask != 0 {
                        self.bitmask = Some(bitmask);
                        continue 'main;
                    }

                    self.offset += STEP;
                } else {
                    while self.offset < len {
                        let c = self.input[self.offset];
                        self.offset += 1;

                        if c == self.splitter.n1 || c == self.splitter.n2 || c == self.splitter.n3 {
                            return Some(self.offset - 1);
                        }
                    }

                    return None;
                }
            }
        }
    }
}

pub struct SimdSplitter {
    n1: u8,
    n2: u8,
    n3: u8,
    v1: u8x32,
    v2: u8x32,
    v3: u8x32,
}

impl SimdSplitter {
    pub fn new(n1: u8, n2: u8, n3: u8) -> Self {
        Self {
            n1,
            n2,
            n3,
            v1: u8x32::splat(n1),
            v2: u8x32::splat(n2),
            v3: u8x32::splat(n3),
        }
    }

    pub fn split<'s, 'i>(&'s self, input: &'i [u8]) -> SimdSplit<'i, 's> {
        SimdSplit {
            input,
            splitter: self,
            offset: 0,
            bitmask: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split() {
        let splitter = SimdSplitter::new(b'\n', b'"', b',');
        let string = b"name,\"surname\",age,color,oper\n,\n,\nation,punctuation\nname,surname,age,color,operation,punctuation";

        dbg!(string.len());
        let offsets = splitter.split(string).collect::<Vec<_>>();
        dbg!(
            &offsets,
            offsets
                .iter()
                .copied()
                .map(|i| bstr::BStr::new(&string[i..i + 1]))
                .collect::<Vec<_>>()
        );

        // Not found at all
        assert_eq!(
            splitter
                .split("b".repeat(75).as_bytes())
                .collect::<Vec<_>>(),
            Vec::<usize>::new()
        );

        // Regular
        assert_eq!(splitter.split("b,".repeat(75).as_bytes()).count(), 75);

        // Exactly 64
        assert_eq!(splitter.split(",".repeat(64).as_bytes()).count(), 64);
    }
}
