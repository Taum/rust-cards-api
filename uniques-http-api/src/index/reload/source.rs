use anyhow::Result;

use crate::index::UniquesIndex;

pub trait IndexSource: Send + Sync + Clone {
    fn read_version(&self) -> Result<u64>;
    fn load_index(&self) -> Result<UniquesIndex>;
}
