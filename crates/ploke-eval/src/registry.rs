#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetRegistryEntry {
    pub key: &'static str,
    pub url: &'static str,
    pub org: &'static str,
    pub repo: &'static str,
    pub language: &'static str,
    pub filename: &'static str,
}

const BUILTIN_DATASET_REGISTRY: &[DatasetRegistryEntry] = &[DatasetRegistryEntry {
    key: "ripgrep",
    url: "https://huggingface.co/datasets/ByteDance-Seed/Multi-SWE-bench/resolve/main/rust/BurntSushi__ripgrep_dataset.jsonl",
    org: "BurntSushi",
    repo: "ripgrep",
    language: "rust",
    filename: "BurntSushi__ripgrep_dataset.jsonl",
}];

pub fn builtin_dataset_registry_entries() -> &'static [DatasetRegistryEntry] {
    BUILTIN_DATASET_REGISTRY
}

pub fn builtin_dataset_registry_entry(key: &str) -> Option<&'static DatasetRegistryEntry> {
    BUILTIN_DATASET_REGISTRY
        .iter()
        .find(|entry| entry.key == key)
}

impl DatasetRegistryEntry {
    pub fn clone_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.org, self.repo)
    }
}
