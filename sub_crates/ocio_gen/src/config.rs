use std::{
    collections::{HashMap, HashSet},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use colorbox::{
    chroma::Chromaticities,
    lut::{Lut1D, Lut3D},
    matrix::{self, AdaptationMethod},
};

const GAMUT_DIR: &str = "gamut_handling";
pub const INPUT_GAMUT_CLIP_LUT_FILENAME: &str = "rgb_input_gamut_clip.cube";
pub const OUTPUT_GAMUT_CLIP_LUT_FILENAME: &str = "rgb_output_gamut_clip.cube";

#[derive(Debug, Clone)]
pub struct OCIOConfig {
    // Not used during export, but rather is used in some convenience
    // functions for creating color spaces.
    pub reference_space_chroma: Chromaticities,

    // Files to include.
    pub output_files: HashMap<PathBuf, OutputFile>,

    // Top-level comment at the start of the config file.
    pub header_comment: String,

    // Header fields.
    pub name: Option<String>,
    pub description: Option<String>,
    pub search_path: HashSet<PathBuf>,

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
            reference_space_chroma: colorbox::chroma::REC709,

            output_files: HashMap::new(),

            header_comment: String::new(),
            name: None,
            description: None,
            search_path: HashSet::new(),

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

    pub fn write_to_directory<P: AsRef<Path>>(&self, dir_path: P) -> std::io::Result<()> {
        let dir_path: &Path = dir_path.as_ref();

        // First ensure all the directories we need exist.
        crate::ensure_dir_exists(dir_path)?;
        for (output_path, _) in self.output_files.iter() {
            if let Some(path) = output_path.parent() {
                crate::ensure_dir_exists(&dir_path.join(path))?;
            }
        }
        for path in self.search_path.iter() {
            if path.is_relative() {
                crate::ensure_dir_exists(&dir_path.join(path))?;
            }
        }

        // Write the output files.
        for (output_path, output_file) in self.output_files.iter() {
            let mut f = BufWriter::new(std::fs::File::create(&dir_path.join(output_path))?);
            match output_file {
                OutputFile::Raw(data) => f.write_all(&data)?,
                OutputFile::Lut1D(lut) => {
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
                            colorbox::formats::cube_iridas::write_1d(&mut f, ranges, tables)?;
                        }

                        _ => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Unsupported LUT output file format",
                            ))
                        }
                    }
                }
                OutputFile::Lut3D(lut) => {
                    match output_path.extension().map(|e| e.to_str()).flatten() {
                        Some("cube") => colorbox::formats::cube_iridas::write_3d(
                            &mut f,
                            lut.range,
                            lut.resolution[0],
                            [&lut.tables[0], &lut.tables[1], &lut.tables[2]],
                        )?,

                        _ => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Unsupported LUT output file format",
                            ))
                        }
                    }
                }
            }
        }

        // Write the config file.
        self.write_config_file(BufWriter::new(std::fs::File::create(
            dir_path.join("config.ocio"),
        )?))?;

        Ok(())
    }

    fn write_config_file<W: std::io::Write>(&self, mut file: W) -> std::io::Result<()> {
        // Header comment.
        if !self.header_comment.trim().is_empty() {
            for line in self.header_comment.lines() {
                file.write_all(format!("# {}\n", line).as_bytes())?;
            }
            file.write_all(b"\n")?;
        }

        // Header.
        file.write_all(b"ocio_profile_version: 2.1\n\n")?;
        if let Some(name) = &self.name {
            file.write_all(format!("name: {}\n", name).as_bytes())?;
        }
        if let Some(description) = &self.description {
            file.write_all(
                format!(
                    "description: |\n  {}\n",
                    description.trim().replace("\n", "  \n")
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
            file.write_all(b"\n")?;
        }
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
        if !self.active_views.is_empty() {
            file.write_all(b"active_views: [")?;
            for (i, v) in self.active_views.iter().enumerate() {
                if i != 0 {
                    file.write_all(b", ")?;
                }
                file.write_all(v.as_bytes())?;
            }
            file.write_all(b"]\n")?;
        }
        if !self.active_displays.is_empty() || !self.active_views.is_empty() {
            file.write_all(b"\n")?;
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
                file.write_all(b"\n")?;
            }
        }

        // Color spaces.
        file.write_all(b"colorspaces:\n")?;
        for colorspace in self.colorspaces.iter() {
            file.write_all(b"  - !<ColorSpace>\n")?;
            file.write_all(format!("    name: {}\n", colorspace.name).as_bytes())?;
            if !colorspace.description.is_empty() {
                file.write_all(
                    format!(
                        "    description: |\n      {}\n",
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
            file.write_all(b"\n")?;
        }

        Ok(())
    }

    /// Peforms some basic validation checks on the configuration.
    ///
    /// This is not 100% thorough by any means.
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Check for duplicate color space names.
        let mut colorspaces = HashSet::new();
        for colorspace in self.colorspaces.iter() {
            if !colorspaces.insert(colorspace.name.as_str()) {
                return Err(ValidationError::DuplicateColorSpace(
                    colorspace.name.clone(),
                ));
            }
        }

        // Check for duplicate role names.
        let mut roles = HashSet::new();
        roles.insert("reference");
        roles.insert("aces_interchange");
        roles.insert("cie_xyz_d65_interchange");
        roles.insert("default");
        roles.insert("data");
        for (role, _) in self.roles.other.iter() {
            if !roles.insert(role.as_str()) {
                return Err(ValidationError::DuplicateRole(role.clone()));
            }
        }

        // Check for duplicate display names.
        // TODO: also check for duplicate view name in the same loop,
        // since they're defined inside displays.
        let mut displays = HashSet::new();
        for display in self.displays.iter() {
            if !displays.insert(display.name.as_str()) {
                return Err(ValidationError::DuplicateDisplay(display.name.clone()));
            }
        }

        // Check for duplicate look names.
        let mut looks = HashSet::new();
        for look in self.looks.iter() {
            if !looks.insert(look.name.as_str()) {
                return Err(ValidationError::DuplicateLook(look.name.clone()));
            }
        }

        // Check for references to non-existent color spaces.
        // TODO: check inside views and color spaces themselves.
        if let Some(ref space) = self.roles.reference {
            if !colorspaces.contains(space.as_str()) {
                return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
            }
        };
        if let Some(ref space) = self.roles.aces_interchange {
            if !colorspaces.contains(space.as_str()) {
                return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
            }
        };
        if let Some(ref space) = self.roles.cie_xyz_d65_interchange {
            if !colorspaces.contains(space.as_str()) {
                return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
            }
        };
        if let Some(ref space) = self.roles.default {
            if !colorspaces.contains(space.as_str()) {
                return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
            }
        };
        if let Some(ref space) = self.roles.data {
            if !colorspaces.contains(space.as_str()) {
                return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
            }
        };
        for (_, space) in self.roles.other.iter() {
            if !colorspaces.contains(space.as_str()) {
                return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
            }
        }
        for display in self.displays.iter() {
            for (_, space) in display.views.iter() {
                if !colorspaces.contains(space.as_str()) {
                    return Err(ValidationError::ReferenceToAbsentColorSpace(space.clone()));
                }
            }
        }

        Ok(())
    }

    //---------------------------------------------------------
    // Convenience functions to help build configs more easily.

    pub fn add_input_colorspace(
        &mut self,
        name: String,
        family: Option<String>,
        description: Option<String>,
        chromaticities: Chromaticities,
        whitepoint_adaptation_method: AdaptationMethod,
        to_linear_transform: Option<Transform>,
        use_gamut_clipping: bool,
    ) {
        // Build to-reference transforms.
        let mut to_reference_transforms = Vec::new();
        if let Some(ref to_linear) = to_linear_transform {
            to_reference_transforms.push(to_linear.clone());
        }
        to_reference_transforms.push(Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[
                matrix::rgb_to_xyz_matrix(chromaticities),
                matrix::xyz_chromatic_adaptation_matrix(
                    chromaticities.w,
                    self.reference_space_chroma.w,
                    whitepoint_adaptation_method,
                ),
                matrix::xyz_to_rgb_matrix(self.reference_space_chroma),
            ]),
        )));
        if use_gamut_clipping && !gamut_is_within_gamut(chromaticities, self.reference_space_chroma)
        {
            self.generate_gamut_clipping_luts();
            to_reference_transforms.extend([
                Transform::ToHSV,
                Transform::FileTransform {
                    src: INPUT_GAMUT_CLIP_LUT_FILENAME.into(),
                    interpolation: Interpolation::Linear,
                    direction_inverse: false,
                },
                Transform::FromHSV,
            ]);
        }

        // Build from-reference transforms.
        let mut from_reference_transforms = Vec::new();
        from_reference_transforms.push(Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::invert(matrix::compose(&[
                matrix::rgb_to_xyz_matrix(chromaticities),
                matrix::xyz_chromatic_adaptation_matrix(
                    chromaticities.w,
                    self.reference_space_chroma.w,
                    whitepoint_adaptation_method,
                ),
                matrix::xyz_to_rgb_matrix(self.reference_space_chroma),
            ]))
            .unwrap(),
        )));
        if let Some(to_linear) = to_linear_transform {
            from_reference_transforms.push(to_linear.invert());
        }
        if use_gamut_clipping && !gamut_is_within_gamut(self.reference_space_chroma, chromaticities)
        {
            self.generate_gamut_clipping_luts();
            from_reference_transforms.extend([
                Transform::ToHSV,
                Transform::FileTransform {
                    src: INPUT_GAMUT_CLIP_LUT_FILENAME.into(),
                    interpolation: Interpolation::Linear,
                    direction_inverse: false,
                },
                Transform::FromHSV,
            ]);
        }

        // Add the colorspace.
        self.colorspaces.push(ColorSpace {
            name: name,
            family: family.unwrap_or("".into()),
            description: description.unwrap_or(String::new()),
            bitdepth: Some(BitDepth::F32),
            isdata: Some(false),
            to_reference: to_reference_transforms,
            from_reference: from_reference_transforms,
            ..ColorSpace::default()
        });
    }

    /// Adds a display color space with basic gamut clipping.
    pub fn add_display_colorspace(
        &mut self,
        name: String,
        description: Option<String>,
        chromaticities: Chromaticities,
        whitepoint_adaptation_method: AdaptationMethod,
        tonemap_transforms: Vec<Transform>,
        from_linear_transform: Transform,
        use_gamut_clipping: bool,
    ) {
        self.generate_gamut_clipping_luts();

        // Build transforms.
        let mut transforms = Vec::new();

        transforms.push(Transform::MatrixTransform(matrix::to_4x4_f32(
            matrix::compose(&[
                matrix::rgb_to_xyz_matrix(self.reference_space_chroma),
                matrix::xyz_chromatic_adaptation_matrix(
                    self.reference_space_chroma.w,
                    chromaticities.w,
                    whitepoint_adaptation_method,
                ),
                matrix::xyz_to_rgb_matrix(chromaticities),
            ]),
        )));

        transforms.extend(tonemap_transforms);

        if use_gamut_clipping {
            self.generate_gamut_clipping_luts();
            let scale = (1 << 24) as f64;
            transforms.extend([
                Transform::ToHSV,
                // HDR gamut clipping.
                // HACK: abuse HSV coordinates to clip saturation to be
                // within the gamut that the gamut clipping LUT can handle.
                // TODO: unfortunately, this distorts the luminance of
                // those clipped colors.  Replace this with a 3D LUT
                // transform that does proper HDR gamut clipping once
                // issue #1763 is fixed in OCIO and released.
                Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                    1.0,
                    1.0 / 1.5,
                    1.0 / scale,
                ]))),
                Transform::RangeTransform {
                    range_in: (0.0, 1.0),
                    range_out: (0.0, 1.0),
                    clamp: true,
                },
                Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                    1.0, 1.5, scale,
                ]))),
                // LDR gamut clipping.
                Transform::FileTransform {
                    src: OUTPUT_GAMUT_CLIP_LUT_FILENAME.into(),
                    interpolation: Interpolation::Linear,
                    direction_inverse: false,
                },
                Transform::FromHSV,
            ]);
        }

        transforms.push(from_linear_transform);

        // Add the colorspace.
        self.colorspaces.push(ColorSpace {
            name: name,
            description: description.unwrap_or(String::new()),
            family: "display".into(),
            bitdepth: Some(BitDepth::F32),
            isdata: Some(false),
            from_reference: transforms,
            ..ColorSpace::default()
        });
    }

    /// Creates and adds the default gamut clipping luts, if
    /// they haven't been already.
    pub fn generate_gamut_clipping_luts(&mut self) {
        use colorbox::transforms::ocio::{hsv_to_rgb, rgb_to_hsv};

        // We use these luminance weights regardless of actual gamut
        // because in practice they work plenty well, they're only
        // used for out-of-gamut colors, and this way we can re-use
        // the same luts for all gamuts.
        let input_luminance_weights = [1.0 / 13.0, 11.0 / 13.0, 1.0 / 13.0];
        let output_luminance_weights = [1.0 / 6.0, 4.0 / 6.0, 1.0 / 6.0];

        self.search_path.insert(GAMUT_DIR.into());

        let res = 3 * 4;
        let upper = (3 * (1i64 << 32)) as f64;
        let _lower = -upper;
        self.output_files
            .entry(Path::new(GAMUT_DIR).join::<PathBuf>(INPUT_GAMUT_CLIP_LUT_FILENAME.into()))
            .or_insert_with(|| {
                OutputFile::Lut3D(crate::hsv_lut::make_hsv_lut(
                    res + 1,
                    (0.0, upper),
                    1.5,
                    |(h, s, v)| {
                        let rgb = hsv_to_rgb([h, s, v]);
                        let rgb2 =
                            crate::gamut_map::rgb_clip(rgb, None, input_luminance_weights, 0.0);
                        let hsv2 = rgb_to_hsv(rgb2);
                        (hsv2[0], hsv2[1], hsv2[2])
                    },
                ))
            });

        let res = 3 * 20;
        let upper = 12;
        assert_eq!(res % upper, 0);
        self.output_files
            .entry(Path::new(GAMUT_DIR).join::<PathBuf>(OUTPUT_GAMUT_CLIP_LUT_FILENAME.into()))
            .or_insert_with(|| {
                OutputFile::Lut3D(crate::hsv_lut::make_hsv_lut(
                    res + 1,
                    (0.0, upper as f64),
                    1.5,
                    |(h, s, v)| {
                        let rgb = hsv_to_rgb([h, s, v]);
                        let rgb2 = crate::gamut_map::rgb_clip(
                            rgb,
                            Some(1.0),
                            output_luminance_weights,
                            0.075,
                        );
                        let hsv2 = rgb_to_hsv(rgb2);
                        (hsv2[0], hsv2[1], hsv2[2])
                    },
                ))
            });
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ValidationError {
    DuplicateColorSpace(String),
    DuplicateDisplay(String),
    DuplicateRole(String),
    DuplicateLook(String),
    ReferenceToAbsentColorSpace(String),
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
    pub other: HashMap<String, String>, // role_name -> colorspace_name
}

impl Default for Roles {
    fn default() -> Roles {
        Roles {
            reference: None,
            aces_interchange: None,
            cie_xyz_d65_interchange: None,
            default: None,
            data: None,
            other: HashMap::new(),
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
    BuiltinTransform {
        name: String,
        direction_inverse: bool,
    },
    AllocationTransform {
        allocation: Allocation,
        vars: Vec<f64>,
        direction_inverse: bool,
    },
    /// Maps `range_in` to `range_out`, and clamps both the low and high end.
    RangeTransform {
        range_in: (f64, f64),
        range_out: (f64, f64),
        clamp: bool,
    },
    ExponentTransform(f64, f64, f64, f64), // Separate exponents for r, g, b, and a.
    ExponentWithLinearTransform {
        gamma: f64,
        offset: f64,
        direction_inverse: bool,
    },
    ToHSV,
    FromHSV,
    ACESGamutMapTransform {
        /// Effectively the "saturation" threshhold at which the mapping kicks in.
        threshhold: [f32; 3],

        /// Effectively the "saturation" that gets mapped back to 1.0 saturation.
        limit: [f32; 3],

        /// How sharp the curve is that does the mapping.
        power: f32,

        direction_inverse: bool,
    },
}

impl Transform {
    pub fn invert(self) -> Self {
        use Transform::*;
        match self {
            FileTransform {
                src,
                interpolation,
                direction_inverse,
            } => FileTransform {
                src: src,
                interpolation: interpolation,
                direction_inverse: !direction_inverse,
            },

            ColorSpaceTransform { src, dst } => ColorSpaceTransform { src: dst, dst: src },

            MatrixTransform(_) => todo!(),

            BuiltinTransform {
                name,
                direction_inverse,
            } => BuiltinTransform {
                name: name,
                direction_inverse: !direction_inverse,
            },

            AllocationTransform {
                allocation,
                vars,
                direction_inverse,
            } => AllocationTransform {
                allocation: allocation,
                vars: vars,
                direction_inverse: !direction_inverse,
            },
            RangeTransform {
                range_in,
                range_out,
                clamp,
            } => RangeTransform {
                range_in: range_out,
                range_out: range_in,
                clamp: clamp,
            },
            ExponentTransform(r, g, b, a) => ExponentTransform(1.0 / r, 1.0 / g, 1.0 / b, 1.0 / a),
            ExponentWithLinearTransform {
                gamma,
                offset,
                direction_inverse,
            } => ExponentWithLinearTransform {
                gamma: gamma,
                offset: offset,
                direction_inverse: !direction_inverse,
            },

            ToHSV => FromHSV,
            FromHSV => ToHSV,
            ACESGamutMapTransform {
                threshhold,
                limit,
                power,
                direction_inverse,
            } => ACESGamutMapTransform {
                threshhold: threshhold,
                limit: limit,
                power: power,
                direction_inverse: !direction_inverse,
            },
        }
    }
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
                matrix_string.push_str(&format!("{}", n));
            }
            format!("!<MatrixTransform> {{ matrix: [{}] }}", matrix_string)
        }
        &Transform::BuiltinTransform {
            ref name,
            direction_inverse,
        } => {
            format!(
                "!<BuiltinTransform> {{ style: {}{} }}",
                name,
                if direction_inverse {
                    ", direction: inverse"
                } else {
                    ""
                },
            )
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
        &Transform::RangeTransform {
            range_in,
            range_out,
            clamp,
        } => {
            format!(
                "!<RangeTransform> {{ min_in_value: {}, max_in_value: {}, min_out_value: {}, max_out_value: {}, style: {} }}",
                range_in.0,
                range_in.1,
                range_out.0,
                range_out.1,
                if clamp { "clamp" } else { "noClamp" },
            )
        }
        &Transform::ExponentTransform(r, g, b, a) => {
            format!(
                "!<ExponentTransform> {{ value: [{}, {}, {}, {}] }}",
                r, g, b, a,
            )
        }
        &Transform::ExponentWithLinearTransform {
            gamma,
            offset,
            direction_inverse,
        } => {
            format!(
                "!<ExponentWithLinearTransform> {{ gamma: [{0}, {0}, {0}, 1.0], offset: [{1}, {1}, {1}, 0.0], style: linear{2} }}",
                gamma,
                offset,
                if direction_inverse {
                    ", direction: inverse"
                } else {
                    ""
                },
            )
        }
        &Transform::ToHSV => "!<FixedFunctionTransform> { style: RGB_TO_HSV }".into(),
        &Transform::FromHSV => {
            "!<FixedFunctionTransform> { style: RGB_TO_HSV, direction: inverse }".into()
        }

        &Transform::ACESGamutMapTransform {
            threshhold,
            limit,
            power,
            direction_inverse,
        } => {
            format!(
                "!<FixedFunctionTransform> {{ style: ACES_GamutComp13, params: [{}, {}, {}, {}, {}, {}, {}]{} }}",
                limit[0],
                limit[0],
                limit[0],
                threshhold[0],
                threshhold[0],
                threshhold[0],
                power,
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
    } else if transforms.len() > 1 {
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
    Raw(Vec<u8>),
    Lut1D(Lut1D),
    Lut3D(Lut3D),
}

/// Returns true if `g1` is fully encompassed by `g2`.
fn gamut_is_within_gamut(g1: Chromaticities, g2: Chromaticities) -> bool {
    fn sign(pa: (f64, f64), pb1: (f64, f64), pb2: (f64, f64)) -> f64 {
        (pa.0 - pb2.0) * (pb1.1 - pb2.1) - (pb1.0 - pb2.0) * (pa.1 - pb2.1)
    }

    fn point_in_triangle(pt: (f64, f64), v1: (f64, f64), v2: (f64, f64), v3: (f64, f64)) -> bool {
        let d1 = sign(pt, v1, v2);
        let d2 = sign(pt, v2, v3);
        let d3 = sign(pt, v3, v1);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

        !(has_neg && has_pos)
    }

    point_in_triangle(g1.r, g2.r, g2.g, g2.b)
        && point_in_triangle(g1.g, g2.r, g2.g, g2.b)
        && point_in_triangle(g1.b, g2.r, g2.g, g2.b)
        && point_in_triangle(g1.w, g2.r, g2.g, g2.b)
}

#[derive(Debug, Copy, Clone)]
pub struct ExponentLUTMapper {
    to_linear_exp: f64,
    linear_max: f64,
    channels: [bool; 3],
    use_tetrahedral_interpolation: bool,
}

impl ExponentLUTMapper {
    /// - `to_linear_exp`: the exponent to use for mapping, in the
    ///   to-linear direction.
    /// - `linear_max`: the linear value that the max lut point should map to.
    /// - `channels`: which channels the mapping is used for (the rest
    ///   are left alone).
    pub fn new(
        to_linear_exp: f64,
        linear_max: f64,
        channels: [bool; 3],
        use_tetrahedral_interpolation: bool,
    ) -> Self {
        Self {
            to_linear_exp: to_linear_exp,
            linear_max: linear_max,
            channels: channels,
            use_tetrahedral_interpolation: use_tetrahedral_interpolation,
        }
    }

    /// The upper bound of the lut range (in lut space).
    pub fn lut_max(&self) -> f64 {
        1.0
    }

    /// The max linear value in the resulting lut.
    pub fn linear_max(&self) -> f64 {
        self.linear_max
    }

    /// Converts a value from lut space to linear.
    pub fn from_lut(&self, xyz: [f64; 3]) -> [f64; 3] {
        [
            if self.channels[0] {
                xyz[0].powf(self.to_linear_exp) * self.linear_max
            } else {
                xyz[0]
            },
            if self.channels[1] {
                xyz[1].powf(self.to_linear_exp) * self.linear_max
            } else {
                xyz[1]
            },
            if self.channels[2] {
                xyz[2].powf(self.to_linear_exp) * self.linear_max
            } else {
                xyz[2]
            },
        ]
    }

    /// Converts a value to lut space from linear.
    pub fn to_lut(&self, xyz: [f64; 3]) -> [f64; 3] {
        [
            if self.channels[0] {
                (xyz[0] / self.linear_max).powf(1.0 / self.to_linear_exp)
            } else {
                xyz[0]
            },
            if self.channels[1] {
                (xyz[1] / self.linear_max).powf(1.0 / self.to_linear_exp)
            } else {
                xyz[1]
            },
            if self.channels[2] {
                (xyz[2] / self.linear_max).powf(1.0 / self.to_linear_exp)
            } else {
                xyz[2]
            },
        ]
    }

    /// Generates the transforms to apply a 3D lut with this mapper.
    #[rustfmt::skip]
    pub fn transforms_lut_3d(&self, lut_3d_path: &str) -> Vec<Transform> {
        vec![
            // To LUT space.
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                if self.channels[0] { 1.0 / self.linear_max } else { 1.0 },
                if self.channels[1] { 1.0 / self.linear_max } else { 1.0 },
                if self.channels[2] { 1.0 / self.linear_max } else { 1.0 },
            ]))),
            Transform::ExponentTransform(
                if self.channels[0] { 1.0 / self.to_linear_exp } else { 1.0 },
                if self.channels[1] { 1.0 / self.to_linear_exp } else { 1.0 },
                if self.channels[2] { 1.0 / self.to_linear_exp } else { 1.0 },
                1.0,
            ),

            // Apply LUT.
            Transform::FileTransform {
                src: lut_3d_path.into(),
                interpolation: if self.use_tetrahedral_interpolation {
                    Interpolation::Tetrahedral
                } else {
                    Interpolation::Linear
                },
                direction_inverse: false,
            },

            // From LUT space.
            Transform::ExponentTransform(
                if self.channels[0] { self.to_linear_exp } else { 1.0 },
                if self.channels[1] { self.to_linear_exp } else { 1.0 },
                if self.channels[2] { self.to_linear_exp } else { 1.0 },
                1.0,
            ),
            Transform::MatrixTransform(matrix::to_4x4_f32(matrix::scale_matrix([
                if self.channels[0] { self.linear_max } else { 1.0 },
                if self.channels[1] { self.linear_max } else { 1.0 },
                if self.channels[2] { self.linear_max } else { 1.0 },
            ]))),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use colorbox::chroma;

    #[test]
    fn gamut_is_within_gamut_01() {
        assert_eq!(gamut_is_within_gamut(chroma::REC709, chroma::REC2020), true);
        assert_eq!(
            gamut_is_within_gamut(chroma::REC2020, chroma::REC709),
            false
        );
        assert_eq!(gamut_is_within_gamut(chroma::REC709, chroma::REC709), true);
        assert_eq!(
            gamut_is_within_gamut(chroma::REC2020, chroma::REC2020),
            true
        );
    }
}
