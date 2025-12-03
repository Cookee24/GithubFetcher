pub mod cli;
pub mod client;
pub mod error;
pub mod models;
pub mod server;

use std::collections::HashSet;

use cli::Args;
use client::GithubClient;
use rmcp::ServiceExt;
use server::GithubServer;

pub async fn run(args: Args) -> anyhow::Result<()> {
    let token = args.resolve_token();
    let allowed_tools: HashSet<_> = args.allowed_tools();

    let client = GithubClient::new(args.api_base, token)?;
    let server = GithubServer::new(client, allowed_tools);

    let service = server.serve(rmcp::transport::stdio()).await?;

    service.waiting().await?;
    Ok(())
}
