#[derive(Debug, Clone)]
pub struct Histogram {
    pub total_samples: usize,
    pub buckets: Vec<usize>,
}

impl Histogram {
    pub fn from_u8s<Itr>(values: Itr) -> Self
    where
        Itr: std::iter::Iterator<Item = u8>,
    {
        let mut hist = Histogram {
            total_samples: 0,
            buckets: vec![0; 1 << 8],
        };
        for v in values {
            hist.total_samples += 1;
            hist.buckets[v as usize] += 1;
        }
        hist
    }

    pub fn from_u16s<Itr>(values: Itr) -> Self
    where
        Itr: std::iter::Iterator<Item = u16>,
    {
        let mut hist = Histogram {
            total_samples: 0,
            buckets: vec![0; 1 << 16],
        };
        for v in values {
            hist.total_samples += 1;
            hist.buckets[v as usize] += 1;
        }
        hist
    }
}
