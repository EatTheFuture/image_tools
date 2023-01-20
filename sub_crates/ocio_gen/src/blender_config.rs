use crate::config::*;

use colorbox::{chroma, lut::Lut1D, matrix, matrix_compose};

pub const REFERENCE_SPACE_CHROMA: chroma::Chromaticities = chroma::REC709;

// LUTs we don't know how to compute, so we include them compressed
// in the executable.
const DCI_XYZ_SPI1D_XZ: &[u8] = include_bytes!("../data/blender/dci_xyz.spi1d.xz");
const LG10_SPI1D_XZ: &[u8] = include_bytes!("../data/blender/lg10.spi1d.xz");
const FILMIC_DESAT65CUBE_SPI3D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_desat65cube.spi3d.xz");
const FILMIC_FALSE_COLOR_SPI3D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_false_color.spi3d.xz");
const FILMIC_TO_0_35_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-35_1-30.spi1d.xz");
const FILMIC_TO_0_48_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-48_1-09.spi1d.xz");
const FILMIC_TO_0_60_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-60_1-04.spi1d.xz");
const FILMIC_TO_0_70_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-70_1-03.spi1d.xz");
const FILMIC_TO_0_85_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-85_1-011.spi1d.xz");
const FILMIC_TO_099_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0.99_1-0075.spi1d.xz");
const FILMIC_TO_120_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_1.20_1-00.spi1d.xz");

