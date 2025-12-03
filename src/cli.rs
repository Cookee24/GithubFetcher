use std::{collections::HashSet, env};

use clap::{Parser, ValueEnum};

/// Command-line arguments for configuring the MCP server.
#[derive(Parser, Debug)]
#[command(
    name = "github-fetcher-mcp",
    about = "MCP server for crawling GitHub code over stdio"
)]
pub struct Args {
    /// GitHub API base URL, defaults to the public API.
    #[arg(long, default_value = "https://api.github.com")]
    pub api_base: String,

    /// Personal access token to authenticate with GitHub.
    #[arg(long)]
    pub token: Option<String>,

    /// Environment variable name to read the GitHub token from when --token is not provided.
    #[arg(long, default_value = "GITHUB_AUTH_TOKEN")]
    pub token_env: String,

    /// Restrict which tools are exposed; defaults to all.
    #[arg(long, value_enum, value_delimiter = ',', num_args = 1..)]
    pub tools: Option<Vec<ToolSelection>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum ToolSelection {
    GetRepo,
    ListTags,
    ListBranches,
    Tree,
    GetFile,
    ListRepos,
    Search,
    GetStats,
}

impl ToolSelection {
    pub const ALL: [ToolSelection; 8] = [
        ToolSelection::GetRepo,
        ToolSelection::ListTags,
        ToolSelection::ListBranches,
        ToolSelection::Tree,
        ToolSelection::GetFile,
        ToolSelection::ListRepos,
        ToolSelection::Search,
        ToolSelection::GetStats,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ToolSelection::GetRepo => "get_repo",
            ToolSelection::ListTags => "list_tags",
            ToolSelection::ListBranches => "list_branches",
            ToolSelection::Tree => "tree",
            ToolSelection::GetFile => "get_file",
            ToolSelection::ListRepos => "list_repos",
            ToolSelection::Search => "search",
            ToolSelection::GetStats => "get_stats",
        }
    }
}

impl Args {
    pub fn resolve_token(&self) -> Option<String> {
        self.token.clone().or_else(|| {
            if self.token_env.is_empty() {
                None
            } else {
                env::var(&self.token_env).ok()
            }
        })
    }

    pub fn allowed_tools(&self) -> HashSet<ToolSelection> {
        self.tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect())
            .unwrap_or_else(|| ToolSelection::ALL.into_iter().collect())
    }
}
