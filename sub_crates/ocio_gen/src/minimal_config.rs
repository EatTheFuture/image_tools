use crate::{config::*, tone_map::Tonemapper};

use colorbox::{chroma, matrix};

/// Builds a config with just the bare basics.
pub fn make_minimal(
    reference_space_chroma: chroma::Chromaticities,
    whitepoint_adaptation_method: matrix::AdaptationMethod,
) -> OCIOConfig {
    // Tone mapping operators, used various places below.
    let tonemap_normal = Tonemapper::new(None, 1.0, 5.0, 1.5, 0.18, None);
    let tonemap_contrast = Tonemapper::new(None, 1.1, 10.0, 1.5, 0.18, None);

    //---------------------------------------------------------

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
            ("Tone Map".into(), "sRGB Gamut Clipped Filmic".into()),
            (
                "Tone Map Punchy".into(),
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
        "Tone Map".into(),
        "Tone Map Punchy".into(),
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
        tonemap_normal.tone_map_transforms(
            "omkr__tonemap_curve_normal_inv.spi1d",
            "omkr__tonemap_chroma_normal.cube",
        ),
        Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: true,
        },
        false,
    );

    config.add_display_colorspace(
        "sRGB Gamut Clipped Filmic Contrast".into(),
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        tonemap_contrast.tone_map_transforms(
            "omkr__tonemap_curve_contrast_inv.spi1d",
            "omkr__tonemap_chroma_contrast.cube",
        ),
        Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: true,
        },
        false,
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
                Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                    matrix::rgb_to_xyz_matrix(config.reference_space_chroma),
                    matrix::xyz_chromatic_adaptation_matrix(
                        config.reference_space_chroma.w,
                        chroma::REC2020.w,
                        whitepoint_adaptation_method,
                    ),
                    matrix::xyz_to_rgb_matrix(chroma::REC2020),
                ]))),
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
        Transform::ExponentTransform(2.6, 2.6, 2.6, 1.0).invert(),
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
            matrix::compose(&[
                matrix::rgb_to_xyz_matrix(reference_space_chroma),
                matrix::xyz_chromatic_adaptation_matrix(
                    reference_space_chroma.w,
                    chroma::WHITEPOINT_D65,
                    whitepoint_adaptation_method,
                ),
            ]),
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

    // Transfer function curves.
    config.output_files.extend([
        //---------------------------
        // Transfer function curves.
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
    ]);

    // Tone mapping LUTs.
    {
        let (tone_1d_normal, tone_3d_normal) = tonemap_normal.generate_luts();
        let (tone_1d_contrast, tone_3d_contrast) = tonemap_contrast.generate_luts();
        config.output_files.extend([
            (
                "luts/omkr__tonemap_curve_normal_inv.spi1d".into(),
                OutputFile::Lut1D(tone_1d_normal),
            ),
            (
                "luts/omkr__tonemap_chroma_normal.cube".into(),
                OutputFile::Lut3D(tone_3d_normal),
            ),
            (
                "luts/omkr__tonemap_curve_contrast_inv.spi1d".into(),
                OutputFile::Lut1D(tone_1d_contrast),
            ),
            (
                "luts/omkr__tonemap_chroma_contrast.cube".into(),
                OutputFile::Lut3D(tone_3d_contrast),
            ),
        ]);
    }

    config
}
