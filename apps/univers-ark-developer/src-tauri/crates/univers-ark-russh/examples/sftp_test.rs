//! Quick SFTP test
//! Run with: cargo run -p univers-ark-russh --example sftp_test
//!
//! Usage: cargo run -p univers-ark-russh --example sftp_test -- <host-alias>

use std::env;

fn main() {
    let args: Vec<_> = env::args().collect();

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        test_sftp(args.get(1).map(|s| s.as_str())).await;
    });
}

async fn test_sftp(host_alias: Option<&str>) {
    // Use default SSH config
    let resolver = match univers_ark_russh::SshConfigResolver::from_default_path() {
        Ok(r) => r,
        Err(e) => {
            println!("Failed to load SSH config: {}", e);
            return;
        }
    };

    println!("Available hosts: use -- to specify one");

    let options = univers_ark_russh::ClientOptions::default();

    // Use provided host or ask user
    let host = match host_alias {
        Some(h) => h.to_string(),
        None => {
            println!("Usage: sftp_test <host-alias>");
            return;
        }
    };

    println!("\n=== Testing list_directory ===");
    match univers_ark_russh::list_directory_alias(&resolver, &host, None, &options).await {
        Ok(listing) => {
            println!("✓ Connected successfully!");
            println!("  Path: {}", listing.path);
            println!("  Entries: {}", listing.entries.len());
            for entry in listing.entries.iter().take(5) {
                println!("    - {} ({}) [{} bytes]", entry.name, entry.kind, entry.size);
            }
        }
        Err(e) => {
            println!("✗ Failed: {:?}", e);
            return;
        }
    }

    println!("\n=== Testing read_file_preview ===");
    // Try to read .bashrc
    let file_path = ".bashrc";
    match univers_ark_russh::read_file_preview_alias(&resolver, &host, file_path, &options).await {
        Ok(preview) => {
            println!("✓ File preview loaded!");
            println!("  Path: {}", preview.path);
            println!("  Size: {} bytes", preview.content.len());
            println!("  Binary: {}", preview.is_binary);
            println!("  Truncated: {}", preview.truncated);
            println!("  Content (first 200 chars):");
            println!("{}", &preview.content.chars().take(200).collect::<String>());
        }
        Err(e) => {
            println!("✗ Failed: {:?}", e);
        }
    }
}
