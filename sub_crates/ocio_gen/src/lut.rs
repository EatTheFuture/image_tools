#[derive(Debug, Clone)]
pub struct Lut1D {
    pub ranges: Vec<(f32, f32)>,
    pub tables: Vec<Vec<f32>>,
}

impl Default for Lut1D {
    fn default() -> Lut1D {
        Lut1D {
            ranges: Vec::new(),
            tables: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lut3D {
    pub ranges: [(f32, f32); 3],
    pub resolution: [usize; 3],
    pub table: Vec<[f32; 3]>,
}

impl Default for Lut3D {
    fn default() -> Lut3D {
        Lut3D {
            ranges: [(0.0, 1.0); 3],
            resolution: [0; 3],
            table: Vec::new(),
        }
    }
}
