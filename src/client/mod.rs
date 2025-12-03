use std::collections::{HashMap, VecDeque};

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{
    error::ApiErrorBody,
    models::{EntryType, LicenseInfo, RepoInfo, RepoSummary, SearchResult, Stats, TreeEntry},
};

#[derive(Clone)]
pub struct GithubClient {
    http: Client,
    base_url: Url,
    token: Option<String>,
}

impl GithubClient {
    pub fn new(api_base: String, token: Option<String>) -> anyhow::Result<Self> {
        let base_url =
            Url::parse(api_base.trim_end_matches('/')).context("Invalid GitHub API base URL")?;

        let http = Client::builder()
            .user_agent(format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url,
            token,
        })
    }

    pub async fn get_repo(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<RepoInfo>, ApiErrorBody> {
        let url = self.build_url(&["repos", owner, repo])?;
        let response = self
            .base_request(url, None)
            .send()
            .await
            .map_err(ApiErrorBody::from_reqwest)?;

        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(ApiErrorBody::from_response(status, response).await);
        }

        let repo: GithubRepo = response
            .json()
            .await
            .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;
        Ok(Some(repo.into()))
    }

    pub async fn list_tags(&self, owner: &str, repo: &str) -> Result<Vec<String>, ApiErrorBody> {
        let url = self.build_url(&["repos", owner, repo, "tags"])?;
        self.get_collection::<GithubTag>(url).await
    }

    pub async fn list_branches(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<String>, ApiErrorBody> {
        let url = self.build_url(&["repos", owner, repo, "branches"])?;
        self.get_collection::<GithubBranch>(url).await
    }

    pub async fn list_repos(
        &self,
        owner: &str,
        page: Option<usize>,
        per_page: Option<usize>,
    ) -> Result<Vec<RepoSummary>, ApiErrorBody> {
        let mut last_err: Option<ApiErrorBody> = None;

        for base in ["users", "orgs"] {
            let url = self.build_url(&[base, owner, "repos"])?;

            let mut request = self.base_request(url, None);

            if let Some(page) = page {
                request = request.query(&[("page", &page.to_string())]);
            }

            if let Some(per_page) = per_page {
                request = request.query(&[("per_page", &per_page.to_string())]);
            }

            let response = request.send().await.map_err(ApiErrorBody::from_reqwest)?;

            let status = response.status();

            if status == StatusCode::NOT_FOUND {
                last_err = Some(ApiErrorBody::from_response(status, response).await);
                continue;
            }

            if !status.is_success() {
                return Err(ApiErrorBody::from_response(status, response).await);
            }

            let repos: Vec<GithubRepoSummary> = response
                .json()
                .await
                .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;

            return Ok(repos.into_iter().map(Into::into).collect());
        }

        Err(last_err.unwrap_or_else(|| {
            ApiErrorBody::new("Failed to list repositories for the requested owner.", "0")
        }))
    }

    pub async fn search_code(
        &self,
        query: &str,
        page: Option<usize>,
        per_page: Option<usize>,
    ) -> Result<Vec<SearchResult>, ApiErrorBody> {
        let url = self.build_url(&["search", "code"])?;

        let mut request = self.base_request(url, None).query(&[("q", query)]);

        if let Some(page) = page {
            request = request.query(&[("page", &page.to_string())]);
        }

        if let Some(per_page) = per_page {
            request = request.query(&[("per_page", &per_page.to_string())]);
        }

        let response = request.send().await.map_err(ApiErrorBody::from_reqwest)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ApiErrorBody::from_response(status, response).await);
        }

        let body: GithubSearchResponse = response
            .json()
            .await
            .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;

        Ok(body.items.into_iter().map(Into::into).collect())
    }

    pub async fn tree(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        depth: usize,
        r#ref: Option<&str>,
    ) -> Result<Vec<TreeEntry>, ApiErrorBody> {
        let contents = self.fetch_contents(owner, repo, path, r#ref).await?;

        let root_parent = match &contents {
            GithubContents::Directory(_) => normalize_root_path(path),
            GithubContents::File(file) => parent_path(&file.path),
        };

        self.expand_tree(owner, repo, contents, depth, r#ref, &root_parent)
            .await
    }

    pub async fn get_stats(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        r#ref: Option<&str>,
    ) -> Result<Stats, ApiErrorBody> {
        let normalized_path = normalize_root_path(path);
        let contents = self.fetch_contents(owner, repo, path, r#ref).await?;

        match contents {
            GithubContents::File(file) => Ok(file.into_stats()),
            GithubContents::Directory(_) => Ok(directory_stats(&normalized_path)),
        }
    }

    pub async fn get_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        r#ref: Option<&str>,
    ) -> Result<String, ApiErrorBody> {
        let contents = self.fetch_contents(owner, repo, path, r#ref).await?;

        match contents {
            GithubContents::File(file) => {
                let encoding = file.encoding.unwrap_or_else(|| "base64".to_string());
                if encoding != "base64" {
                    return Err(ApiErrorBody::new(
                        format!("Unsupported encoding: {}", encoding),
                        "0",
                    ));
                }

                let payload = file
                    .content
                    .ok_or_else(|| ApiErrorBody::new("File content missing", "0"))?;

                let decoded = STANDARD
                    .decode(payload.replace('\n', ""))
                    .map_err(|err| ApiErrorBody::new(err.to_string(), "0"))?;

                String::from_utf8(decoded).map_err(|err| ApiErrorBody::new(err.to_string(), "0"))
            }
            GithubContents::Directory(_) => Err(ApiErrorBody::new(
                "Requested path is a directory, not a file.",
                "400",
            )),
        }
    }

    fn build_url(&self, segments: &[&str]) -> Result<Url, ApiErrorBody> {
        let mut url = self.base_url.clone();
        {
            let mut parts = url
                .path_segments_mut()
                .map_err(|_| ApiErrorBody::new("API base URL is not valid for paths", "0"))?;
            parts.pop_if_empty();
            for segment in segments {
                parts.push(segment);
            }
        }
        Ok(url)
    }

    fn base_request(&self, url: Url, r#ref: Option<&str>) -> reqwest::RequestBuilder {
        let mut builder = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");

        if let Some(token) = &self.token {
            builder = builder.bearer_auth(token);
        }

        if let Some(r#ref) = r#ref {
            builder = builder.query(&[("ref", r#ref)]);
        }

        builder
    }

    async fn get_collection<T>(&self, url: Url) -> Result<Vec<String>, ApiErrorBody>
    where
        T: NamedItem + DeserializeOwned,
    {
        let response = self
            .base_request(url, None)
            .send()
            .await
            .map_err(ApiErrorBody::from_reqwest)?;
        let status = response.status();

        if !status.is_success() {
            return Err(ApiErrorBody::from_response(status, response).await);
        }

        let items: Vec<T> = response
            .json()
            .await
            .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;

        Ok(items.into_iter().map(|item| item.name()).collect())
    }

    async fn fetch_contents(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        r#ref: Option<&str>,
    ) -> Result<GithubContents, ApiErrorBody> {
        let mut segments = vec![
            "repos".to_string(),
            owner.to_string(),
            repo.to_string(),
            "contents".to_string(),
        ];
        segments.extend(
            path.split('/')
                .filter(|s| !s.is_empty())
                .map(|p| p.to_string()),
        );

        let url = self.build_url(&segments.iter().map(String::as_str).collect::<Vec<_>>())?;

        let response = self
            .base_request(url, r#ref)
            .send()
            .await
            .map_err(ApiErrorBody::from_reqwest)?;
        let status = response.status();

        if !status.is_success() {
            return Err(ApiErrorBody::from_response(status, response).await);
        }

        let body = response
            .text()
            .await
            .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;

        if value.is_array() {
            let entries: Vec<GithubDirectoryEntry> = serde_json::from_value(value)
                .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;
            Ok(GithubContents::Directory(entries))
        } else {
            let file: GithubFile = serde_json::from_value(value)
                .map_err(|err| ApiErrorBody::new(err.to_string(), status.as_u16()))?;
            Ok(GithubContents::File(file))
        }
    }

    async fn expand_tree(
        &self,
        owner: &str,
        repo: &str,
        contents: GithubContents,
        depth: usize,
        r#ref: Option<&str>,
        root_parent: &str,
    ) -> Result<Vec<TreeEntry>, ApiErrorBody> {
        let mut queue: VecDeque<(GithubContents, usize)> = VecDeque::new();
        let mut children_by_parent: HashMap<String, Vec<TreeEntry>> = HashMap::new();

        queue.push_back((contents, depth));

        while let Some((node, remaining_depth)) = queue.pop_front() {
            match node {
                GithubContents::File(file) => {
                    let parent = parent_path(&file.path);
                    children_by_parent
                        .entry(parent)
                        .or_default()
                        .push(file.into_tree_entry(Vec::new()));
                }
                GithubContents::Directory(entries) => {
                    for entry in entries {
                        let parent = parent_path(&entry.path);
                        let is_dir = matches!(entry.r#type, GithubContentType::Dir);
                        let path = entry.path.clone();

                        children_by_parent
                            .entry(parent)
                            .or_default()
                            .push(entry.into_tree_entry(Vec::new()));

                        if is_dir && remaining_depth > 1 {
                            let nested_contents =
                                self.fetch_contents(owner, repo, &path, r#ref).await?;
                            queue.push_back((nested_contents, remaining_depth - 1));
                        }
                    }
                }
            }
        }

        Ok(assemble_tree(&mut children_by_parent, root_parent))
    }
}

#[derive(Debug, Deserialize)]
struct GithubRepo {
    description: Option<String>,
    stargazers_count: u64,
    forks_count: u64,
    license: Option<GithubLicense>,
}

#[derive(Debug, Deserialize)]
struct GithubLicense {
    key: Option<String>,
    name: Option<String>,
    spdx_id: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GithubBranch {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GithubSearchResponse {
    items: Vec<GithubSearchItem>,
}

#[derive(Debug, Deserialize)]
struct GithubSearchItem {
    name: String,
    path: String,
    repository: GithubSearchRepo,
}

#[derive(Debug, Deserialize)]
struct GithubSearchRepo {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct GithubRepoSummary {
    name: String,
    full_name: String,
    private: bool,
    html_url: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubFile {
    path: String,
    #[serde(rename = "type")]
    r#type: GithubContentType,
    size: Option<u64>,
    content: Option<String>,
    encoding: Option<String>,
    target: Option<String>,
    submodule_git_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubDirectoryEntry {
    #[serde(rename = "name")]
    _name: String,
    path: String,
    #[serde(rename = "type")]
    r#type: GithubContentType,
    size: Option<u64>,
    target: Option<String>,
    submodule_git_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum GithubContentType {
    File,
    Dir,
    Symlink,
    Submodule,
}

impl GithubContentType {
    fn to_entry_type(&self) -> EntryType {
        match self {
            GithubContentType::Dir => EntryType::Dir,
            GithubContentType::File => EntryType::File,
            GithubContentType::Symlink => EntryType::Symlink,
            GithubContentType::Submodule => EntryType::Submodule,
        }
    }
}

fn entry_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn directory_stats(path: &str) -> Stats {
    let normalized = path.to_string();
    Stats {
        r#type: EntryType::Dir,
        name: entry_name(&normalized),
        path: normalized,
        size: None,
        target: None,
        submodule_git_url: None,
    }
}

fn parent_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_else(String::new)
}

fn normalize_root_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

impl GithubFile {
    fn into_tree_entry(self, children: Vec<TreeEntry>) -> TreeEntry {
        let r#type = self.r#type.to_entry_type();

        TreeEntry {
            r#type,
            name: entry_name(&self.path),
            path: self.path,
            size: match r#type {
                EntryType::Dir | EntryType::Submodule => None,
                _ => self.size,
            },
            target: self.target,
            submodule_git_url: self.submodule_git_url,
            children,
        }
    }

    fn into_stats(self) -> Stats {
        let r#type = self.r#type.to_entry_type();

        Stats {
            r#type,
            name: entry_name(&self.path),
            path: self.path,
            size: match r#type {
                EntryType::Dir | EntryType::Submodule => None,
                _ => self.size,
            },
            target: self.target,
            submodule_git_url: self.submodule_git_url,
        }
    }
}

impl GithubDirectoryEntry {
    fn into_tree_entry(self, children: Vec<TreeEntry>) -> TreeEntry {
        let r#type = self.r#type.to_entry_type();

        TreeEntry {
            r#type,
            name: entry_name(&self.path),
            path: self.path,
            size: match r#type {
                EntryType::Dir | EntryType::Submodule => None,
                _ => self.size,
            },
            target: self.target,
            submodule_git_url: self.submodule_git_url,
            children,
        }
    }
}

#[derive(Debug)]
enum GithubContents {
    File(GithubFile),
    Directory(Vec<GithubDirectoryEntry>),
}

trait NamedItem {
    fn name(self) -> String;
}

impl NamedItem for GithubTag {
    fn name(self) -> String {
        self.name
    }
}

impl NamedItem for GithubBranch {
    fn name(self) -> String {
        self.name
    }
}

impl From<GithubRepo> for RepoInfo {
    fn from(repo: GithubRepo) -> Self {
        RepoInfo {
            description: repo.description,
            stars: repo.stargazers_count,
            forks: repo.forks_count,
            license: repo.license.map(Into::into),
        }
    }
}

impl From<GithubLicense> for LicenseInfo {
    fn from(license: GithubLicense) -> Self {
        LicenseInfo {
            key: license.key,
            name: license.name,
            spdx_id: license.spdx_id,
            url: license.url,
        }
    }
}

impl From<GithubSearchItem> for SearchResult {
    fn from(item: GithubSearchItem) -> Self {
        SearchResult {
            name: item.name,
            path: item.path,
            repository: item.repository.full_name,
        }
    }
}

impl From<GithubRepoSummary> for RepoSummary {
    fn from(repo: GithubRepoSummary) -> Self {
        RepoSummary {
            name: repo.name,
            full_name: repo.full_name,
            private: repo.private,
            description: repo.description,
            html_url: repo.html_url,
        }
    }
}

fn assemble_tree(
    children_by_parent: &mut HashMap<String, Vec<TreeEntry>>,
    parent: &str,
) -> Vec<TreeEntry> {
    let mut entries = children_by_parent.remove(parent).unwrap_or_default();

    for entry in entries.iter_mut() {
        if matches!(entry.r#type, EntryType::Dir) {
            entry.children = assemble_tree(children_by_parent, &entry.path);
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn expands_symlink_and_submodule_entries() {
        let client = GithubClient::new("https://example.com".to_string(), None).unwrap();

        let contents = GithubContents::Directory(vec![
            GithubDirectoryEntry {
                _name: "link".to_string(),
                path: "link".to_string(),
                r#type: GithubContentType::Symlink,
                size: Some(12),
                target: Some("target/path".to_string()),
                submodule_git_url: None,
            },
            GithubDirectoryEntry {
                _name: "module".to_string(),
                path: "module".to_string(),
                r#type: GithubContentType::Submodule,
                size: None,
                target: None,
                submodule_git_url: Some("https://example.com/repo.git".to_string()),
            },
        ]);

        let entries = client
            .expand_tree("owner", "repo", contents, 1, None, "")
            .await
            .unwrap();

        assert_eq!(entries.len(), 2);

        let link = entries.iter().find(|entry| entry.name == "link").unwrap();
        assert!(matches!(link.r#type, EntryType::Symlink));
        assert_eq!(link.target.as_deref(), Some("target/path"));
        assert_eq!(link.size, Some(12));
        assert!(link.children.is_empty());

        let module = entries.iter().find(|entry| entry.name == "module").unwrap();
        assert!(matches!(module.r#type, EntryType::Submodule));
        assert_eq!(
            module.submodule_git_url.as_deref(),
            Some("https://example.com/repo.git")
        );
        assert!(module.size.is_none());
        assert!(module.children.is_empty());
    }

    #[tokio::test]
    async fn expands_top_level_symlink_file_entry() {
        let client = GithubClient::new("https://example.com".to_string(), None).unwrap();

        let contents = GithubContents::File(GithubFile {
            path: "link".to_string(),
            r#type: GithubContentType::Symlink,
            size: Some(3),
            content: None,
            encoding: None,
            target: Some("target".to_string()),
            submodule_git_url: None,
        });

        let entries = client
            .expand_tree("owner", "repo", contents, 1, None, "")
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert!(matches!(entries[0].r#type, EntryType::Symlink));
        assert_eq!(entries[0].target.as_deref(), Some("target"));
        assert_eq!(entries[0].size, Some(3));
        assert!(entries[0].children.is_empty());
    }

    #[test]
    fn converts_search_item() {
        let item = GithubSearchItem {
            name: "file.rs".to_string(),
            path: "src/file.rs".to_string(),
            repository: GithubSearchRepo {
                full_name: "octo/repo".to_string(),
            },
        };

        let result: SearchResult = item.into();

        assert_eq!(result.name, "file.rs");
        assert_eq!(result.path, "src/file.rs");
        assert_eq!(result.repository, "octo/repo");
    }

    #[test]
    fn converts_repo_summary() {
        let repo = GithubRepoSummary {
            name: "repo".to_string(),
            full_name: "octo/repo".to_string(),
            private: false,
            html_url: "https://github.com/octo/repo".to_string(),
            description: Some("cool repo".to_string()),
        };

        let summary: RepoSummary = repo.into();

        assert_eq!(summary.name, "repo");
        assert_eq!(summary.full_name, "octo/repo");
        assert!(!summary.private);
        assert_eq!(summary.html_url, "https://github.com/octo/repo");
        assert_eq!(summary.description.as_deref(), Some("cool repo"));
    }

    #[test]
    fn builds_stats_for_file() {
        let file = GithubFile {
            path: "dir/file.txt".to_string(),
            r#type: GithubContentType::File,
            size: Some(10),
            content: None,
            encoding: None,
            target: None,
            submodule_git_url: None,
        };

        let stats = file.into_stats();

        assert!(matches!(stats.r#type, EntryType::File));
        assert_eq!(stats.name, "file.txt");
        assert_eq!(stats.path, "dir/file.txt");
        assert_eq!(stats.size, Some(10));
        assert!(stats.target.is_none());
    }

    #[test]
    fn builds_stats_for_directory() {
        let stats = directory_stats("dir/sub");

        assert!(matches!(stats.r#type, EntryType::Dir));
        assert_eq!(stats.name, "sub");
        assert_eq!(stats.path, "dir/sub");
        assert!(stats.size.is_none());
        assert!(stats.target.is_none());
    }
}
