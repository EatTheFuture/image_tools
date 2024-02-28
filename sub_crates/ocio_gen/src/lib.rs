pub mod blender_3_config;
pub mod blender_4_config;
pub mod config;
pub mod minimal_config;

mod agx;
mod bezier;
mod data;
mod gamut_map;
mod hsv_lut;
mod tone_map;

/// Helper function to decompress in-memory xz-compressed data.
fn decompress_xz(data: &[u8]) -> Vec<u8> {
    let mut decompressed_data = std::io::Cursor::new(Vec::new());
    lzma_rs::xz_decompress(&mut std::io::Cursor::new(data), &mut decompressed_data).unwrap();
    decompressed_data.into_inner()
}

/// Ensures that a directory path exists and that we have permission to
/// write to it.  If it doesn't exists, this will attempt to create it.
///
/// Will return an error if:
/// - The path exists, but is not a directory.
/// - The path exists, but we don't have permission to write to it.
/// - The path doesn't exist, and we are unable to create it.
fn ensure_dir_exists(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    } else {
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Specified path is not a directory",
            ));
        }
        if metadata.permissions().readonly() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Specified path is read only",
            ));
        }
    }
    Ok(())
}
