use anyhow::Result;

use crate::index::loader::{load_uniques_index_from, ObjectStoreIndexClient, TarZstIndexStorage};
use crate::index::UniquesIndex;

use super::source::IndexSource;

#[derive(Clone)]
pub struct RemoteIndexSource {
    client: ObjectStoreIndexClient,
}

impl RemoteIndexSource {
    pub fn new(client: ObjectStoreIndexClient) -> Self {
        Self { client }
    }
}

impl IndexSource for RemoteIndexSource {
    fn read_version(&self) -> Result<u64> {
        self.client.read_version_sync()
    }

    fn load_index(&self) -> Result<UniquesIndex> {
        let fetch = self.client.fetch_version_sync()?;
        let bytes = self.client.fetch_archive_bytes_sync(&fetch.sidecar)?;
        let storage =
            TarZstIndexStorage::from_bytes(&bytes, self.client.archive_source_label())?;
        load_uniques_index_from(&storage)
    }
}
