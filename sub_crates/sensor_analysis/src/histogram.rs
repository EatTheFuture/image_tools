#[derive(Debug, Clone)]
pub struct Histogram {
    pub total_samples: usize,
    pub buckets: Vec<usize>,
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
}