/// Builds a config that matches Blender 3.0's default.
pub fn make_blender_3_0() -> OCIOConfig {
    let mut config = OCIOConfig::default();

    config.name = Some("Blender 3.0 (customized)".into());
    config.description = Some("Customized variant of the Blender 3.0 configuration.".into());
    config.search_path = vec!["luts".into(), "filmic".into()];

    config.roles.reference = Some("Linear".into());
    config.roles.aces_interchange = Some("Linear ACES".into());
    config.roles.default = Some("Linear".into());
    config.roles.data = Some("Non-Color".into());
    config.roles.other = [
        ("scene_linear".into(), "Linear".into()),
        ("rendering".into(), "Linear".into()),
        ("default_byte".into(), "sRGB".into()),
        ("default_float".into(), "Linear".into()),
        ("default_sequencer".into(), "sRGB".into()),
        ("color_picking".into(), "sRGB".into()),
        ("color_timing".into(), "Filmic Log".into()),
        ("compositing_log".into(), "Filmic Log".into()),
        ("matte_paint".into(), "Linear".into()),
        ("texture_paint".into(), "Linear".into()),
    ]
    .into();

    config.displays.push(Display {
        name: "sRGB".into(),
        views: vec![
            ("Standard".into(), "sRGB".into()),
            ("Filmic".into(), "Filmic sRGB".into()),
            ("Filmic Log".into(), "Filmic Log".into()),
            ("Raw".into(), "Raw".into()),
            ("False Color".into(), "False Color".into()),
        ],
    });
    config.displays.push(Display {
        name: "XYZ".into(),
        views: vec![
            ("Standard".into(), "XYZ".into()),
            ("DCI".into(), "dci_xyz".into()),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.displays.push(Display {
        name: "None".into(),
        views: vec![("Standard".into(), "Raw".into())],
    });

    config.active_displays = vec!["sRGB".into(), "XYZ".into(), "None".into()];
    config.active_views = vec![
        "Standard".into(),
        "Filmic".into(),
        "Filmic Log".into(),
        "Raw".into(),
        "False Color".into(),
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

    //---------------------------------------------------------
    // Color spaces.

    config.colorspaces.push(ColorSpace {
        name: "Linear".into(),
        description: "Rec. 709 (Full Range), Blender native linear space".into(),
        family: "linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Raw".into(),
        family: "raw".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(true),
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Linear ACES".into(),
        description: "ACES linear space".into(),
        family: "linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix_compose!(
                matrix::rgb_to_xyz_matrix(chroma::REC709),
                matrix::xyz_chromatic_adaptation_matrix(
                    chroma::REC709.w,
                    (1.0 / 3.0, 1.0 / 3.0),
                    matrix::AdaptationMethod::XYZScale,
                ),
                matrix::xyz_to_rgb_matrix(chroma::ACES_AP0),
            ),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "nuke_rec709".into(),
        description: "Rec. 709 (Full Range) Display Space".into(),
        family: "display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        to_reference: vec![Transform::FileTransform {
            src: "rec709.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: false,
        }],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "XYZ".into(),
        family: "linear".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::rgb_to_xyz_matrix(chroma::REC709),
        ))],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "dci_xyz".into(),
        description: "OpenDCP output LUT with DCI reference white and Gamma 2.6".into(),
        family: "display".into(),
        bitdepth: Some(BitDepth::F16),
        isdata: Some(false),
        from_reference: vec![
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::rgb_to_xyz_matrix(
                chroma::REC709,
            ))),
            Transform::FileTransform {
                src: "dci_xyz.spi1d".into(),
                interpolation: Interpolation::Linear,
                direction_inverse: false,
            },
        ],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "lg10".into(),
        description: "conversion from film log".into(),
        family: "display".into(),
        bitdepth: Some(BitDepth::UI10),
        isdata: Some(false),
        to_reference: vec![Transform::FileTransform {
            src: "lg10.spi1d".into(),
            interpolation: Interpolation::Nearest,
            direction_inverse: false,
        }],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "sRGB".into(),
        description: "Standard RGB Display Space".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        to_reference: vec![Transform::FileTransform {
            src: "srgb.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: false,
        }],
        from_reference: vec![Transform::FileTransform {
            src: "srgb_inv.spi1d".into(),
            interpolation: Interpolation::Linear,
            direction_inverse: false,
        }],
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
        name: "Filmic Log".into(),
        description:
            "Log based filmic shaper with 16.5 stops of latitude, and 25 stops of dynamic range"
                .into(),
        family: "log".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::AllocationTransform {
                allocation: Allocation::Log2,
                vars: vec![-12.473931188, 12.526068812],
                direction_inverse: false,
            },
            Transform::FileTransform {
                src: "filmic_desat65cube.spi3d".into(),
                interpolation: Interpolation::Best,
                direction_inverse: false,
            },
            Transform::AllocationTransform {
                allocation: Allocation::Uniform,
                vars: vec![0.0, 0.66],
                direction_inverse: false,
            },
        ],
        to_reference: vec![Transform::AllocationTransform {
            allocation: Allocation::Log2,
            vars: vec![-12.473931188, 4.026068812],
            direction_inverse: true,
        }],
        ..ColorSpace::default()
    });

    config.colorspaces.push(ColorSpace {
        name: "Filmic sRGB".into(),
        description: "Filmic sRGB view transform".into(),
        family: "display".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(false),
        from_reference: vec![
            Transform::ColorSpaceTransform {
                src: "Linear".into(),
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

    config.colorspaces.push(ColorSpace {
        name: "False Color".into(),
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
            Transform::FileTransform {
                src: "filmic_false_color.spi3d".into(),
                interpolation: Interpolation::Best,
                direction_inverse: false,
            },
        ],
        ..ColorSpace::default()
    });

    //---------------------------------------------------------
    // Output files.

    config.output_files = vec![
        OutputFile::Raw {
            output_path: "luts/dci_xyz.spi1d".into(),
            data: crate::decompress_xz(DCI_XYZ_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "luts/lg10.spi1d".into(),
            data: crate::decompress_xz(LG10_SPI1D_XZ),
        },
        OutputFile::Lut1D {
            output_path: "luts/rec709.spi1d".into(),
            lut: Lut1D::from_fn_1(
                4101,
                -0.125,
                1.125,
                colorbox::transfer_functions::rec709::to_linear,
            ),
        },
        OutputFile::Lut1D {
            output_path: "luts/srgb.spi1d".into(),
            lut: Lut1D::from_fn_1(
                65561,
                -0.125,
                4.875,
                colorbox::transfer_functions::srgb::to_linear,
            ),
        },
        OutputFile::Lut1D {
            output_path: "luts/srgb_inv.spi1d".into(),
            lut: Lut1D::from_fn_1(
                65561,
                -0.00967492260062,
                40.4600768322,
                colorbox::transfer_functions::srgb::from_linear,
            ),
        },
        // Filmic Blender.
        OutputFile::Raw {
            output_path: "filmic/filmic_desat65cube.spi3d".into(),
            data: crate::decompress_xz(FILMIC_DESAT65CUBE_SPI3D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_false_color.spi3d".into(),
            data: crate::decompress_xz(FILMIC_FALSE_COLOR_SPI3D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_0-35_1-30.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_0_35_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_0-48_1-09.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_0_48_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_0-60_1-04.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_0_60_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_0-70_1-03.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_0_70_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_0-85_1-011.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_0_85_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_0.99_1-0075.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_099_SPI1D_XZ),
        },
        OutputFile::Raw {
            output_path: "filmic/filmic_to_1.20_1-00.spi1d".into(),
            data: crate::decompress_xz(FILMIC_TO_120_SPI1D_XZ),
        },
    ];

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
