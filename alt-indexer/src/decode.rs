use crate::catalog::Catalog;
use anyhow::Result;
use std::path::Path;

pub fn decode_bit(catalog_path: &Path, bit: u32) -> Result<crate::catalog::DecodedCard> {
    let catalog = Catalog::load(catalog_path)?;
    catalog.decode_bit(bit)
}
