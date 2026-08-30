use anyhow::{Context, Result};
use clap::Parser;
use log::{error, info};
use notify_rust::{Notification, NotificationResponse};
use std::time::{Duration, SystemTime};

mod models;

use crate::models::{Entries, Entry};

#[derive(Parser, Debug)]
struct Args {
    /// The fully qualified URL to the Miniflux server
    server: String,

    /// The wait period between each check in minutes
    #[clap(default_value = "10")]
    wait_period: u64,

    /// The API key used to authenticate with the Miniflux server
    #[clap(long, env)]
    miniflux_api_key: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if std::env::var("RUST_LOG").is_err() {
        // Unsafe is acceptable as manipulating `envp` is only dangerous when there are multiple
        // threads.
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }
    env_logger::init();

    if check_for_server_existence(&args.server).await.is_err() {
        anyhow::bail!(
            "Server was not found! Make sure it is running and the specified URL is correct."
        );
    }

    info!(
        "Server check passed. Will poll for updates every {} minutes",
        args.wait_period
    );

    let mut last_check = SystemTime::now();
    let sleep_time = Duration::new(60 * args.wait_period, 0);

    loop {
        let since = last_check;
        last_check = SystemTime::now();
        let unread_entries = get_unread_entries(&args.server, &args.miniflux_api_key, since)
            .await
            .context("Fetching unread entries")?;

        info!("Unread entries: {}", unread_entries.entries.len());
        send_notification_batch(unread_entries.entries).await?;

        tokio::time::sleep(sleep_time).await;
    }
}

/// Run a simple healthcheck on the provided server
///
/// This serves two purposes:
/// 1. Handling invalid URLs that a user might provide
/// 2. Checking that the server is available
async fn check_for_server_existence(server: &String) -> Result<()> {
    info!("Checking for server existence");
    reqwest::get(format!("{}/healthcheck", server))
        .await?
        .error_for_status()?;
    Ok(())
}

/// Get all of the unread entries that have been created since the specified time
async fn get_unread_entries(
    server: &String,
    auth_token: &String,
    since: SystemTime,
) -> Result<Entries> {
    let client = reqwest::Client::new();
    let entries = client
        .get(format!(
            "{}/v1/entries?status=unread&direction=desc&after={}",
            server,
            since.duration_since(std::time::UNIX_EPOCH)?.as_secs()
        ))
        .header("X-Auth-Token", auth_token)
        .send()
        .await?
        .error_for_status()?
        .json::<Entries>()
        .await?;

    Ok(entries)
}

/// Send a set of notifications in bulk
async fn send_notification_batch(entries: Vec<Entry>) -> Result<()> {
    for entry in entries {
        let source = match &entry.author.is_empty() {
            true => &entry.feed.title,
            false => &entry.author,
        };

        let handle = Notification::new()
            .summary(format!("New RSS Entry from {}", source).as_str())
            .body(&entry.title)
            .action("open", "Open in web browser")
            .timeout(notify_rust::Timeout::Milliseconds(2000))
            .show_async()
            .await?;

        tokio::spawn(async move {
            handle
                .wait_for_action_async(|response| match response {
                    NotificationResponse::Action(action) if action == "open" => {
                        if let Err(e) = open::that_detached(&entry.url) {
                            error!("{:?}", e);
                        }
                    }
                    NotificationResponse::Action(_) => {}
                    NotificationResponse::Closed(_) => {}
                    NotificationResponse::Reply(_) => {}
                    NotificationResponse::Default => {}
                })
                .await;
        });
    }

    Ok(())
}
