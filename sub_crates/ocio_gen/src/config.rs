use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct OCIOConfig {
    // Header fields.
    pub name: Option<String>,
    pub description: Option<String>,
    pub search_path: Vec<PathBuf>,

    // Config sections.
    pub roles: Roles,
    // pub file_rules: TODO.
    pub displays: Vec<Display>,
    pub looks: Vec<Look>,
    pub colorspaces: Vec<ColorSpace>,
}

impl OCIOConfig {
    pub fn new() -> OCIOConfig {
        OCIOConfig::default()
    }

    pub fn write<W: std::io::Write>(&self, mut file: W) -> std::io::Result<()> {
        // Header.
        file.write_all(b"ocio_profile_version:2\n\n")?;
        if let Some(name) = &self.name {
            file.write_all(format!("name: {}\n", name).as_bytes())?;
        }
        if let Some(description) = &self.description {
            file.write_all(
                format!(
                    "description: |\n{}\n",
                    description.trim().replace("\n", "    \n")
                )
                .as_bytes(),
            )?;
        }
        if !self.search_path.is_empty() {
            file.write_all(b"search_path: \"")?;
            let mut is_first = true;
            for path in self.search_path.iter() {
                if is_first {
                    is_first = false;
                } else {
                    file.write_all(b":")?;
                }
                file.write_all(path.to_string_lossy().as_bytes())?
            }
            file.write_all(b"\"\n")?;
        }
        file.write_all(b"\n")?;

        // Roles.
        file.write_all(b"roles:\n")?;
        if let Some(colorspace) = &self.roles.reference {
            file.write_all(format!("  reference: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.aces_interchange {
            file.write_all(format!("  aces_interchange: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.cie_xyz_d65_interchange {
            file.write_all(format!("  cie_xyz_d65_interchange: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.scene_linear {
            file.write_all(format!("  scene_linear: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.rendering {
            file.write_all(format!("  rendering: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.compositing_linear {
            file.write_all(format!("  compositing_linear: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.compositing_log {
            file.write_all(format!("  compositing_log: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.color_timing {
            file.write_all(format!("  color_timing: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.texture_paint {
            file.write_all(format!("  texture_paint: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.matte_paint {
            file.write_all(format!("  matte_paint: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.color_picking {
            file.write_all(format!("  color_picking: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.data {
            file.write_all(format!("  data: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.default {
            file.write_all(format!("  default: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.default_byte {
            file.write_all(format!("  default_byte: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.default_float {
            file.write_all(format!("  default_float: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.default_sequencer {
            file.write_all(format!("  default_sequencer: {}\n", colorspace).as_bytes())?;
        }
        for (role, colorspace) in &self.roles.custom {
            file.write_all(format!("  {}: {}\n", role, colorspace).as_bytes())?;
        }
        file.write_all(b"\n")?;

        // Displays.
        file.write_all(b"displays:\n")?;
        for display in self.displays.iter() {
            file.write_all(format!("  {}:\n", display.name).as_bytes())?;
            for (name, colorspace) in display.views.iter() {
                file.write_all(
                    format!(
                        "    - !<View> {{ name: {}, colorspace: {} }}\n",
                        name, colorspace
                    )
                    .as_bytes(),
                )?;
            }
        }
        file.write_all(b"\n")?;

        // Looks.
        if !self.looks.is_empty() {
            file.write_all(b"looks:\n")?;
            for look in self.looks.iter() {
                file.write_all(b"  - !<Look>\n")?;
                file.write_all(format!("    name: {}\n", look.name).as_bytes())?;
                file.write_all(format!("    process_space: {}\n", look.process_space).as_bytes())?;
                write_transform_yaml(&mut file, 4, "transform", &look.transform[..])?;
                if !look.inverse_transform.is_empty() {
                    write_transform_yaml(
                        &mut file,
                        4,
                        "inverse_transform",
                        &look.inverse_transform[..],
                    )?;
                }
            }
        }
        file.write_all(b"\n")?;

        // Color spaces.
        file.write_all(b"colorspaces:\n")?;
        for colorspace in self.colorspaces.iter() {
            file.write_all(b"  - !<ColorSpace>\n")?;
            file.write_all(format!("    name: {}\n", colorspace.name).as_bytes())?;
            if let Some(encoding) = colorspace.encoding {
                file.write_all(format!("    encoding: {}\n", encoding.as_str()).as_bytes())?;
            }
            if let Some(bitdepth) = colorspace.bitdepth {
                file.write_all(format!("    bitdepth: {}\n", bitdepth.as_str()).as_bytes())?;
            }
            if colorspace.isdata == Some(true) {
                file.write_all(b"    isdata: true\n")?;
            }

            if !colorspace.from_reference.is_empty() {
                write_transform_yaml(
                    &mut file,
                    4,
                    "from_reference",
                    &colorspace.from_reference[..],
                )?;
            }
            if !colorspace.to_reference.is_empty() {
                write_transform_yaml(&mut file, 4, "to_reference", &colorspace.to_reference[..])?;
            }
        }

        Ok(())
    }
}

impl Default for OCIOConfig {
    fn default() -> OCIOConfig {
        OCIOConfig {
            name: None,
            description: None,
            search_path: Vec::new(),

            roles: Roles::default(),
            displays: Vec::new(),
            looks: Vec::new(),
            colorspaces: Vec::new(),
        }
    }
}

/// Specifies what color spaces to use for various purposes.
///
/// The `String`s should all be names of colorspaces that are in the config.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Roles {
    // Reference OCIO roles.
    pub reference: Option<String>, // Everything else is defined in terms of this.
    pub aces_interchange: Option<String>, // ACES2065-1.
    pub cie_xyz_d65_interchange: Option<String>, // A D65-adapted CIE 1931 XYZ space.

    // Other common OCIO roles.
    pub scene_linear: Option<String>,
    pub rendering: Option<String>,
    pub compositing_linear: Option<String>,
    pub compositing_log: Option<String>,
    pub color_timing: Option<String>, // Color grading.
    pub texture_paint: Option<String>,
    pub matte_paint: Option<String>,
    pub color_picking: Option<String>,
    pub data: Option<String>,
    pub default: Option<String>,

    // Blender roles.
    pub default_byte: Option<String>,
    pub default_float: Option<String>,
    pub default_sequencer: Option<String>,

    // Custom roles.
    pub custom: Vec<(String, String)>, // (role_name, colorspace_name)
}

impl Default for Roles {
    fn default() -> Roles {
        Roles {
            reference: None,
            aces_interchange: None,
            cie_xyz_d65_interchange: None,

            scene_linear: None,
            rendering: None,
            compositing_linear: None,
            compositing_log: None,
            color_timing: None,
            texture_paint: None,
            matte_paint: None,
            color_picking: None,
            data: None,
            default: None,

            default_byte: None,
            default_float: None,
            default_sequencer: None,

            custom: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Display {
    pub name: String,
    pub views: Vec<(String, String)>, // (view_name, colorspace_name)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    name: String,
    process_space: String,
    transform: Vec<Transform>,         // Required.
    inverse_transform: Vec<Transform>, // Optional, can be empty.
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorSpace {
    name: String,
    encoding: Option<Encoding>,
    bitdepth: Option<BitDepth>,
    isdata: Option<bool>,

    // At least one of these needs to be filled in.
    from_reference: Vec<Transform>,
    to_reference: Vec<Transform>,
}

/// A color transform.
///
/// `GroupTransform` is not represented here, as all places
/// where this is used are `Vec`s, and are automatically
/// treated as a group transform when more than one transform
/// is in the `Vec`.
#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    FileTransform {
        src: PathBuf,
        interpolation: Interpolation,
        direction_inverse: bool, // Whether to apply it in reverse.
    },
    ColorSpaceTransform {
        src: String,
        dst: String,
    },
    MatrixTransform([f32; 16]),
}

fn write_transform_yaml<W: std::io::Write>(
    mut file: W,
    indent: usize,
    header: &str,
    transforms: &[Transform],
) -> std::io::Result<()> {
    let indent: String = [' '].iter().cycle().take(indent).collect();

    let transform_text = |t| match t {
        &Transform::FileTransform {
            ref src,
            interpolation,
            direction_inverse,
        } => {
            format!(
                "!<FileTransform> {{ src: {}, interpolation: {}{} }}",
                src.to_string_lossy(),
                interpolation.as_str(),
                if direction_inverse {
                    ", direction: inverse"
                } else {
                    ""
                },
            )
        }
        &Transform::ColorSpaceTransform { ref src, ref dst } => {
            format!("!<ColorSpaceTransform> {{ src: {}, dst: {} }}", src, dst)
        }
        &Transform::MatrixTransform(matrix) => {
            let mut matrix_string = String::new();
            let mut is_first = true;
            for n in matrix.iter() {
                if !is_first {
                    matrix_string.push_str(", ");
                } else {
                    is_first = false;
                }
                matrix_string.push_str(&n.to_string());
            }
            format!("!<MatrixTransform> {{ matrix: [{}] }}", matrix_string)
        }
    };

    if transforms.len() == 1 {
        file.write_all(
            format!("{}{}: {}\n", indent, header, transform_text(&transforms[0])).as_bytes(),
        )?;
    } else {
        file.write_all(format!("{}{}: !<GroupTransform>\n", indent, header).as_bytes())?;
        file.write_all(format!("{}  children:\n", indent).as_bytes())?;
        for transform in transforms.iter() {
            file.write_all(format!("{}    - {}\n", indent, transform_text(transform)).as_bytes())?;
        }
    }

    Ok(())
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Interpolation {
    Nearest,
    Linear,
    Best,
    Tetrahedral, // For 3d LUTs only.
}

impl Interpolation {
    fn as_str(&self) -> &'static str {
        match *self {
            Interpolation::Nearest => "nearest",
            Interpolation::Linear => "linear",
            Interpolation::Best => "best",
            Interpolation::Tetrahedral => "tetrahedral",
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Encoding {
    SceneLinear,
    DisplayLinear,
    Log,
    SDRVideo,
    HDRVideo,
    Data,
}

impl Encoding {
    fn as_str(&self) -> &'static str {
        match *self {
            Encoding::SceneLinear => "scene-linear",
            Encoding::DisplayLinear => "display-linear",
            Encoding::Log => "log",
            Encoding::SDRVideo => "sdr-video",
            Encoding::HDRVideo => "hdr-video",
            Encoding::Data => "data",
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BitDepth {
    // Unsigned integers.
    UI8,
    UI10,
    UI12,
    UI14,
    UI16,
    UI32,

    // Floating point.
    F16,
    F32,
}

impl BitDepth {
    fn as_str(&self) -> &'static str {
        match *self {
            BitDepth::UI8 => "8ui",
            BitDepth::UI10 => "10ui",
            BitDepth::UI12 => "12ui",
            BitDepth::UI14 => "14ui",
            BitDepth::UI16 => "16ui",
            BitDepth::UI32 => "32ui",
            BitDepth::F16 => "16f",
            BitDepth::F32 => "32f",
        }
    }
}
