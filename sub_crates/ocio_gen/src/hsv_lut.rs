use colorbox::lut::Lut3D;

/// Note: `convert` receives and should produce HSV values according
/// to the OCIO implementation.
pub fn make_hsv_lut<F>(res: usize, val_range: (f64, f64), max_sat: f64, convert: F) -> Lut3D
where
    F: Fn((f64, f64, f64)) -> (f64, f64, f64),
{
    assert!(val_range.0 < val_range.1);

    const HUE_RANGE: (f64, f64) = (0.0, 1.0);
    let sat_range = (0.0, max_sat);
    let val_delta = val_range.1 - val_range.0;

    let mut lut = Lut3D {
        range: [
            (HUE_RANGE.0 as f32, HUE_RANGE.1 as f32),
            (sat_range.0 as f32, sat_range.1 as f32),
            (val_range.0 as f32, val_range.1 as f32),
        ],
        resolution: [res; 3],
        tables: {
            let mut tables = vec![Vec::new(), Vec::new(), Vec::new()];
            for val_i in 0..res {
                let val = val_range.0 + (val_delta / (res - 1) as f64 * val_i as f64);
                for sat_i in 0..res {
                    let sat = sat_range.1 / (res - 1) as f64 * sat_i as f64;
                    for hue_i in 0..res {
                        let hue = HUE_RANGE.1 / (res - 1) as f64 * hue_i as f64;

                        // Compute the mapping.
                        let hsv_in = (hue, sat, val);
                        let mut hsv_out = convert(hsv_in);

                        // Make sure that hue doesn't wrap around the long way.
                        // Note: this assumes a relatively sane mapping, where
                        // e.g. blues aren't mapped to yellows.
                        while (hsv_out.0 - hsv_in.0) > 0.5 {
                            hsv_out.0 -= 1.0;
                        }
                        while (hsv_out.0 - hsv_in.0) <= -0.5 {
                            hsv_out.0 += 1.0;
                        }

                        tables[0].push(hsv_out.0 as f32);
                        tables[1].push(hsv_out.1 as f32);
                        tables[2].push(hsv_out.2 as f32);
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
pub fn to_hsv(rgb: (f64, f64, f64)) -> (f64, f64, f64) {
    let hsv = colorbox::transforms::ocio::rgb_to_hsv([rgb.0, rgb.1, rgb.2]);
    (hsv[0], hsv[1], hsv[2])
}

/// OCIO-compatible HSV -> RGB conversion.
pub fn from_hsv(hsv: (f64, f64, f64)) -> (f64, f64, f64) {
    let rgb = colorbox::transforms::ocio::hsv_to_rgb([hsv.0, hsv.1, hsv.2]);
    (rgb[0], rgb[1], rgb[2])
}
