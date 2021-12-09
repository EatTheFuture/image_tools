pub mod blender_config;
pub mod config;

/// Helper function to decompress in-memory xz-compressed data.
fn decompress_xz(data: &[u8]) -> Vec<u8> {
    let mut decompressed_data = std::io::Cursor::new(Vec::new());
    lzma_rs::xz_decompress(&mut std::io::Cursor::new(data), &mut decompressed_data).unwrap();
    decompressed_data.into_inner()
}
