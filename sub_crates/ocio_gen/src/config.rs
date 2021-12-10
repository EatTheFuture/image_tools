use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use colorbox::lut::{Lut1D, Lut3D};

#[derive(Debug, Clone)]
pub struct OCIOConfig {
    // Files to include.
    pub output_files: Vec<OutputFile>,

    // Header fields.
    pub name: Option<String>,
    pub description: Option<String>,
    pub search_path: Vec<PathBuf>,

    // Config sections.
    pub roles: Roles,

    // pub file_rules: TODO.
    pub displays: Vec<Display>,
    pub active_displays: Vec<String>, // If empty, not written to config.
    pub active_views: Vec<String>,    // If empty, not written to config.

    pub looks: Vec<Look>,

    pub colorspaces: Vec<ColorSpace>,
}

impl Default for OCIOConfig {
    fn default() -> OCIOConfig {
        OCIOConfig {
            output_files: Vec::new(),

            name: None,
            description: None,
            search_path: Vec::new(),

            roles: Roles::default(),
            displays: Vec::new(),
            active_displays: Vec::new(),
            active_views: Vec::new(),
            looks: Vec::new(),
            colorspaces: Vec::new(),
        }
    }
}

impl OCIOConfig {
    pub fn new() -> OCIOConfig {
        OCIOConfig::default()
    }

