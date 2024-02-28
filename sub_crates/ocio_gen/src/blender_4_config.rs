use crate::config::*;

use colorbox::{chroma, matrix};

pub const REFERENCE_SPACE_CHROMA: chroma::Chromaticities = chroma::XYZ;

/// Builds a config that matches Blender 4.0's default.
pub fn make_blender_4_0() -> OCIOConfig {
    let e_to_d65 = matrix::xyz_chromatic_adaptation_matrix(
        chroma::illuminant::E,
        chroma::illuminant::D65,
        matrix::AdaptationMethod::Bradford,
    );
    let d65_to_e = matrix::xyz_chromatic_adaptation_matrix(
        chroma::illuminant::D65,
        chroma::illuminant::E,
        matrix::AdaptationMethod::Bradford,
    );

    //----

    let mut config = OCIOConfig::default();

    config.reference_space_chroma = REFERENCE_SPACE_CHROMA;

    config.name = Some("Blender 4.0 (customized)".into());
    config.description = Some("Customized variant of the Blender 4.0 configuration.".into());
    config.search_path.extend(["luts".into(), "filmic".into()]);

    config.roles.reference = Some("Linear CIE-XYZ E".into());
    config.roles.aces_interchange = Some("ACES2065-1".into());
    config.roles.cie_xyz_d65_interchange = Some("Linear CIE-XYZ D65".into());
    config.roles.default = Some("Linear Rec.709".into());
    config.roles.data = Some("Non-Color".into());
    config.roles.other = [
        ("scene_linear".into(), "Linear Rec.709".into()),
        ("rendering".into(), "Linear Rec.709".into()),
        ("default_byte".into(), "sRGB".into()),
        ("default_float".into(), "Linear Rec.709".into()),
        ("default_sequencer".into(), "sRGB".into()),
        ("color_picking".into(), "sRGB".into()),
        ("color_timing".into(), "AgX Log".into()),
        ("compositing_log".into(), "AgX Log".into()),
        ("matte_paint".into(), "Linear Rec.709".into()),
        ("texture_paint".into(), "Linear Rec.709".into()),
    ]
    .into();

    config.displays.push(Display {
        name: "sRGB".into(),
        views: vec![
            ("Standard".into(), "sRGB".into()),
            // ("AgX".into(), "AgX Base sRGB".into()),
            ("Filmic".into(), "Filmic sRGB".into()),
            ("Filmic Log".into(), "Filmic Log".into()),
            // ("False Color".into(), "AgX False Color Rec.709".into()),
            ("Raw".into(), "Non-Color".into()),
        ],
    });
    config.displays.push(Display {
        name: "Display P3".into(),
        views: vec![
            ("Standard".into(), "Display P3".into()),
            // ("AgX".into(), "AgX Base Display P3".into()),
            // ("False Color".into(), "AgX False Color P3".into()),
            ("Raw".into(), "Non-Color".into()),
        ],
    });
    config.displays.push(Display {
        name: "Rec.1886".into(),
        views: vec![
            ("Standard".into(), "Rec.1886".into()),
            // ("AgX".into(), "AgX Base Rec.1886".into()),
            // ("False Color".into(), "AgX False Color Rec.709".into()),
            ("Raw".into(), "Non-Color".into()),
        ],
    });
    config.displays.push(Display {
        name: "Rec.2020".into(),
        views: vec![
            ("Standard".into(), "Rec.2020".into()),
            // ("AgX".into(), "AgX Base Rec.2020".into()),
            // ("False Color".into(), "AgX False Color Rec.2020".into()),
            ("Raw".into(), "Non-Color".into()),
        ],
    });

    config.active_displays = vec![
        "sRGB".into(),
        "Display P3".into(),
        "Rec.1886".into(),
        "Rec.2020".into(),
    ];
    config.active_views = vec![
        "Standard".into(),
        // "AgX".into(),
        "Filmic".into(),
        "Filmic Log".into(),
        "False Color".into(),
        "Raw".into(),
    ];
    config.inactive_colorspaces = vec![
        "Luminance Compensation Rec.2020".into(),
        "Luminance Compensation sRGB".into(),
        "Luminance Compensation P3".into(),
        // "AgX False Color Rec.709".into(),
        // "AgX False Color P3".into(),
        // "AgX False Color Rec.1886".into(),
        // "AgX False Color Rec.2020".into(),
    ];

    // Filmic looks.
    for (name, path_a, path_b) in [
        (
            "Very High Contrast",
            "filmic_to_1.20_1-00.spi1d",
            "filmic_to_0-70_1-03.spi1d",
        ),
        (
            "High Contrast",
            "filmic_to_0.99_1-0075.spi1d",
            "filmic_to_0-70_1-03.spi1d",
        ),
        (
            "Medium High Contrast",
            "filmic_to_0-85_1-011.spi1d",
            "filmic_to_0-70_1-03.spi1d",
        ),
        ("Medium Contrast", "", ""),
        (
            "Medium Low Contrast",
            "filmic_to_0-60_1-04.spi1d",
            "filmic_to_0-70_1-03.spi1d",
        ),
        (
            "Low Contrast",
            "filmic_to_0-48_1-09.spi1d",
            "filmic_to_0-70_1-03.spi1d",
        ),
        (
            "Very Low Contrast",
            "filmic_to_0-35_1-30.spi1d",
            "filmic_to_0-70_1-03.spi1d",
        ),
    ] {
        config.looks.push(Look {
            name: name.into(),
            description: "".into(),
            process_space: "Filmic Log".into(),
            transform: if path_a.is_empty() && path_b.is_empty() {
                Vec::new()
            } else {
                vec![
                    Transform::FileTransform {
                        src: path_a.into(),
                        interpolation: Interpolation::Linear,
                        direction_inverse: false,
                    },
                    Transform::FileTransform {
                        src: path_b.into(),
                        interpolation: Interpolation::Linear,
                        direction_inverse: true,
                    },
                ]
            },
            inverse_transform: Vec::new(),
        });
    }

    // AgX looks.
    config.looks.push(Look {
        name: "AgX - Punchy".into(),
        description: "A darkening punchy look".into(),
        process_space: "AgX Log".into(),
        transform: vec![
            Transform::GradingToneTransform {
                style: GradingStyle::Log,
                blacks: None,
                shadows: Some(Tone {
                    rgb: [0.2, 0.2, 0.2],
                    master: 0.35,
                    start_center: 0.4,
                    width_pivot: 0.1,
                }),
                midtones: None,
                highlights: None,
                whites: None,
                s_contrast: None,
                direction_inverse: false,
            },
            Transform::CDLTransform {
                power: [1.0912; 3],
                direction_inverse: false,
            },
        ],
        inverse_transform: Vec::new(),
    });
    config.looks.push(Look {
        name: "AgX - Greyscale".into(),
        description: "A Black and White Look".into(),
        process_space: "AgX Log".into(),
        transform: vec![
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: true,
            },
            Transform::MatrixTransform([
                0.2658180370250449,
                0.59846986045365,
                0.1357121025213052,
                0.0,
                0.2658180370250449,
                0.59846986045365,
                0.1357121025213052,
                0.0,
                0.2658180370250449,
                0.59846986045365,
                0.1357121025213052,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ]),
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: false,
            },
        ],
        inverse_transform: Vec::new(),
    });
    for (name, description, transform) in [
        (
            "AgX - Very High Contrast",
            "A Very High Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.57; 3],
                saturation: 0.9,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "AgX - High Contrast",
            "A High Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.4; 3],
                saturation: 0.95,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "AgX - Medium High Contrast",
            "A Medium High Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.2; 3],
                saturation: 1.0,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "AgX - Base Contrast",
            "A Base Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.0; 3],
                saturation: 1.0,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "AgX - Medium Low Contrast",
            "A Medium Low Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [0.9; 3],
                saturation: 1.05,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "AgX - Low Contrast",
            "A Low Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [0.8; 3],
                saturation: 1.1,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "AgX - Very Low Contrast",
            "A Very Low Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [0.7; 3],
                saturation: 1.15,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
    ] {
        config.looks.push(Look {
            name: name.into(),
            description: description.into(),
            process_space: "AgX Log".into(),
            transform: vec![transform],
            inverse_transform: Vec::new(),
        });
    }

    // False Color looks.
    config.looks.push(Look {
        name: "False Color - Punchy".into(),
        description: "A darkening punchy look".into(),
        process_space: "AgX Log".into(),
        transform: vec![
            Transform::GradingToneTransform {
                style: GradingStyle::Log,
                blacks: None,
                shadows: Some(Tone {
                    rgb: [0.2, 0.2, 0.2],
                    master: 0.35,
                    start_center: 0.4,
                    width_pivot: 0.1,
                }),
                midtones: None,
                highlights: None,
                whites: None,
                s_contrast: None,
                direction_inverse: false,
            },
            Transform::CDLTransform {
                power: [1.0912; 3],
                direction_inverse: false,
            },
        ],
        inverse_transform: Vec::new(),
    });
    config.looks.push(Look {
        name: "False Color - Greyscale".into(),
        description: "A Black and White Look".into(),
        process_space: "AgX Log".into(),
        transform: vec![
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: true,
            },
            Transform::MatrixTransform([
                0.2658180370250449,
                0.59846986045365,
                0.1357121025213052,
                0.0,
                0.2658180370250449,
                0.59846986045365,
                0.1357121025213052,
                0.0,
                0.2658180370250449,
                0.59846986045365,
                0.1357121025213052,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ]),
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: false,
            },
        ],
        inverse_transform: Vec::new(),
    });
    for (name, description, transform) in [
        (
            "False Color - Very High Contrast",
            "A Very High Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.57; 3],
                saturation: 0.9,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "False Color - High Contrast",
            "A High Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.4; 3],
                saturation: 0.95,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "False Color - Medium High Contrast",
            "A Medium High Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.2; 3],
                saturation: 1.0,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "False Color - Base Contrast",
            "A Base Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [1.0; 3],
                saturation: 1.0,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "False Color - Medium Low Contrast",
            "A Medium Low Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [0.9; 3],
                saturation: 1.05,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "False Color - Low Contrast",
            "A Low Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [0.8; 3],
                saturation: 1.1,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
        (
            "False Color - Very Low Contrast",
            "A Very Low Contrast Look",
            Transform::GradingPrimaryTransform {
                style: GradingStyle::Log,
                contrast: [0.7; 3],
                saturation: 1.15,
                pivot_contrast: -0.2,
                direction_inverse: false,
            },
        ),
    ] {
        config.looks.push(Look {
            name: name.into(),
            description: description.into(),
            process_space: "AgX Log".into(),
            transform: vec![transform],
            inverse_transform: Vec::new(),
        });
    }

    //---------------------------------------------------------
    // Input color spaces.

    config.colorspaces.push(ColorSpace {
        name: "Linear CIE-XYZ E".into(),
        description: "1931 CIE XYZ standard with assumed illuminant E white point".into(),
        aliases: vec![
            "\"FilmLight: Linear - XYZ\"".into(),
            "Linear CIE-XYZ I-E".into(),
        ],
        family: "Chromaticity".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear CIE-XYZ D65".into(),
        description: "1931 CIE XYZ with adapted illuminant D65 white point".into(),
        aliases: vec![
            "cie_xyz_d65".into(),
            "CIE-XYZ-D65".into(),
            "Linear CIE-XYZ I-D65".into(),
        ],
        family: "Chromaticity".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(e_to_d65))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear Rec.709".into(),
        description: "Linear BT.709 with illuminant D65 white point".into(),
        aliases: vec![
            "Linear".into(),
            "Linear BT.709".into(),
            "Linear BT.709 I-D65".into(),
            "Linear Tristimulus".into(),
            "linrec709".into(),
            "Utility - Linear - sRGB".into(),
            "Utility - Linear - Rec.709".into(),
            "lin_srgb".into(),
            "Linear Rec.709 (sRGB)".into(),
            "lin_rec709_srgb".into(),
            "lin_rec709".into(),
            "lin_srgb".into(),
            "\"CGI: Linear - Rec.709\"".into(),
        ],
        family: "Linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[e_to_d65, matrix::xyz_to_rgb_matrix(chroma::REC709)]),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear DCI-P3 D65".into(),
        description: "Linear DCI-P3 with illuminant D65 white point".into(),
        aliases: vec![
            "Linear DCI-P3 I-D65".into(),
            "Linear P3-D65".into(),
            "lin_p3d65".into(),
            "Utility - Linear - P3-D65".into(),
            "Apple DCI-P3 D65".into(),
        ],
        family: "Linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[e_to_d65, matrix::xyz_to_rgb_matrix(chroma::DISPLAY_P3)]),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear Rec.2020".into(),
        description: "Linear BT.2020 with illuminant D65 white point".into(),
        aliases: vec![
            "Linear BT.2020 I-D65".into(),
            "Linear BT.2020".into(),
            "lin_rec2020".into(),
            "Utility - Linear - Rec.2020".into(),
        ],
        family: "Linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[e_to_d65, matrix::xyz_to_rgb_matrix(chroma::REC2020)]),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "ACES2065-1".into(),
        description: "Linear AP0 with ACES white point".into(),
        aliases: vec![
            "Linear ACES".into(),
            "aces2065_1".into(),
            "ACES - ACES2065-1".into(),
            "lin_ap0".into(),
            "\"ACES: Linear - AP0\"".into(),
        ],
        family: "Linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[
                matrix::xyz_chromatic_adaptation_matrix(
                    chroma::illuminant::E,
                    chroma::ACES_AP0.w,
                    matrix::AdaptationMethod::Bradford,
                ),
                matrix::xyz_to_rgb_matrix(chroma::ACES_AP0),
            ]),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "ACEScg".into(),
        description: "Linear AP1 with ACES white point".into(),
        aliases: vec![
            "Linear ACEScg".into(),
            "lin_ap1".into(),
            "ACES - ACEScg".into(),
            "\"ACEScg: Linear - AP1\"".into(),
        ],
        family: "Linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[
                matrix::xyz_chromatic_adaptation_matrix(
                    chroma::illuminant::E,
                    chroma::ACES_AP1.w,
                    matrix::AdaptationMethod::Bradford,
                ),
                matrix::xyz_to_rgb_matrix(chroma::ACES_AP1),
            ]),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear FilmLight E-Gamut".into(),
        description: "Linear E-Gamut with illuminant D65 white point".into(),
        aliases: vec![
            "Linear E-Gamut I-D65".into(),
            "\"FilmLight: Linear - E-Gamut\"".into(),
        ],
        family: "Linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[
                matrix::xyz_chromatic_adaptation_matrix(
                    chroma::illuminant::E,
                    chroma::E_GAMUT.w,
                    matrix::AdaptationMethod::Bradford,
                ),
                matrix::xyz_to_rgb_matrix(chroma::E_GAMUT),
            ]),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Non-Color".into(),
        description:
            "Generic data that is not color, will not apply any color transform (e.g. normal maps)"
                .into(),
        aliases: vec![
            "Generic Data".into(),
            "Non-Colour Data".into(),
            "Raw".into(),
            "Utility - Raw".into(),
        ],
        family: "Data".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(true),
        ..ColorSpace::default()
    });

    //---------------------------------------------------------
    // Display color spaces.

    config.colorspaces.push(ColorSpace {
        name: "sRGB".into(),
        description: "sRGB IEC 61966-2-1 compound (piece-wise) encoding".into(),
        aliases: vec![
            "sRGB 2.2".into(),
            "sRGB I-D65".into(),
            "srgb_display".into(),
            "sRGB - Display".into(),
            "g22_rec709".into(),
            "Utility - Gamma 2.2 - Rec.709 - Texture".into(),
            "Utility - sRGB - Texture".into(),
            "sRGB - Texture".into(),
            "srgb_tx".into(),
            "srgb_texture".into(),
            "Input - Generic - sRGB - Texture".into(),
            "\"sRGB Display: 2.2 Gamma - Rec.709\"".into(),
        ],
        family: "Display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                e_to_d65,
                matrix::xyz_to_rgb_matrix(chroma::REC709),
            ]))),
            Transform::ExponentWithLinearTransform {
                gamma: 2.4,
                offset: 0.055,
                direction_inverse: true,
            },
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Display P3".into(),
        description: "Apple's Display P3 with sRGB compound (piece-wise) encoding transfer function, common on Mac devices".into(),
        aliases: vec![
            "Display P3 2.2".into(),
            "Display P3 I-D65".into(),
            "P3-D65 - Display".into(),
            "p3_d65_display".into(),
            "p3d65_display".into(),
            "AppleP3 sRGB OETF".into(),
        ],
        family: "Display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                e_to_d65,
                matrix::xyz_to_rgb_matrix(chroma::DISPLAY_P3),
            ]))),
            Transform::ExponentWithLinearTransform {
                gamma: 2.4,
                offset: 0.055,
                direction_inverse: true,
            },
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Rec.1886".into(),
        description: "BT.1886 2.4 Exponent EOTF Display, commonly used for TVs".into(),
        aliases: vec![
            "BT.1886".into(),
            "BT.1886 2.4".into(),
            "BT.1886 EOTF".into(),
            "BT.1886 I-D65".into(),
            "Rec.1886 / Rec.709 Video - Display".into(),
            "rec1886_rec709_video_display".into(),
            "Rec.1886 Rec.709 - Display".into(),
            "rec1886_rec709_display".into(),
            "\"Rec1886: 2.4 Gamma - Rec.709\"".into(),
        ],
        family: "Display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                e_to_d65,
                matrix::xyz_to_rgb_matrix(chroma::REC709),
            ]))),
            Transform::ExponentTransform(1.0 / 2.4, 1.0 / 2.4, 1.0 / 2.4, 1.0),
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Rec.2020".into(),
        description: "BT.2020 2.4 Exponent EOTF Display".into(),
        aliases: vec![
            "BT.2020".into(),
            "BT.2020 2.4".into(),
            "BT.2020 I-D65".into(),
            "Rec.1886 / Rec.2020 Video - Display".into(),
            "rec1886_rec2020_video_display".into(),
            "Rec.1886 Rec.2020 - Display".into(),
            "rec1886_rec2020_display".into(),
            "\"Rec1886: 2.4 Gamma - Rec.2020\"".into(),
        ],
        family: "Display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                e_to_d65,
                matrix::xyz_to_rgb_matrix(chroma::REC2020),
            ]))),
            Transform::ExponentTransform(1.0 / 2.4, 1.0 / 2.4, 1.0 / 2.4, 1.0),
        ],
        ..ColorSpace::default()
    });

    //---------------------------------------------------------
    // Tone mapping color spaces.

    config.colorspaces.push(ColorSpace {
        name: "Filmic Log".into(),
        description:
            "Log based filmic shaper with 16.5 stops of latitude, and 25 stops of dynamic range"
                .into(),
        family: "Log Encodings".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                e_to_d65,
                matrix::xyz_to_rgb_matrix(chroma::REC709),
            ]))),
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.473931188, 12.526068812],
                direction_inverse: false,
            },
            Transform::FileTransform {
                src: "filmic_desat_33.cube".into(),
                interpolation: Interpolation::Tetrahedral,
                direction_inverse: false,
            },
            Transform::AllocationTransform {
                allocation: Allocation::Uniform,
                vars: vec![0.0, 0.66],
                direction_inverse: false,
            },
        ],
        to_reference: vec![
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.473931188, 4.026068812],
                direction_inverse: true,
            },
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::compose(&[
                matrix::rgb_to_xyz_matrix(chroma::REC709),
                d65_to_e,
            ]))),
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Filmic sRGB".into(),
        description: "sRGB display space with Filmic view transform".into(),
        family: "Filmic".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {
                src: "Linear CIE-XYZ E".into(),
                dst: "Filmic Log".into(),
            },
            Transform::FileTransform {
                src: "filmic_to_0-70_1-03.spi1d".into(),
                interpolation: Interpolation::Linear,
                direction_inverse: false,
            },
        ],
        ..ColorSpace::default()
    });

    // TODO: do these properly.
    config.colorspaces.push(ColorSpace {
        name: "False Color".into(), // TODO: replace with the AgX variants.
        description: "Filmic false color view transform".into(),
        family: "display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {
                src: "Linear".into(),
                dst: "Filmic Log".into(),
            },
            Transform::MatrixTransform([
                0.2126729, 0.7151521, 0.0721750, 0.0, 0.2126729, 0.7151521, 0.0721750, 0.0,
                0.2126729, 0.7151521, 0.0721750, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]),
            // TODO:
            // Transform::FileTransform {
            //     src: "filmic_false_color.spi3d".into(),
            //     interpolation: Interpolation::Best,
            //     direction_inverse: false,
            // },
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Luminance Compensation Rec.2020".into(),
        description:
            "Offset the negative values in BT.2020 and compensate for luminance, ensuring there is no negative values in Rec.2020"
                .into(),
        family: "Utilities".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {src: "Linear CIE-XYZ E".into(), dst: "Linear FilmLight E-Gamut".into()},
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: false,
            },
            // TODO:
            // Transform::FileTransform {
            //     src: "luminance_compensation_bt2020.cube".into(),
            //     interpolation: Interpolation::Tetrahedral,
            //     direction_inverse: false,
            // },
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: true,
            },
            Transform::ColorSpaceTransform {src: "Linear FilmLight E-Gamut".into(), dst: "Linear Rec.2020".into()},
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Luminance Compensation sRGB".into(),
        description:
            "Offset the negative values in BT.709 and compensate for luminance, ensuring there is no negative values in Rec.709"
                .into(),
        family: "Utilities".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {src: "Linear CIE-XYZ E".into(), dst: "Linear FilmLight E-Gamut".into()},
            Transform::MatrixTransform(matrix::to_4x4_f32([
                [0.960599732262383, 0.0196075412762159, 0.019792726461401],
                [0.0105997322623829, 0.969607541276216, 0.0197927264614012],
                [0.0105997322623829, 0.0196075412762162, 0.969792726461401],
            ])),
            // TODO:
            // Transform::FileTransform {
            //     src: "Guard_Rail_Shaper_EOTF.spi1d".into(),
            //     interpolation: Interpolation::Linear,
            //     direction_inverse: true,
            // },
            // Transform::FileTransform {
            //     src: "luminance_compensation_srgb.cube".into(),
            //     interpolation: Interpolation::Tetrahedral,
            //     direction_inverse: false,
            // },
            // Transform::FileTransform {
            //     src: "Guard_Rail_Shaper_EOTF.spi1d".into(),
            //     interpolation: Interpolation::Linear,
            //     direction_inverse: false,
            // },
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::invert([
                [0.960599732262383, 0.0196075412762159, 0.019792726461401],
                [0.0105997322623829, 0.969607541276216, 0.0197927264614012],
                [0.0105997322623829, 0.0196075412762162, 0.969792726461401],
            ]).unwrap())),
            Transform::ColorSpaceTransform {src: "Linear FilmLight E-Gamut".into(), dst: "Linear Rec.709".into()},
            Transform::RangeTransform {
                range_in: (Some(0.0), None),
                range_out: (Some(0.0), None),
                clamp: true,
            },
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Luminance Compensation P3".into(),
        description:
            "Offset the negative values in P3 and compensate for luminance, ensuring there is no negative values in P3"
                .into(),
        family: "Utilities".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {src: "Linear CIE-XYZ E".into(), dst: "Linear FilmLight E-Gamut".into()},
            Transform::MatrixTransform(matrix::to_4x4_f32([
                [0.960599732262383, 0.0196075412762159, 0.019792726461401],
                [0.0105997322623829, 0.969607541276216, 0.0197927264614012],
                [0.0105997322623829, 0.0196075412762162, 0.969792726461401],
            ])),
            // TODO:
            // Transform::FileTransform {
            //     src: "Guard_Rail_Shaper_EOTF.spi1d".into(),
            //     interpolation: Interpolation::Linear,
            //     direction_inverse: true,
            // },
            // Transform::FileTransform {
            //     src: "luminance_compensation_p3.cube".into(),
            //     interpolation: Interpolation::Tetrahedral,
            //     direction_inverse: false,
            // },
            // Transform::FileTransform {
            //     src: "Guard_Rail_Shaper_EOTF.spi1d".into(),
            //     interpolation: Interpolation::Linear,
            //     direction_inverse: false,
            // },
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::invert([
                [0.960599732262383, 0.0196075412762159, 0.019792726461401],
                [0.0105997322623829, 0.969607541276216, 0.0197927264614012],
                [0.0105997322623829, 0.0196075412762162, 0.969792726461401],
            ]).unwrap())),
            Transform::ColorSpaceTransform {src: "Linear FilmLight E-Gamut".into(), dst: "Linear DCI-P3 D65".into()},
            Transform::RangeTransform {
                range_in: (Some(0.0), None),
                range_out: (Some(0.0), None),
                clamp: true,
            },
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "AgX Log".into(),
        description:
            "Log Encoding with Chroma inset and rotation, and with 25 Stops of Dynamic Range"
                .into(),
        family: "Log Encodings".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {src: "Linear CIE-XYZ E".into(), dst: "Luminance Compensation Rec.2020".into()},
            Transform::MatrixTransform(matrix::to_4x4_f32([
                [0.856627153315983, 0.0951212405381588, 0.0482516061458583],
                [0.137318972929847, 0.761241990602591, 0.101439036467562],
                [0.11189821299995, 0.0767994186031903, 0.811302368396859],
            ])),
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: false,
            },
        ],
        to_reference: vec![
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.47393, 12.5260688117],
                direction_inverse: true,
            },
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::invert([
                [0.856627153315983, 0.0951212405381588, 0.0482516061458583],
                [0.137318972929847, 0.761241990602591, 0.101439036467562],
                [0.11189821299995, 0.0767994186031903, 0.811302368396859],
            ]).unwrap())),
            Transform::ColorSpaceTransform {src: "Linear Rec.2020".into(), dst: "Linear CIE-XYZ E".into()},
        ],
        ..ColorSpace::default()
    });

    //---------------------------------------------------------
    // Output files.

    config.output_files.extend([
        // Filmic Blender.
        (
            "filmic/filmic_desat_33.cube".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_DESAT_33_CUBE_XZ)),
        ),
        (
            "filmic/filmic_false_color.spi3d".into(),
            OutputFile::Raw(crate::decompress_xz(
                crate::data::FILMIC_FALSE_COLOR_SPI3D_XZ,
            )),
        ),
        (
            "filmic/filmic_to_0-35_1-30.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_0_35_SPI1D_XZ)),
        ),
        (
            "filmic/filmic_to_0-48_1-09.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_0_48_SPI1D_XZ)),
        ),
        (
            "filmic/filmic_to_0-60_1-04.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_0_60_SPI1D_XZ)),
        ),
        (
            "filmic/filmic_to_0-70_1-03.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_0_70_SPI1D_XZ)),
        ),
        (
            "filmic/filmic_to_0-85_1-011.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_0_85_SPI1D_XZ)),
        ),
        (
            "filmic/filmic_to_0.99_1-0075.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_099_SPI1D_XZ)),
        ),
        (
            "filmic/filmic_to_1.20_1-00.spi1d".into(),
            OutputFile::Raw(crate::decompress_xz(crate::data::FILMIC_TO_120_SPI1D_XZ)),
        ),
    ]);

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_blender_3_0_test() {
        make_blender_3_0();
    }
}
