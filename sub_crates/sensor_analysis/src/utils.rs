pub(crate) type Curve = Vec<(f32, f32)>;

#[inline(always)]
pub fn lerp_slice(slice: &[f32], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);

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

/// Does the inverse of `lerp_slice()`.
///
/// In other words, `n == inv_lerp_slice(slice, lerp_slice(slice, n))`.
///
/// This assumes that the slice is monotonically increasing.
#[inline(always)]
pub fn inv_lerp_slice(slice: &[f32], t: f32) -> f32 {
    let (i1, i2) = match slice.binary_search_by(|v| v.partial_cmp(&t).unwrap()) {
        Ok(i) => (i - 1, i),
        Err(i) => {
            if i == 0 {
                (i, i + 1)
            } else {
                (i - 1, i)
            }
        }
    };

    let out_1 = i1 as f32 / (slice.len() - 1) as f32;
    let out_2 = i2 as f32 / (slice.len() - 1) as f32;

    let alpha = if slice[i1] == slice[i2] {
        return (out_1 + out_2) * 0.5;
    } else {
        (t - slice[i1]) / (slice[i2] - slice[i1])
    };

    out_1 + ((out_2 - out_1) * alpha)
}

// Returns the y value at the given x value.
pub(crate) fn lerp_curve_at_x(curve: &[(f32, f32)], t: f32) -> f32 {
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
pub(crate) fn lerp_curve_at_y(curve: &[(f32, f32)], t: f32) -> f32 {
    let (p1, p2) = match curve.binary_search_by(|v| v.1.partial_cmp(&t).unwrap()) {
        Ok(i) => return curve[i].0, // Early out.
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
