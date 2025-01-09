use tdigest::TDigest;

const DIGEST_SIZE: usize = 100;
const BUFFER_SIZE: usize = 512;

#[derive(Debug, Clone)]
pub struct ApproxQuantiles {
    digest: Option<TDigest>,
    buffer: Vec<f64>,
}

impl ApproxQuantiles {
    pub fn new() -> Self {
        Self {
            digest: Some(TDigest::new_with_size(DIGEST_SIZE)),
            buffer: Vec::with_capacity(BUFFER_SIZE),
        }
    }

    pub fn clear(&mut self) {
        self.digest = Some(TDigest::new_with_size(DIGEST_SIZE));
        self.buffer.clear();
    }

    pub fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        self.digest = Some(
            self.digest
                .as_mut()
                .unwrap()
                .merge_unsorted(self.buffer.clone()),
        );

        self.buffer.clear();
    }

    pub fn add(&mut self, value: f64) {
        self.buffer.push(value);

        if self.buffer.len() == BUFFER_SIZE {
            self.flush();
        }
    }

    pub fn finalize(&mut self) {
        self.flush();
    }

    pub fn get(&self, q: f64) -> f64 {
        self.digest.as_ref().unwrap().estimate_quantile(q)
    }

    pub fn merge(&mut self, other: Self) {
        self.flush();
        self.buffer = other.buffer;
        self.digest = Some(TDigest::merge_digests(vec![
            self.digest.take().unwrap(),
            other.digest.unwrap(),
        ]));
    }
}
