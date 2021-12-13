#[derive(Debug, Clone)]
pub struct Histogram {
    pub total_samples: usize,
    pub buckets: Vec<usize>,
}

impl Default for Histogram {
    fn default() -> Histogram {
        Histogram {
            total_samples: 0,
            buckets: Vec::new(),
        }
    }
}

impl Histogram {
    /// Builds a histogram from any iterator yielding items than can be converted to `usize`.
    ///
    /// The values yielded by the iterator are used to directly index
    /// into the histogram buckets, so `bucket_count` should be large
    /// enough to accommodate any yielded values.
    pub fn from_iter<T, Itr>(values: Itr, bucket_count: usize) -> Self
    where
        T: Into<usize>,
        Itr: std::iter::Iterator<Item = T>,
    {
        let mut hist = Histogram {
            total_samples: 0,
            buckets: vec![0; bucket_count],
        };
        for v in values {
            hist.total_samples += 1;
            hist.buckets[v.into()] += 1;
        }
        hist
    }

    pub fn sum_under(&self, bucket_index: usize) -> usize {
        self.buckets[0..bucket_index].iter().sum()
    }

    pub fn find_sum(&self, sum: usize) -> usize {
        let mut cur_sum = 0;
        for (i, b) in self.buckets.iter().enumerate() {
            cur_sum += *b;
            if cur_sum >= sum {
                return i;
            }
        }

        self.buckets.len() - 1
    }

    pub fn find_sum_lerp(&self, sum: usize) -> f32 {
        let mut prev_sum;
        let mut cur_sum = 0;
        for (i, b) in self.buckets.iter().enumerate() {
            prev_sum = cur_sum;
            cur_sum += *b;
            if cur_sum >= sum {
                if i == 0 {
                    return 0.0;
                } else {
                    let a = (sum - prev_sum) as f32 / (cur_sum - prev_sum) as f32;
                    return ((i - 1) as f32 * (1.0 - a)) + (i as f32 * a);
                }
            }
        }

        (self.buckets.len() - 1) as f32
    }
}
