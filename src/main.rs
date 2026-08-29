use anyhow::Result;
use clap::Parser;
use log::{error, info};
use notify_rust::Notification;
use std::thread;
use std::time::Duration;

mod models;

use crate::models::{Entries, Entry};

#[derive(Parser, Debug)]
struct Args {
    /// The fully qualified URL to the Miniflux server
    server: String,

    /// The wait period between each check in minutes
    #[clap(default_value = "60")]
    wait_period: u64,

    /// The API key used to authenticate with the Miniflux server
    #[clap(long, env)]
    miniflux_api_key: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut entries_cache: Option<Entries> = None;

    if std::env::var("RUST_LOG").is_err() {
        // Unsafe is acceptable as manipulating `envp` is only dangerous when there are multiple
        // threads.
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }
    env_logger::init();

    let server_check = check_for_server_existence(&args.server);
    if !server_check.unwrap_or(false) {
        eprintln!(
            "Server was not found! Make sure it is running and the specified URL is correct."
        );
    }

    info!("Server check passed. Beginning main loop.");

    loop {
        if entries_cache.is_some() {
            thread::sleep(Duration::new(60 * args.wait_period, 0));
        }

        let unread_entries = get_unread_entries(&args.server, &args.miniflux_api_key);

        if unread_entries.is_err() {
            error!("Failed to get unread entries!\n\t{:?}", unread_entries);
            continue;
        }

        let unread_entries = unread_entries.unwrap();

        info!("Unread entries: {}", unread_entries.entries.len());

        // Don't consider any "new" entries when there is no cache
        if let Some(entries_cache) = entries_cache {
            let first_new_index = find_new_entries(&entries_cache.entries, &unread_entries.entries);

            if let Ok(first_new_index) = first_new_index {
                if first_new_index != 0 {
                    let nb = send_notification_batch(
                        unread_entries.entries[0..first_new_index + 1].to_vec(),
                    );
                    if nb.is_err() {
                        error!("{:?}", nb);
                    }
                }
            } else {
                error!("{:?}", first_new_index);
            }
        }

        entries_cache = Some(unread_entries);
    }
}

/// Run a simple healthcheck on the provided server
///
/// This serves two purposes:
/// 1. Handling invalid URLs that a user might provide
/// 2. Checking that the server is available
fn check_for_server_existence(server: &String) -> Result<bool> {
    info!("Checking for server existence");
    reqwest::blocking::get(format!("{}/healthcheck", server))?.error_for_status()?;
    Ok(true)
}

fn get_unread_entries(server: &String, auth_token: &String) -> Result<Entries> {
    let client = reqwest::blocking::Client::new();
    let entries = client
        .get(format!(
            "{}/v1/entries?status=unread&direction=desc",
            server
        ))
        .header("X-Auth-Token", auth_token)
        .send()?
        .error_for_status()?
        .json::<Entries>()?;
    Ok(entries)
}

/// Finds the index in a list of entries to the first "new" entry.
///
/// We want to figure out which entries are "new". To do this, we start with the first
/// entry in the cache and look through the list of unread entries. If we find a match,
/// we can note where the match is and know that all of the entries that came before it
/// are "new". If we don't find a match, we repeat the process using the second entry in
/// the cache and continue in this manner. If every entry in the cache has been
/// exhausted, then everything in the unread entry list is "new".
fn find_new_entries(cache: &Vec<Entry>, new: &[Entry]) -> Result<usize> {
    if new.is_empty() {
        return Err(anyhow::anyhow!(
            "No entries provided when searching for new entries"
        ));
    }

    for cached_entry in cache {
        for (i, new_entry) in new.iter().enumerate() {
            // Found first "new" entry
            if cached_entry.hash == new_entry.hash {
                return Ok(i);
            }
        }
    }

    // Everything must be "new" then
    Ok(0)
}

fn send_notification_batch(entries: Vec<Entry>) -> Result<()> {
    let mut threads = vec![];

    for entry in entries {
        threads.push(thread::spawn(move || {
            let source = match &entry.author.is_empty() {
                true => &entry.feed.title,
                false => &entry.author,
            };
            let notif = Notification::new()
                .summary(format!("New RSS Entry from {}", source).as_str())
                .body(&entry.title)
                .action("open", "Open in web browser")
                .finalize();
            match notif.show() {
                Ok(handle) => handle.wait_for_action(|action| {
                    if action == "open"
                        && let Err(e) = open::that_detached(&entry.url)
                    {
                        error!("{:?}", e);
                    }
                }),
                Err(e) => {
                    error!("{:?}", e);
                }
            };
        }));
    }

    for thread in threads {
        if let Err(e) = thread.join() {
            error!("{:?}", e);
        }
    }

    Ok(())
}
