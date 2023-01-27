use crate::config::*;

use colorbox::{chroma, matrix, matrix_compose};

/// Builds a config with just the bare basics.
pub fn make_minimal(
    reference_space_chroma: chroma::Chromaticities,
    whitepoint_adaptation_method: matrix::AdaptationMethod,
) -> OCIOConfig {
    let mut config = OCIOConfig::default();

    config.reference_space_chroma = reference_space_chroma;

    config.search_path.extend(["luts".into()]);

    config.roles.reference = Some("Linear".into());
    config.roles.aces_interchange = Some("Linear ACES".into());
    config.roles.cie_xyz_d65_interchange = Some("XYZ D65".into());

    config.roles.default = Some("Linear".into());
    config.roles.data = Some("Non-Color".into());
    config.roles.other = [
        ("scene_linear".into(), "Linear".into()),
        ("rendering".into(), "Linear".into()),
        ("compositing_linear".into(), "Linear".into()),
        ("texture_paint".into(), "Linear".into()),
        ("matte_paint".into(), "Linear".into()),
        ("color_picking".into(), "sRGB".into()),
        // compositing_log
        // color_timing
        ("default_byte".into(), "sRGB".into()),
        ("default_float".into(), "sRGB Linear".into()),
    ]
    .into();

    //---------------------------------------------------------
    // Displays

    config.displays.push(Display {
        name: "None".into(),
        views: vec![("Standard".into(), "Raw".into())],
    });
    config.active_displays.push("None".into());

    config.displays.push(Display {
        name: "sRGB".into(),
        views: vec![
            ("Standard".into(), "sRGB Gamut Clipped".into()),
            ("Filmic".into(), "sRGB Gamut Clipped Filmic".into()),
            (
                "Filmic High Contrast".into(),
                "sRGB Gamut Clipped Filmic Contrast".into(),
            ),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("sRGB".into());

    config.displays.push(Display {
        name: "Rec.709".into(),
        views: vec![
            ("Standard".into(), "Rec.709 Gamut Clipped".into()),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("Rec.709".into());

    config.displays.push(Display {
        name: "Rec.2020".into(),
        views: vec![
            ("Standard".into(), "Rec.2020 Gamut Clipped".into()),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("Rec.2020".into());

    config.displays.push(Display {
        name: "Rec.2100 PQ 10000 nits".into(),
        views: vec![
            (
                "Standard".into(),
                "Rec.2100 PQ 10000 nits Gamut Clipped".into(),
            ),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("Rec.2100 PQ 10000 nits".into());

    config.displays.push(Display {
        name: "Rec.2100 PQ 1000 nits".into(),
        views: vec![
            (
                "Standard".into(),
                "Rec.2100 PQ 1000 nits Gamut Clipped".into(),
            ),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("Rec.2100 PQ 1000 nits".into());

    config.displays.push(Display {
        name: "Rec.2100 PQ 100 nits".into(),
        views: vec![
            (
                "Standard".into(),
                "Rec.2100 PQ 100 nits Gamut Clipped".into(),
            ),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("Rec.2100 PQ 100 nits".into());

    config.displays.push(Display {
        name: "Rec.2100 HLG".into(),
        views: vec![
            ("Standard".into(), "Rec.2100 HLG Gamut Clipped".into()),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("Rec.2100 HLG".into());

    config.displays.push(Display {
        name: "DCI-P3".into(),
        views: vec![
            ("Standard".into(), "DCI-P3 Gamut Clipped".into()),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.active_displays.push("DCI-P3".into());

    config.active_views = vec![
        "Standard".into(),
        "Filmic".into(),
        "Filmic High Contrast".into(),
        "Raw".into(),
    ];

    //---------------------------------------------------------
    // Display color spaces.

    config.add_display_colorspace(
        "sRGB Gamut Clipped".into(),
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        vec![],
        Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: true,
        },
        true,
    );

    config.add_display_colorspace(
        "sRGB Gamut Clipped Filmic".into(),
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        vec![Transform::FileTransform {
            src: "omkr__tonemap_curve_normal.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: false,
        }],
        Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: true,
        },
        true,
    );

    config.add_display_colorspace(
        "sRGB Gamut Clipped Filmic Contrast".into(),
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        vec![Transform::FileTransform {
            src: "omkr__tonemap_curve_contrast.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: false,
        }],
        Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: true,
        },
        true,
    );

    config.add_display_colorspace(
        "Rec.709 Gamut Clipped".into(),
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        vec![],
        Transform::ExponentWithLinearTransform {
            gamma: 1.0 / 0.45,
            offset: 0.09929682680944,
            direction_inverse: true,
        },
        true,
    );

    config.add_display_colorspace(
        "Rec.2020 Gamut Clipped".into(),
        None,
        chroma::REC2020,
        whitepoint_adaptation_method,
        vec![],
        Transform::ExponentWithLinearTransform {
            gamma: 1.0 / 0.45,
            offset: 0.09929682680944,
            direction_inverse: true,
        },
        true,
    );

    config.generate_gamut_clipping_luts();
    for nits in [100, 1000, 10000] {
        config.colorspaces.push(ColorSpace {
            name: format!("Rec.2100 PQ {} nits Gamut Clipped", nits),
            description: "".into(),
            family: "display".into(),
            bitdepth: Some(BitDepth::F32),
            isdata: Some(false),
            from_reference: vec![
                //---------------------
                // Convert color gamut.
                Transform::MatrixTransform(matrix::to_4x4_f32(matrix_compose!(
                    matrix::rgb_to_xyz_matrix(config.reference_space_chroma),
                    matrix::xyz_chromatic_adaptation_matrix(
                        config.reference_space_chroma.w,
                        chroma::REC2020.w,
                        whitepoint_adaptation_method,
                    ),
                    matrix::xyz_to_rgb_matrix(chroma::REC2020),
                ))),
                //------------------------
                // Gamut and tone mapping.
                Transform::ToHSV,
                Transform::FileTransform {
                    src: OUTPUT_GAMUT_CLIP_LUT_FILENAME.into(),
                    interpolation: Interpolation::Linear,
                    direction_inverse: false,
                },
                Transform::FromHSV,
                //--------
                // Encode.
                Transform::RangeTransform {
                    range_in: (0.0, 1.0),
                    range_out: (0.0, nits as f64 / 10000.0),
                    clamp: true,
                },
                Transform::FileTransform {
                    src: "pq_norm_to_linear.spi1d".into(),
                    interpolation: Interpolation::Linear,
                    direction_inverse: true,
                },
            ],
            ..ColorSpace::default()
        });
    }

    config.add_display_colorspace(
        "Rec.2100 HLG Gamut Clipped".into(),
        None,
        chroma::REC2020,
        whitepoint_adaptation_method,
        vec![],
        Transform::FileTransform {
            src: "hlg_to_linear.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: true,
        },
        true,
    );

    config.add_display_colorspace(
        "DCI-P3 Gamut Clipped".into(),
        None,
        chroma::DCI_P3,
        whitepoint_adaptation_method,
        vec![],
        Transform::ExponentTransform {
            gamma: 2.6,
            direction_inverse: true,
        },
        true,
    );

    //---------------------------------------------------------
    // Input color spaces.

    config.colorspaces.push(ColorSpace {
        name: "Raw".into(),
        family: "raw".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(true),
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Non-Color".into(),
        description: "Color space used for images which contains non-color data (i,e, normal maps)"
            .into(),
        family: "raw".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(true),
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear".into(),
        family: "linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "XYZ".into(),
        family: "linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::rgb_to_xyz_matrix(reference_space_chroma),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "XYZ D65".into(),
        description: "CIE XYZ with a D65 white point".into(),
        family: "linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix_compose!(
                matrix::rgb_to_xyz_matrix(reference_space_chroma),
                matrix::xyz_chromatic_adaptation_matrix(
                    reference_space_chroma.w,
                    chroma::WHITEPOINT_D65,
                    whitepoint_adaptation_method,
                ),
            ),
        ))],
        ..ColorSpace::default()
    });

    config.add_input_colorspace(
        "Linear ACES".into(),
        Some("linear".into()),
        Some("ACES AP0 linear space".into()),
        chroma::ACES_AP0,
        whitepoint_adaptation_method,
        None,
        true,
    );

    config.add_input_colorspace(
        "sRGB Linear".into(),
        Some("linear".into()),
        Some("Linear color space with sRGB/Rec.709 gamut".into()),
        chroma::REC709,
        whitepoint_adaptation_method,
        None,
        false,
    );

    config.add_input_colorspace(
        "sRGB".into(),
        None,
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        Some(Transform::FileTransform {
            src: "srgb_to_linear.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: false,
        }),
        false,
    );

    //---------------------------------------------------------
    // Generate output files.

    config.output_files.extend([
        (
            "luts/srgb_to_linear.spi1d".into(),
            OutputFile::Lut1D(colorbox::lut::Lut1D::from_fn_1(
                65561,
                -0.125,
                4.875,
                colorbox::transfer_functions::srgb::to_linear,
            )),
        ),
        (
            "luts/pq_norm_to_linear.spi1d".into(),
            OutputFile::Lut1D(colorbox::lut::Lut1D::from_fn_1(4096, 0.0, 1.0, |n| {
                colorbox::transfer_functions::rec2100_pq::to_linear(n)
                    / colorbox::transfer_functions::rec2100_pq::LUMINANCE_MAX
            })),
        ),
        (
            "luts/hlg_to_linear.spi1d".into(),
            OutputFile::Lut1D(colorbox::lut::Lut1D::from_fn_1(
                4096,
                0.0,
                1.0,
                colorbox::transfer_functions::rec2100_hlg::to_linear,
            )),
        ),
        ("luts/omkr__tonemap_curve_normal.spi1d".into(), {
            let fixed_point = 0.18;
            let stops_range = [-16.0_f64, 8.0];
            let upper = fixed_point * stops_range[1].exp2();
            OutputFile::Lut1D(colorbox::lut::Lut1D::from_fn_1(
                1 << 16,
                0.0,
                upper as f32,
                |n| filmic_curve(n as f64, fixed_point, stops_range, 0.2, 0.0) as f32,
            ))
        }),
        ("luts/omkr__tonemap_curve_contrast.spi1d".into(), {
            let fixed_point = 0.18;
            let stops_range = [-16.0_f64, 6.0];
            let upper = fixed_point * stops_range[1].exp2();
            OutputFile::Lut1D(colorbox::lut::Lut1D::from_fn_1(
                1 << 16,
                0.0,
                upper as f32,
                |n| filmic_curve(n as f64, fixed_point, stops_range, 0.05, 0.0) as f32,
            ))
        }),
    ]);

    config
}

/// A simple filmic tonemapping curve.
///
/// The basic idea behind this is to apply an s-curve in log2 space.  In
/// practice this produces pleasing results, but it has no real basis in
/// e.g. the actual physics of film stock.
///
/// - `x`: the input value.
/// - `fixed_point`: the value of `x` that should map to itself.  For
///   example, you might set this to 0.18 (18% gray) so that colors of
///   that brightness remain the same.
/// - `stops_range`: the stops to map to [0.0, 1.0], specified relative
///   to `fixed_point`. `[-16.0, 8.0]` is a reasonable setting.  The
///   upper range value tends to determine how constrasty the filmic look
///   with smaller values producing more constrast.  The lower range also
///   has some impact, but not as much, and can typically be left around -16.
/// - `foot_sharpness`: how sharp the foot is.  Reasonable values are in [0.0, 1.0]
/// - `shoulder_sharpness`: how sharp the shoulder is.  Reasonable values are in [0.0, 1.0].
///
/// Returns the tonemapped value, always in the range [0.0, 1.0].
fn filmic_curve(
    x: f64,
    fixed_point: f64,
    stops_range: [f64; 2],
    foot_sharpness: f64,
    shoulder_sharpness: f64,
) -> f64 {
    // Map inputs in an user-friendly way, so that [0.0, 1.0] are reasonable.
    let foot_start = (0.6 - (0.6 * foot_sharpness)).sqrt(); // [0.0, 1.0] -> [~0.77, 0.0]
    let shoulder_sharpness = 3.0 + (5.0 * shoulder_sharpness * 4.0); // [0.0, 1.0] -> [3.0, 8.0]

    let mapper = |n: f64| {
        // Map to [0.0, 1.0] in log2 space, spanning `[stops_below, stops_above]` from the fixed_point.
        let a = fixed_point.log2() + stops_range[0];
        let b = fixed_point.log2() + stops_range[1];
        let lg2 = (n.log2() - a) / (b - a);
        s_curve(lg2, foot_start, 1.0, shoulder_sharpness)
    };

    // Exponent needed to map `fixed_point` back to itself.
    let exp = fixed_point.log2() / mapper(fixed_point).log2();

    mapper(x).powf(exp)
}

/// A tweakable sigmoid function that maps [0.0, 1.0] to [0.0, 1.0].
///
/// - `transition`: the value of `x` where the foot transitions to the shoulder.
/// - `foot_exp`: the exponent used for the foot part of the curve.
///   1.0 = linear, 2.0 = quadratic, etc.
/// - `shoulder_exp`: the exponent used for the shoulder part of the curve.
fn s_curve(x: f64, transition: f64, foot_exp: f64, shoulder_exp: f64) -> f64 {
    // Early-out for off-the-end values.
    if x <= 0.0 {
        return 0.0;
    } else if x >= 1.0 {
        return 1.0;
    }

    // Foot and shoulder curve functions.
    let foot = |x: f64, scale: f64| -> f64 { x.powf(foot_exp) * scale };
    let shoulder = |x: f64, scale: f64| -> f64 { 1.0 - ((1.0 - x).powf(shoulder_exp) * scale) };

    // Foot and shoulder slopes at the transition.
    let foot_slope = foot_exp * transition.powf(foot_exp - 1.0);
    let shoulder_slope = shoulder_exp * (1.0 - transition).powf(shoulder_exp - 1.0);

    // Vertical scale factors needed to make the foot and shoulder meet
    // at the transition with equal slopes.
    let s1 = shoulder_slope / foot_slope;
    let s2 = 1.0 / (1.0 + foot(transition, s1) - shoulder(transition, 1.0));

    // The full curve output.
    if x < transition {
        foot(x, s1 * s2)
    } else {
        shoulder(x, s2)
    }
    .clamp(0.0, 1.0)
}

// /// Inverse of `s_curve()`.
// fn s_curve_inv(x: f64, transition: f64, p1: f64, p2: f64) -> f64 {
//     todo!()
// }
