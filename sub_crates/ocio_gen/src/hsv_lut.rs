use colorbox::lut::Lut3D;

/// Note: convert should take and produce *RGB values*, not HSV.
pub fn make_hsv_lut<F>(res: usize, val_range: (f32, f32), convert: F) -> Lut3D
where
    F: Fn((f32, f32, f32)) -> (f32, f32, f32),
{
    assert!(val_range.0 < val_range.1);

    const HUE_RANGE: (f32, f32) = (0.0, 1.0);
    const SAT_RANGE: (f32, f32) = (0.0, 2.0);
    let val_delta = val_range.1 - val_range.0;

    let mut lut = Lut3D {
        range: [HUE_RANGE, SAT_RANGE, val_range],
        resolution: [res; 3],
        tables: {
            let mut tables = vec![Vec::new(), Vec::new(), Vec::new()];
            for val_i in 0..res {
                let val = val_range.0 + (val_delta / (res - 1) as f32 * val_i as f32);
                for sat_i in 0..res {
                    let sat = SAT_RANGE.1 / (res - 1) as f32 * sat_i as f32;
                    for hue_i in 0..res {
                        let hue = HUE_RANGE.1 / (res - 1) as f32 * hue_i as f32;

                        // Compute the mapping.
                        let hsv_in = (hue, sat, val);
                        let mut hsv_out = to_hsv(convert(from_hsv(hsv_in)));

                        // Make sure that hue doesn't wrap around the long way.
                        // Note: this assumes a relatively sane mapping, where
                        // e.g. blues aren't mapped to yellows.
                        let hue_delta = hsv_out.0 - hsv_in.0;
                        if hue_delta > 0.5 {
                            hsv_out.0 -= 1.0;
                        } else if hue_delta <= -0.5 {
                            hsv_out.0 += 1.0;
                        }

                        tables[0].push(hsv_out.0);
                        tables[1].push(hsv_out.1);
                        tables[2].push(hsv_out.2);
                    }
                }
            }
            tables
        },
    };

    //-----------------------------------------------------------------
    // Clean up weird things that can happen due to the nature of HSV.

    let idx = |mut hi: usize, mut si: usize, mut vi: usize| {
        hi = hi.min(res - 1);
        si = si.min(res - 1);
        vi = vi.min(res - 1);

        (vi * res * res) + (si * res) + hi
    };

    let val_thresh = 0.000_001;
    let sat_thresh = 0.000_01;

    // Sweep down and fix things.
    for val_i in (0..res).rev() {
        for sat_i in (0..res).rev() {
            for hue_i in 0..res {
                let i = idx(hue_i, sat_i, val_i);

                // If value is zero, copy hue and sat from next value up.
                if lut.tables[2][i].abs() < val_thresh {
                    let i2 = idx(hue_i, sat_i, val_i + 1);
                    lut.tables[0][i] = lut.tables[0][i2];
                    lut.tables[1][i] = lut.tables[1][i2];
                }

                // If saturation is zero, copy from hue from next saturation up.
                if lut.tables[1][i] < sat_thresh {
                    let i2 = idx(hue_i, sat_i + 1, val_i);
                    lut.tables[0][i] = lut.tables[0][i2];
                }
            }
        }
    }

    // Sweep up and fix things.
    for val_i in 0..res {
        for sat_i in 0..res {
            for hue_i in 0..res {
                let i = idx(hue_i, sat_i, val_i);

                // If value is zero, copy hue and sat from next value down.
                if lut.tables[2][i].abs() < val_thresh {
                    let i2 = idx(hue_i, sat_i, val_i.saturating_sub(1));
                    lut.tables[0][i] = lut.tables[0][i2];
                    lut.tables[1][i] = lut.tables[1][i2];
                }

                // If saturation is zero, copy from hue from next saturation down.
                if lut.tables[1][i] < sat_thresh {
                    let i2 = if sat_i > 0 {
                        idx(hue_i, sat_i.saturating_sub(1), val_i)
                    } else {
                        idx(hue_i, sat_i, val_i.saturating_sub(1))
                    };
                    lut.tables[0][i] = lut.tables[0][i2];
                }
            }
        }
    }

    lut
}

/// OCIO-compatible RGB -> HSV conversion.
fn to_hsv(rgb: (f32, f32, f32)) -> (f32, f32, f32) {
    let (red, grn, blu) = rgb;

    let rgb_min = red.min(grn.min(blu));
    let rgb_max = red.max(grn.max(blu));
    let delta = rgb_max - rgb_min;

    let mut val = rgb_max;
    let mut sat = 0.0f32;
    let mut hue = 0.0f32;

    if delta != 0.0 {
        // Sat
        if rgb_max != 0.0 {
            sat = delta / rgb_max;
        }

        // Hue
        if red == rgb_max {
            hue = (grn - blu) / delta;
        } else if grn == rgb_max {
            hue = 2.0 + (blu - red) / delta;
        } else {
            hue = 4.0 + (red - grn) / delta;
        }

        if hue < 0.0 {
            hue += 6.0;
        }

        hue *= 1.0 / 6.0;
    }

    // Handle extended range inputs.
    if rgb_min < 0.0 {
        val += rgb_min;
    }

    if -rgb_min > rgb_max {
        sat = (rgb_max - rgb_min) / -rgb_min;
    }

    (hue, sat, val)
}

/// OCIO-compatible HSV -> RGB conversion.
fn from_hsv(hsv: (f32, f32, f32)) -> (f32, f32, f32) {
    const MAX_SAT: f32 = 1.999;

    let hue = (hsv.0 - hsv.0.floor()) * 6.0;
    let sat = hsv.1.clamp(0.0, MAX_SAT);
    let val = hsv.2;

    let red = ((hue - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let grn = (2.0 - (hue - 2.0).abs()).clamp(0.0, 1.0);
    let blu = (2.0 - (hue - 4.0).abs()).clamp(0.0, 1.0);

    let mut rgb_max = val;
    let mut rgb_min = val * (1.0 - sat);

    // Handle extended range inputs.
    if sat > 1.0 {
        rgb_min = val * (1.0 - sat) / (2.0 - sat);
        rgb_max = val - rgb_min;
    }
    if val < 0.0 {
        rgb_min = val / (2.0 - sat);
        rgb_max = val - rgb_min;
    }

    let delta = rgb_max - rgb_min;

    (
        red * delta + rgb_min,
        grn * delta + rgb_min,
        blu * delta + rgb_min,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const RGB_COLORS: &[(f32, f32, f32)] = &[
        (0.0, 0.0, 0.0),
        (0.01, 0.6, 0.2),
        (0.01, 0.6, -0.2),
        (0.01, 0.6, -20.0),
        (0.01, -0.6, 0.2),
        (-0.01, 0.6, 0.2),
        (-0.01, -0.6, -0.2),
    ];

    #[test]
    fn hsv_round_trip() {
        for rgb in RGB_COLORS.iter().copied() {
            let rgb2 = from_hsv(to_hsv(rgb));
            assert!((rgb.0 - rgb2.0).abs() < 0.00001);
            assert!((rgb.1 - rgb2.1).abs() < 0.00001);
            assert!((rgb.2 - rgb2.2).abs() < 0.00001);
        }
    }
}
