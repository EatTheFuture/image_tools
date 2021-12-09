use crate::config::*;

/// Builds a config that matches Blender 3.0's default.
fn make_blender_3_0() -> OCIOConfig {
    let mut config = OCIOConfig::default();

    config.description = Some(
        "A customized variant of the Blender 3.0 configuration.\
         Uses a linear Rec.709 space as the reference color space."
            .into(),
    );
    config.search_path.push("luts".into());
    config.search_path.push("filmic".into());

    config.roles.reference = Some("Linear".into());
    config.roles.aces_interchange = Some("Linear ACES".into());
    config.roles.default = Some("Linear".into());
    config.roles.data = Some("Non-Color".into());

    config.roles.other = vec![
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
    ];

    config
}
