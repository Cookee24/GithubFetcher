use std::collections::HashSet;

use rmcp::{
    ServerHandler,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::{
    cli::ToolSelection,
    client::GithubClient,
    error::ApiErrorBody,
    models::{
        BranchesResponse, FileResponse, GetFileArgs, LineRange, ListReposArgs, RepoArgs,
        RepoResponse, ReposResponse, SearchArgs, SearchResponse, StatsArgs, StatsResponse,
        TagsResponse, TreeArgs, TreeResponse,
    },
};

#[derive(Clone)]
pub struct GithubServer {
    pub client: GithubClient,
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl GithubServer {
    pub fn new(client: GithubClient, allowed_tools: HashSet<ToolSelection>) -> Self {
        let mut server = Self {
            client,
            tool_router: Self::tool_router(),
        };

        for tool in ToolSelection::ALL {
            if !allowed_tools.contains(&tool) {
                server.tool_router.remove_route(tool.as_str());
            }
        }

        server
    }

    #[tool(name = "get_repo", description = "Fetch repository metadata.")]
    async fn get_repo(
        &self,
        Parameters(args): Parameters<RepoArgs>,
    ) -> Result<Json<RepoResponse>, ApiErrorBody> {
        let repo = self.client.get_repo(&args.owner, &args.repo).await?;

        Ok(Json(RepoResponse { repo }))
    }

    #[tool(name = "list_tags", description = "List all tags for a repository.")]
    async fn list_tags(
        &self,
        Parameters(args): Parameters<RepoArgs>,
    ) -> Result<Json<TagsResponse>, ApiErrorBody> {
        let tags = self.client.list_tags(&args.owner, &args.repo).await?;
        Ok(Json(TagsResponse { tags }))
    }

    #[tool(
        name = "list_branches",
        description = "List all branches for a repository."
    )]
    async fn list_branches(
        &self,
        Parameters(args): Parameters<RepoArgs>,
    ) -> Result<Json<BranchesResponse>, ApiErrorBody> {
        let branches = self.client.list_branches(&args.owner, &args.repo).await?;
        Ok(Json(BranchesResponse { branches }))
    }

    #[tool(
        name = "list_repos",
        description = "List repositories for a user or organization."
    )]
    async fn list_repos(
        &self,
        Parameters(args): Parameters<ListReposArgs>,
    ) -> Result<Json<ReposResponse>, ApiErrorBody> {
        let repos = self
            .client
            .list_repos(&args.owner, args.page, args.per_page)
            .await?;

        Ok(Json(ReposResponse { repos }))
    }

    #[tool(name = "tree", description = "List files and folders under a path.")]
    async fn tree(
        &self,
        Parameters(args): Parameters<TreeArgs>,
    ) -> Result<Json<TreeResponse>, ApiErrorBody> {
        let depth = args.depth.max(1);
        let r#ref = args.r#ref.as_deref();
        let entries = self
            .client
            .tree(
                &args.owner,
                &args.repo,
                args.path.as_deref().unwrap_or_default(),
                depth,
                r#ref,
            )
            .await?;

        Ok(Json(TreeResponse { entries }))
    }

    #[tool(
        name = "get_file",
        description = "Fetch and decode the contents of a file."
    )]
    async fn get_file(
        &self,
        Parameters(args): Parameters<GetFileArgs>,
    ) -> Result<Json<FileResponse>, ApiErrorBody> {
        let r#ref = args.r#ref.as_deref();

        let content = self
            .client
            .get_file(&args.owner, &args.repo, &args.path, r#ref)
            .await?;

        let content = apply_content_limits(&content, args.line_range, args.max_chars);

        Ok(Json(FileResponse { content }))
    }

    #[tool(
        name = "search",
        description = "Search code across GitHub. Qualifiers: in:file|path, language:<lang>, repo:<owner/repo>, user:<user>, org:<org>, enterprise:<enterprise>, size:<range>, filename:<glob>, extension:<ext>."
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<SearchResponse>, ApiErrorBody> {
        let results = self
            .client
            .search_code(&args.query, args.page, args.per_page)
            .await?;

        Ok(Json(SearchResponse { results }))
    }

    #[tool(
        name = "get_stats",
        description = "Get metadata for a file, folder, submodule, or symlink."
    )]
    async fn get_stats(
        &self,
        Parameters(args): Parameters<StatsArgs>,
    ) -> Result<Json<StatsResponse>, ApiErrorBody> {
        let r#ref = args.r#ref.as_deref();
        let item = self
            .client
            .get_stats(&args.owner, &args.repo, &args.path, r#ref)
            .await?;

        Ok(Json(StatsResponse { item }))
    }
}

#[tool_handler]
impl ServerHandler for GithubServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "GitHub crawler: query repository metadata, tags, branches, trees, and file contents."
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }
}

fn apply_content_limits(
    content: &str,
    line_range: Option<LineRange>,
    max_chars: Option<usize>,
) -> String {
    let mut output: String = match max_chars {
        Some(limit) => content.chars().take(limit).collect(),
        None => content.to_string(),
    };

    if let Some(range) = line_range {
        output = match range {
            crate::models::LineRange::End(end) => slice_lines(&output, 1, end),
            crate::models::LineRange::Range([start, end]) => slice_lines(&output, start, end),
        };
    }

    output
}

fn slice_lines(content: &str, start: usize, end: usize) -> String {
    if start == 0 || end == 0 || end < start {
        return String::new();
    }

    let mut limited = String::new();

    for (idx, line) in content.split_inclusive('\n').enumerate() {
        let line_no = idx + 1;
        if line_no < start {
            continue;
        }
        if line_no > end {
            break;
        }
        limited.push_str(line);
    }

    // When the content lacks a trailing newline, split_inclusive will still return
    // the final segment without a newline, which we already appended above.
    limited
}

#[cfg(test)]
mod tests {
    use super::apply_content_limits;
    use crate::models::LineRange;

    #[test]
    fn enforces_character_limit_without_splitting_codepoints() {
        let content = "héllo";
        let limited = apply_content_limits(content, None, Some(3));

        assert_eq!(limited, "hél");
    }

    #[test]
    fn trims_to_requested_number_of_lines() {
        let content = "one\ntwo\nthree\nfour\n";
        let limited = apply_content_limits(content, Some(LineRange::End(2)), None);

        assert_eq!(limited, "one\ntwo\n");
    }

    #[test]
    fn applies_both_limits_when_set() {
        let content = "1\n2\n3\n4";
        let limited = apply_content_limits(content, Some(LineRange::End(2)), Some(5));

        assert_eq!(limited, "1\n2\n");
    }

    #[test]
    fn returns_empty_when_line_limit_is_zero() {
        let content = "content";
        let limited = apply_content_limits(content, Some(LineRange::End(0)), Some(10));

        assert_eq!(limited, "");
    }

    #[test]
    fn trims_to_line_range() {
        let content = "a\nb\nc\nd\n";
        let limited = apply_content_limits(content, Some(LineRange::Range([2, 3])), None);

        assert_eq!(limited, "b\nc\n");
    }
}
