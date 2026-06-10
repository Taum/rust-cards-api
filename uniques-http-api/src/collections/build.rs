use anyhow::Result;
use index_core::build_bitmap_from_ref_strs;
use index_core::catalog::Catalog;
use roaring::RoaringBitmap;

pub fn build_collection_bitmap(
    catalog: &Catalog,
    refs: &[String],
    total_bit_span: u32,
) -> Result<(u64, RoaringBitmap)> {
    let ref_slices: Vec<&str> = refs.iter().map(String::as_str).collect();
    let bitmap = build_bitmap_from_ref_strs(catalog, &ref_slices)?;
    index_core::validate_bitmap_span(&bitmap, total_bit_span)?;
    Ok((bitmap.len(), bitmap))
}
