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

    config.displays.push(Display {
        name: "sRGB".into(),
        views: vec![
            ("Standard".into(), "sRGB Gamut Clipped".into()),
            ("Raw".into(), "Raw".into()),
        ],
    });
    config.displays.push(Display {
        name: "None".into(),
        views: vec![("Standard".into(), "Raw".into())],
    });

    config.active_displays = vec!["sRGB".into(), "None".into()];
    config.active_views = vec!["Standard".into(), "Raw".into()];

    //---------------------------------------------------------
    // Color spaces.

    config.colorspaces.push(ColorSpace {
        name: "Linear".into(),
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
        true,
    );

    config.add_input_colorspace(
        "sRGB".into(),
        None,
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        Some(Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: false,
        }),
        true,
    );

    config.add_display_colorspace(
        "sRGB Gamut Clipped".into(),
        None,
        chroma::REC709,
        whitepoint_adaptation_method,
        Transform::ExponentWithLinearTransform {
            gamma: 2.4,
            offset: 0.055,
            direction_inverse: true,
        },
        true,
    );

    config.colorspaces.push(ColorSpace {
        name: "Non-Color".into(),
        description: "Color space used for images which contains non-color data (i,e, normal maps)"
            .into(),
        family: "raw".into(),
        bitdepth: Some(BitDepth::F32),
        isdata: Some(true),
        ..ColorSpace::default()
    });

    config
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn make_minimal_test() {
//         make_minimal();
//     }
// }
