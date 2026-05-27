use crate::bitmap::BitmapStore;
use crate::catalog::Catalog;
use anyhow::{Context, Result};
use std::path::Path;

pub struct QueryResult {
    pub id_gd: u32,
    pub cardinality: u64,
    pub references: Vec<String>,
}

pub fn query_id_gd(
    index_dir: &Path,
    set: &str,
    id_gd: u32,
    list_limit: Option<usize>,
) -> Result<QueryResult> {
    let set_dir = index_dir.join(set);
    let catalog = Catalog::load(&set_dir.join("catalog.json"))?;
    let bitmap_path = set_dir.join("id_gd").join(format!("{id_gd}.roar"));
    let bitmap = BitmapStore::load(id_gd, &bitmap_path)
        .with_context(|| format!("idGd {id_gd} not found at {}", bitmap_path.display()))?;

    let cardinality = bitmap.len();
    let mut references = Vec::new();
    if let Some(limit) = list_limit {
        for bit in bitmap.iter().take(limit) {
            references.push(catalog.decode_bit(bit)?.reference);
        }
    }

    Ok(QueryResult {
        id_gd,
        cardinality,
        references,
    })
}
