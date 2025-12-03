use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoArgs {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatsArgs {
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TreeArgs {
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default = "default_depth")]
    pub depth: usize,
    pub r#ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileArgs {
    pub owner: String,
    pub repo: String,
    pub path: String,
    pub r#ref: Option<String>,
    #[serde(default)]
    pub line_range: Option<LineRange>,
    #[serde(default)]
    pub max_chars: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoInfo {
    pub description: Option<String>,
    pub stars: u64,
    pub forks: u64,
    pub license: Option<LicenseInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepoResponse {
    pub repo: Option<RepoInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LicenseInfo {
    pub key: Option<String>,
    pub name: Option<String>,
    pub spdx_id: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TreeEntry {
    pub r#type: EntryType,
    /// Base name of the entry (no parent path).
    pub name: String,
    /// Full path relative to the repository root, kept internal for tree assembly.
    #[serde(skip)]
    #[schemars(skip)]
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule_git_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TreeResponse {
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatsResponse {
    pub item: Stats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum EntryType {
    File,
    Dir,
    Symlink,
    Submodule,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
#[serde(untagged)]
pub enum LineRange {
    /// A single number N means keep lines 1..=N.
    End(usize),
    /// Two numbers [start, end] mean keep lines start..=end.
    Range([usize; 2]),
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileResponse {
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Stats {
    #[serde(rename = "type")]
    pub r#type: EntryType,
    /// Base name of the entry (no parent path).
    pub name: String,
    /// Full path relative to the repository root.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule_git_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListReposArgs {
    pub owner: String,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub repository: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepoSummary {
    pub name: String,
    pub full_name: String,
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReposResponse {
    pub repos: Vec<RepoSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TagsResponse {
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchesResponse {
    pub branches: Vec<String>,
}

pub fn default_depth() -> usize {
    1
}