    pub fn write_to_directory(&self, dir_path: &Path) -> std::io::Result<()> {
        // First ensure all the directories we need exist.
        crate::ensure_dir_exists(dir_path)?;
        for output_file in self.output_files.iter() {
            if let Some(path) = output_file.path().parent() {
                crate::ensure_dir_exists(&dir_path.join(path))?;
            }
        }
        for path in self.search_path.iter() {
            if path.is_relative() {
                crate::ensure_dir_exists(&path)?;
            }
        }

        // Write the output files.
        for output_file in self.output_files.iter() {
            let mut f = BufWriter::new(std::fs::File::create(&dir_path.join(output_file.path()))?);
            match output_file {
                OutputFile::Raw { data, .. } => f.write_all(&data)?,
                OutputFile::Lut1D { output_path, lut } => {
                    match output_path.extension().map(|e| e.to_str()).flatten() {
                        Some("spi1d") => {
                            if lut.ranges.len() > 1 {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    ".spi1d files don't support per-channel input ranges",
                                ));
                            } else {
                                let tables: Vec<&[f32]> =
                                    lut.tables.iter().map(|t| t.as_ref()).collect();
                                colorbox::formats::spi1d::write(
                                    &mut f,
                                    lut.ranges[0].0,
                                    lut.ranges[0].1,
                                    &tables,
                                )?;
                            }
                        }

                        Some("cube") => {
                            let ranges = match lut.ranges.len() {
                                1 => [lut.ranges[0], lut.ranges[0], lut.ranges[0]],
                                2 => [lut.ranges[0], lut.ranges[1], lut.ranges[1]],
                                _ => [lut.ranges[0], lut.ranges[1], lut.ranges[2]],
                            };
                            let tables = match lut.tables.len() {
                                1 => [&lut.tables[0][..], &lut.tables[0][..], &lut.tables[0][..]],
                                2 => [&lut.tables[0][..], &lut.tables[1][..], &lut.tables[1][..]],
                                _ => [&lut.tables[0][..], &lut.tables[1][..], &lut.tables[2][..]],
                            };
                            colorbox::formats::cube::write_1d(&mut f, ranges, tables)?;
                        }

                        _ => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Unsupported LUT output file format",
                            ))
                        }
                    }
                }
                OutputFile::Lut3D { .. } => todo!(),
            }
        }

        // Write the config file.
        self.write_config_file(BufWriter::new(std::fs::File::create(
            dir_path.join("config.ocio"),
        )?))?;

        Ok(())
    }

    fn write_config_file<W: std::io::Write>(&self, mut file: W) -> std::io::Result<()> {
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
            for (i, path) in self.search_path.iter().enumerate() {
                if i != 0 {
                    file.write_all(b":")?;
                }
                file.write_all(path.to_string_lossy().as_bytes())?
            }
            file.write_all(b"\"\n")?;
        }
        file.write_all(b"strictparsing: true\n")?;
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
        if let Some(colorspace) = &self.roles.default {
            file.write_all(format!("  default: {}\n", colorspace).as_bytes())?;
        }
        if let Some(colorspace) = &self.roles.data {
            file.write_all(format!("  data: {}\n", colorspace).as_bytes())?;
        }
        for (role, colorspace) in &self.roles.other {
            file.write_all(format!("  {}: {}\n", role, colorspace).as_bytes())?;
        }
        file.write_all(b"\n")?;

        // Displays and views.
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
        if !self.active_displays.is_empty() {
            file.write_all(b"active_displays: [")?;
            for (i, d) in self.active_displays.iter().enumerate() {
                if i != 0 {
                    file.write_all(b", ")?;
                }
                file.write_all(d.as_bytes())?;
            }
            file.write_all(b"]\n")?;
        }
        if !self.active_displays.is_empty() {
            file.write_all(b"active_views: [")?;
            for (i, v) in self.active_views.iter().enumerate() {
                if i != 0 {
                    file.write_all(b", ")?;
                }
                file.write_all(v.as_bytes())?;
            }
            file.write_all(b"]\n")?;
        }

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
            if !colorspace.description.is_empty() {
                file.write_all(
                    format!(
                        "description: |\n{}\n",
                        colorspace.description.trim().replace("\n", "      \n")
                    )
                    .as_bytes(),
                )?;
            }
            if !colorspace.family.is_empty() {
                file.write_all(format!("    family: {}\n", colorspace.family).as_bytes())?;
            }
            if !colorspace.equalitygroup.is_empty() {
                file.write_all(
                    format!("    equalitygroup: {}\n", colorspace.equalitygroup).as_bytes(),
                )?;
            }
            if let Some(encoding) = colorspace.encoding {
                file.write_all(format!("    encoding: {}\n", encoding.as_str()).as_bytes())?;
            }
            if let Some(bitdepth) = colorspace.bitdepth {
                file.write_all(format!("    bitdepth: {}\n", bitdepth.as_str()).as_bytes())?;
            }
            if colorspace.isdata == Some(true) {
                file.write_all(b"    isdata: true\n")?;
            }
            if let Some(allocation) = colorspace.allocation {
                file.write_all(format!("    allocation: {}\n", allocation.as_str()).as_bytes())?;
                file.write_all(b"    allocationvars: [")?;
                for (i, n) in colorspace.allocationvars.iter().enumerate() {
                    if i != 0 {
                        file.write_all(b", ")?;
                    }
                    file.write_all(n.to_string().as_bytes())?;
                }
                file.write_all(b"]\n")?;
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

/// Specifies what color spaces to use for various purposes.
///
/// The reference, interchange, default, and data spaces have their own
/// fields in the struct, but the rest are up to the configuration.
///
/// For the hard-coded roles, the string is the name of the color space.
/// For the other roles, the left-side string is the name of the role
/// and the right-side string is the name of the color space.  (The color
/// space names should all be the names of color spaces in the config.)
///
/// Some common roles that are implemented in most configs:
///
/// - scene_linear
/// - rendering
/// - compositing_linear
/// - compositing_log
/// - color_timing (a.k.a. color grading)
/// - texture_paint
/// - matte_paint
/// - color_picking
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Roles {
    pub reference: Option<String>, // Everything else is defined in terms of this.

    pub aces_interchange: Option<String>,        // ACES2065-1.
    pub cie_xyz_d65_interchange: Option<String>, // A D65-adapted CIE 1931 XYZ space.

    pub default: Option<String>,
    pub data: Option<String>,

    // Other roles
    pub other: Vec<(String, String)>, // (role_name, colorspace_name)
}

impl Default for Roles {
    fn default() -> Roles {
        Roles {
            reference: None,
            aces_interchange: None,
            cie_xyz_d65_interchange: None,
            default: None,
            data: None,
            other: Vec::new(),
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
    pub name: String,
    pub process_space: String,
    pub transform: Vec<Transform>,         // Required.
    pub inverse_transform: Vec<Transform>, // Optional, can be empty.
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorSpace {
    pub name: String,
    pub description: String,

    pub family: String,
    pub equalitygroup: String,

    pub encoding: Option<Encoding>,
    pub bitdepth: Option<BitDepth>,
    pub isdata: Option<bool>, // OCIO treats absence as "false".
    pub allocation: Option<Allocation>,
    pub allocationvars: Vec<f64>,

    // At least one of these needs to be filled in.
    pub from_reference: Vec<Transform>,
    pub to_reference: Vec<Transform>,
}

impl Default for ColorSpace {
    fn default() -> ColorSpace {
        ColorSpace {
            name: String::new(),
            description: String::new(),
            family: String::new(),
            equalitygroup: String::new(),
            encoding: None,
            bitdepth: None,
            isdata: None,
            allocation: None,
            allocationvars: Vec::new(),
            from_reference: Vec::new(),
            to_reference: Vec::new(),
        }
    }
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
    AllocationTransform {
        allocation: Allocation,
        vars: Vec<f32>,
        direction_inverse: bool,
    },
}

pub fn write_transform_yaml<W: std::io::Write>(
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
            for (i, n) in matrix.iter().enumerate() {
                if i != 0 {
                    matrix_string.push_str(", ");
                }
                matrix_string.push_str(&n.to_string());
            }
            format!("!<MatrixTransform> {{ matrix: [{}] }}", matrix_string)
        }
        &Transform::AllocationTransform {
            allocation,
            ref vars,
            direction_inverse,
        } => {
            let mut vars_string = String::new();
            for (i, n) in vars.iter().enumerate() {
                if i != 0 {
                    vars_string.push_str(", ");
                }
                vars_string.push_str(&n.to_string());
            }
            format!(
                "!<AllocationTransform> {{ allocation: {}, vars: [{}]{} }}",
                allocation.as_str(),
                vars_string,
                if direction_inverse {
                    ", direction: inverse"
                } else {
                    ""
                },
            )
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Allocation {
    Uniform,
    Log2,
}

impl Allocation {
    fn as_str(&self) -> &'static str {
        match *self {
            Allocation::Uniform => "uniform",
            Allocation::Log2 => "lg2",
        }
    }
}

#[derive(Debug, Clone)]
pub enum OutputFile {
    Raw { output_path: PathBuf, data: Vec<u8> },
    Lut1D { output_path: PathBuf, lut: Lut1D },
    Lut3D { output_path: PathBuf, lut: Lut3D },
}

impl OutputFile {
    pub fn path(&self) -> &Path {
        match self {
            OutputFile::Raw {
                ref output_path, ..
            } => output_path,
            OutputFile::Lut1D {
                ref output_path, ..
            } => output_path,
            OutputFile::Lut3D {
                ref output_path, ..
            } => output_path,
        }
    }
}
