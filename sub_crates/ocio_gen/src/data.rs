// LUTs we don't know how to compute, so we include them compressed
// in the executable.
pub const DCI_XYZ_SPI1D_XZ: &[u8] = include_bytes!("../data/blender/dci_xyz.spi1d.xz");
pub const LG10_SPI1D_XZ: &[u8] = include_bytes!("../data/blender/lg10.spi1d.xz");
pub const FILMIC_DESAT65CUBE_SPI3D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_desat65cube.spi3d.xz");
pub const FILMIC_DESAT_33_CUBE_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_desat_33.cube.xz");
pub const FILMIC_FALSE_COLOR_SPI3D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_false_color.spi3d.xz");
pub const FILMIC_TO_0_35_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-35_1-30.spi1d.xz");
pub const FILMIC_TO_0_48_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-48_1-09.spi1d.xz");
pub const FILMIC_TO_0_60_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-60_1-04.spi1d.xz");
pub const FILMIC_TO_0_70_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-70_1-03.spi1d.xz");
pub const FILMIC_TO_0_85_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0-85_1-011.spi1d.xz");
pub const FILMIC_TO_099_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_0.99_1-0075.spi1d.xz");
pub const FILMIC_TO_120_SPI1D_XZ: &[u8] =
    include_bytes!("../data/blender/filmic_to_1.20_1-00.spi1d.xz");
