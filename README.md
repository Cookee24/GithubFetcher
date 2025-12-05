# GitHub Fetcher

This project aims to provide a simple way for large models to access code files in GitHub and to minimize noise in the context.

An MCP server that surfaces a focused set of GitHub-read endpoints (repos, tags, branches, trees, files, stats, search) over stdio so agents can fetch just the code and metadata they need.

## Quick start
- Install Rust (edition 2024) and ensure `cargo` is available.
- Export a GitHub token (recommended to avoid rate limits): `export GITHUB_AUTH_TOKEN=ghp_yourtoken`.
- Run the server: `cargo run -- --token-env GITHUB_AUTH_TOKEN`.
- Point your MCP client at the stdio transport; by default all tools are enabled. Limit exposure with `--tools get_repo,list_repos,tree` (comma-separated).

## CLI flags
- `--api-base` (`https://api.github.com` default): override for GitHub Enterprise or testing.
- `--token`: personal access token; if omitted, `--token-env` is used.
- `--token-env` (`GITHUB_AUTH_TOKEN` default): env var name to read the token from; set to an empty string to skip env lookup.
- `--tools`: restrict which tools are exposed (`get_repo`, `list_tags`, `list_branches`, `tree`, `get_file`, `list_repos`, `search`, `get_stats`).

## Notes on responses
- `tree` and `get_stats` emit `type` values: `file`, `dir`, `symlink`, or `submodule`.
- `get_file` can trim content by `line_range` or `max_chars` (UTF-8 safe). Ranges are 1-based and inclusive; strings like `1..200`, `1...200`, `..200`, `1..`, `1:200`, `1:`, `:200`, or a single number `N` meaning lines `1..=N`.
- `list_repos` transparently tries both user and org scopes.

<details>
<summary>Tools, inputs, and outputs</summary>

#### get_repo
- Input: `owner` (string), `repo` (string)
- Output: `repo` (nullable) with `description` (string?), `stars` (u64), `forks` (u64), `license` (object? with `key`, `name`, `spdx_id`, `url`)

#### list_tags
- Input: `owner` (string), `repo` (string)
- Output: `tags` (array of tag names)

#### list_branches
- Input: `owner` (string), `repo` (string)
- Output: `branches` (array of branch names)

#### list_repos
- Input: `owner` (string), `page` (usize?, optional), `per_page` (usize?, optional)
- Output: `repos` (array) with `name`, `full_name`, `private` (bool), `description` (string?), `html_url`

#### tree
- Input: `owner` (string), `repo` (string), `path` (string?, defaults to root), `depth` (usize, defaults to `1`, minimum `1`), `ref` (string?, git ref)
- Output: `entries` (array of tree nodes) each with `type`, `name`, `size` (u64?), `target` (string? for symlink), `submodule_git_url` (string?), `children` (nested entries)

#### get_file
- Input: `owner` (string), `repo` (string), `path` (string), `ref` (string?, git ref), `line_range` (string formats like `1..200`, `1...200`, `..200`, `1..`, `1:200`, `1:`, `:200`, or a number `N`), `max_chars` (usize?)
- Output: `content` (string, decoded and optionally trimmed)

#### search
- Input: `query` (string, supports GitHub code search qualifiers), `page` (usize?, optional), `per_page` (usize?, optional)
- Output: `results` (array) with `name`, `path`, `repository` (full `owner/repo`)

#### get_stats
- Input: `owner` (string), `repo` (string), `path` (string), `ref` (string?, git ref)
- Output: `item` with `type`, `name`, `path`, `size` (u64?), `target` (string?), `submodule_git_url` (string?)

</details>

## Development
- Format is handled by `rustfmt`; run tests with `cargo test`.
- The server speaks MCP over stdio using `rmcp`; no HTTP server is exposed.
