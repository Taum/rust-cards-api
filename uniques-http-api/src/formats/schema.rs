use serde::Deserialize;

pub const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct FormatsManifestEntry {
    pub id: String,
    pub path: String,
    pub version: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FormatDefinition {
    pub id: String,
    pub version: u64,
    #[serde(default)]
    pub included_refs: Vec<String>,
    #[serde(default)]
    pub excluded_sets: Vec<String>,
    #[serde(default)]
    pub excluded_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    Include,
    Exclude,
}

impl FormatDefinition {
    pub fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty() || id.len() > 32 {
            return Err(format!("format id must be 1-32 characters, got {}", id.len()));
        }
        Ok(())
    }

    pub fn mode(&self) -> Result<FormatMode, String> {
        let has_include = !self.included_refs.is_empty();
        let has_exclude = !self.excluded_sets.is_empty() || !self.excluded_refs.is_empty();
        match (has_include, has_exclude) {
            (true, true) => Err("included_refs cannot coexist with excluded_sets/excluded_refs".into()),
            (true, false) => Ok(FormatMode::Include),
            (false, true) => Ok(FormatMode::Exclude),
            (false, false) => Err("format must specify included_refs or excluded_sets/excluded_refs".into()),
        }
    }

    pub fn cross_check_manifest(&self, entry: &FormatsManifestEntry) -> Result<(), String> {
        Self::validate_id(&self.id)?;
        if self.id != entry.id {
            return Err(format!(
                "format id {:?} does not match manifest id {:?}",
                self.id, entry.id
            ));
        }
        if self.version != entry.version {
            return Err(format!(
                "format version {} does not match manifest version {}",
                self.version, entry.version
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_include_vs_exclude() {
        let include = FormatDefinition {
            id: "a".into(),
            version: 1,
            included_refs: vec!["r".into()],
            ..Default::default()
        };
        assert_eq!(include.mode().unwrap(), FormatMode::Include);

        let exclude = FormatDefinition {
            id: "b".into(),
            version: 1,
            excluded_sets: vec!["CORE".into()],
            ..Default::default()
        };
        assert_eq!(exclude.mode().unwrap(), FormatMode::Exclude);

        let both = FormatDefinition {
            id: "c".into(),
            version: 1,
            included_refs: vec!["r".into()],
            excluded_sets: vec!["CORE".into()],
            ..Default::default()
        };
        assert!(both.mode().is_err());
    }
}
