pub type Curve = Vec<(f32, f32)>;

pub fn lerp_slice(slice: &[f32], t: f32) -> f32 {
    let i1 = ((slice.len() - 1) as f32 * t) as usize;
    let alpha = ((slice.len() - 1) as f32 * t) - i1 as f32;

    if i1 == (slice.len() - 1) {
        *slice.last().unwrap()
    } else {
        let v1 = slice[i1];
        let v2 = slice[i1 + 1];
        v1 + ((v2 - v1) * alpha)
    }
}

pub fn flip_slice_xy(slice: &[f32]) -> Vec<f32> {
    let mut curve = Vec::new();
    for i in 0..slice.len() {
        let x = i as f32 / (slice.len() - 1) as f32;
        let y = slice[i];
        curve.push((x, y));
    }

    let mut flipped = Vec::new();
    for i in 0..slice.len() {
        let y = i as f32 / (slice.len() - 1) as f32;
        let x = lerp_curve_at_y(&curve, y);
        flipped.push(x);
    }

    flipped
}

// Returns the y value at the given x value.
#[allow(dead_code)]
pub fn lerp_curve_at_x(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.0.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].1, // Early out.
        Err(i) => {
            if i == 0 {
                ((0.0f32, 0.0f32), curve[i])
            } else if i == curve.len() {
                (curve[i - 1], (1.0f32, 1.0f32))
            } else {
                (curve[i - 1], curve[i])
            }
        }
    };

    let alpha = (t - p1.0) / (p2.0 - p1.0);
    p1.1 + ((p2.1 - p1.1) * alpha)
}

// Returns the x value at the given y value.
#[allow(dead_code)]
pub fn lerp_curve_at_y(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.1.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].1, // Early out.
        Err(i) => {
            if i == 0 {
                ((0.0f32, 0.0f32), curve[i])
            } else if i == curve.len() {
                (curve[i - 1], (1.0f32, 1.0f32))
            } else {
                (curve[i - 1], curve[i])
            }
        }
    };

    let alpha = (t - p1.1) / (p2.1 - p1.1);
    p1.0 + ((p2.0 - p1.0) * alpha)
}
