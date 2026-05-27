use anyhow::Context;
use anyhow::Result;
use roaring::RoaringBitmap;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub struct BitmapStore {
    map: BTreeMap<u32, RoaringBitmap>,
}

impl BitmapStore {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, id_gd: u32, card_index: u32) {
        self.map
            .entry(id_gd)
            .or_default()
            .insert(card_index);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &RoaringBitmap)> {
        self.map.iter()
    }

    /// Write bitmap files; returns serialized byte size per `idGd`.
    pub fn write_dir(&self, dir: &Path) -> Result<BTreeMap<u32, u64>> {
        fs::create_dir_all(dir)?;
        let mut sizes = BTreeMap::new();
        for (id_gd, bitmap) in &self.map {
            let path = dir.join(format!("{id_gd}.roar"));
            let mut bytes = Vec::new();
            bitmap
                .serialize_into(&mut bytes)
                .with_context(|| format!("serialize idGd {id_gd}"))?;
            let size = bytes.len() as u64;
            fs::write(&path, bytes)?;
            sizes.insert(*id_gd, size);
        }
        Ok(sizes)
    }

    pub fn load(id_gd: u32, path: &Path) -> Result<RoaringBitmap> {
        let bytes = fs::read(path)?;
        RoaringBitmap::deserialize_from(&bytes[..])
            .with_context(|| format!("deserialize idGd {id_gd} from {}", path.display()))
    }
}

impl Default for BitmapStore {
    fn default() -> Self {
        Self::new()
    }
}
