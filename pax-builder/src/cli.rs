use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use chrono;

use axum::{
    extract::Path as AxumPath, extract::Query, extract::Request, http::StatusCode,
    response::IntoResponse, routing::get, Router,
};
use std::fs;
use tower_http::services::ServeDir;

mod version_checker;
mod worker;

#[derive(Parser)]
#[command(name = "pax-build-infra")]
#[command(about = "PAX Build Infrastructure - Automated build system with web GUI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start {
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    CloneRecipes {
        #[arg(
            long,
            default_value = "https://github.com/oreonproject/paxbuild-recipes"
        )]
        repo: String,
        #[arg(long, default_value = "./recipes")]
        output_dir: PathBuf,
    },
    BuildAll {
        #[arg(long, default_value = "./recipes/oreon-11")]
        recipes_dir: PathBuf,
        #[arg(long, default_value = "./results")]
        output_dir: PathBuf,
    },
    Worker {
        #[arg(long)]
        server_url: String,
        #[arg(long, default_value = "4")]
        workers: usize,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { port } => {
            println!("Starting PAX Build Infrastructure server on port {}", port);
            println!("Web GUI available at: http://localhost:{}/", port);
            println!(
                "Results browser available at: http://localhost:{}/results/",
                port
            );

            // Start Axum server with static file serving
            start_axum_server(port).await?;
        }
        Commands::CloneRecipes { repo, output_dir } => {
            clone_recipes(&repo, &output_dir).await?;
        }
        Commands::BuildAll {
            recipes_dir,
            output_dir,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            build_all_packages(&recipes_dir, &output_dir).await?;
        }
        Commands::Worker {
            server_url,
            workers,
        } => {
            println!(
                "Worker mode: connecting to {} with {} workers",
                server_url, workers
            );
        }
    }

    Ok(())
}

async fn start_axum_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Create the results directory if it doesn't exist
    std::fs::create_dir_all("./results")?;

    let app = Router::new()
        .route("/", get(dashboard_page))
        .route("/version", get(version_check_page))
        .route("/api/status", get(api_status))
        .route(
            "/results",
            get(|| results_handler(AxumPath("".to_string()))),
        )
        .route(
            "/results/",
            get(|| results_handler(AxumPath("".to_string()))),
        )
        .route("/results/*path", get(results_handler))
        .fallback(fallback_404);

    println!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn dashboard_page() -> impl IntoResponse {
    axum::response::Html(
        r#"<!DOCTYPE html>
<html>
<head><title>PAX Build Dashboard</title>
<style>
body { font-family: Arial; margin: 20px; background: #f5f5f5; }
.container { background: white; padding: 30px; border-radius: 8px; max-width: 1200px; margin: 0 auto; }
h1 { color: #333; }
.nav { display: flex; gap: 20px; margin-bottom: 30px; border-bottom: 2px solid #ddd; padding-bottom: 10px; }
.nav a { color: #0066cc; text-decoration: none; padding: 8px 16px; }
.nav a.active { border-bottom: 2px solid #0066cc; }
.package { border: 1px solid #ddd; border-radius: 6px; padding: 15px; margin-bottom: 10px; }
.status-dot { display: inline-block; width: 12px; height: 12px; border-radius: 50%; margin-right: 10px; }
.green { background: #4caf50; }
.orange { background: #ff9800; }
.red { background: #f44336; }
.blue { background: #2196f3; }
</style>
</head>
<body>
<div class="container">
<h1>PAX Build Dashboard</h1>
<div class="nav">
<a href="/" class="active">Build Status</a>
<a href="/version">Version Check</a>
<a href="/results" target="_blank">Results Browser</a>
</div>
<div class="package-list">
<div class="package">
<div class="status-dot green"></div>
<span>System is ready. Use build-all.sh build to start building packages.</span>
</div>
</div>
</div>
</body>
</html>
"#,
    )
}

async fn version_check_page() -> impl IntoResponse {
    axum::response::Html(
        r#"<!DOCTYPE html>
<html>
<head><title>Version Check - PAX Build</title>
<style>
body { font-family: Arial; margin: 20px; background: #f5f5f5; }
.container { background: white; padding: 30px; border-radius: 8px; max-width: 1200px; margin: 0 auto; }
h1 { color: #333; }
.nav { display: flex; gap: 20px; margin-bottom: 30px; border-bottom: 2px solid #ddd; padding-bottom: 10px; }
.nav a { color: #0066cc; text-decoration: none; padding: 8px 16px; }
.nav a.active { border-bottom: 2px solid #0066cc; }
table { width: 100%; border-collapse: collapse; }
th, td { padding: 12px; text-align: left; border-bottom: 1px solid #ddd; }
th { background: #f5f5f5; }
</style>
</head>
<body>
<div class="container">
<h1>Version Check</h1>
<div class="nav">
<a href="/">Build Status</a>
<a href="/version" class="active">Version Check</a>
<a href="/results" target="_blank">Results Browser</a>
</div>
<table>
<thead><tr><th>Package</th><th>Current Version</th><th>Upstream Version</th><th>Status</th></tr></thead>
<tbody>
<tr><td colspan="4" style="text-align:center;padding:40px;">Check versions here</td></tr>
</tbody>
</table>
</div>
</body>
</html>
"#,
    )
}

async fn api_status() -> impl IntoResponse {
    axum::response::Json(serde_json::json!({"packages": []}))
}

async fn results_handler(AxumPath(path): AxumPath<String>) -> impl IntoResponse {
    let full_path = std::path::Path::new("../results").join(&path);

    // If it's a directory, show directory listing
    if full_path.is_dir() {
        let dir_path = format!("../results/{}", path);
        match generate_directory_listing(&dir_path, &path) {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(_) => axum::response::Html(
                r#"<!DOCTYPE html>
<html>
<head><title>Results - Error</title></head>
<body>
<h1>Error</h1>
<p>Unable to read directory</p>
</body>
</html>
"#
                .to_string(),
            )
            .into_response(),
        }
    } else if full_path.exists() {
        // If it's a file, serve it
        match tokio::fs::read(&full_path).await {
            Ok(data) => {
                let content_type = mime_guess::from_path(&full_path)
                    .first_or_octet_stream()
                    .to_string();
                (StatusCode::OK, [("Content-Type", content_type)], data).into_response()
            }
            Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
        }
    } else {
        (StatusCode::NOT_FOUND, "Not found").into_response()
    }
}

fn generate_directory_listing(
    dir_path: &str,
    current_path: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();

    if let Ok(dir_entries) = std::fs::read_dir(dir_path) {
        for entry in dir_entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                let metadata = entry.metadata()?;
                let size = if metadata.is_file() {
                    format!("{} bytes", metadata.len())
                } else {
                    "-".to_string()
                };

                let modified = metadata.modified()?;
                let datetime: chrono::DateTime<chrono::Local> = modified.into();
                let modified_str = datetime.format("%Y-%m-%d %H:%M").to_string();

                entries.push((file_name.to_string(), size, modified_str, metadata.is_dir()));
            }
        }
    }

    // Sort: directories first, then files alphabetically
    entries.sort_by(|a, b| match (a.3, b.3) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.cmp(&b.0),
    });

    // Generate breadcrumbs
    let path_parts: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
    let mut breadcrumbs = String::from("<a href=\"/results\">results</a>");
    for (i, part) in path_parts.iter().enumerate() {
        let path_so_far = format!("/results/{}", path_parts[..=i].join("/"));
        breadcrumbs.push_str(&format!("/<a href=\"{}\">{}</a>", path_so_far, part));
    }

    let title = if current_path.is_empty() {
        "Index of /results".to_string()
    } else {
        format!("Index of /results/{}", current_path)
    };

    // Generate parent link
    let parent_link = if current_path.is_empty() {
        "/".to_string()
    } else {
        let parent_path = if let Some(last_slash) = current_path.rfind('/') {
            &current_path[..last_slash]
        } else {
            ""
        };
        format!("/results/{}", parent_path)
    };

    let mut html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <style>
        body {{ font-family: monospace; margin: 20px; }}
        h1 {{ color: #333; }}
        .breadcrumbs {{ margin-bottom: 20px; color: #666; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ padding: 8px; text-align: left; border-bottom: 1px solid #ddd; }}
        th {{ background-color: #f2f2f2; }}
        tr:hover {{ background-color: #f5f5f5; }}
        a {{ text-decoration: none; color: #0066cc; }}
        a:hover {{ text-decoration: underline; }}
        .dir {{ font-weight: bold; }}
        .parent {{ color: #666; }}
    </style>
</head>
<body>
    <h1>{}</h1>
    <div class="breadcrumbs">{}</div>
    <table>
        <thead>
            <tr>
                <th>Name</th>
                <th>Size</th>
                <th>Last Modified</th>
            </tr>
        </thead>
        <tbody>
            <tr>
                <td><a href="{}" class="parent">../</a></td>
                <td>-</td>
                <td>-</td>
            </tr>"#,
        title, title, breadcrumbs, parent_link
    );

    for (name, size, modified, is_dir) in entries {
        let css_class = if is_dir { "dir" } else { "" };
        let display_name = if is_dir {
            format!("{}/", name)
        } else {
            name.clone()
        };
        let link_path = if current_path.is_empty() {
            if is_dir {
                format!("/results/{}/", name)
            } else {
                format!("/results/{}", name)
            }
        } else {
            if is_dir {
                format!("/results/{}/{}/", current_path, name)
            } else {
                format!("/results/{}/{}", current_path, name)
            }
        };
        html.push_str(&format!(
            r#"<tr>
                <td><a href="{}" class="{}">{}</a></td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            link_path, css_class, display_name, size, modified
        ));
    }

    html.push_str(
        r#"
        </tbody>
    </table>
</body>
</html>"#,
    );

    Ok(html)
}

async fn fallback_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}

async fn clone_recipes(repo: &str, output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Cloning recipes from {} to {}", repo, output_dir.display());

    let output = tokio::process::Command::new("git")
        .arg("clone")
        .arg(repo)
        .arg(output_dir)
        .output()
        .await?;

    if !output.status.success() {
        return Err(format!("Git clone failed").into());
    }

    println!("Recipes cloned successfully");
    Ok(())
}

async fn build_all_packages(
    recipes_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Building packages from {}", recipes_dir.display());

    if !recipes_dir.exists() {
        return Err("Recipes directory not found".into());
    }

    let entries = std::fs::read_dir(recipes_dir)?;
    let mut package_count = 0;
    let mut built_count = 0;
    let mut failed_count = 0;
    let mut failed_packages = Vec::new();

    // Collect packages first
    let mut packages = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_dir() {
                packages.push(path);
                package_count += 1;
            }
        }
    }

    println!("Found {} packages to build", package_count);
    println!("Results will go to: {}", output_dir.display());
    println!();

    // Build each package
    for package_dir in packages {
        let package_name = package_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        // Look for any .yaml file in the package directory
        let yaml_path = std::fs::read_dir(&package_dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "yaml" || ext == "yml" {
                        return Some(path);
                    }
                }
                None
            })
            .next();

        let yaml_path = match yaml_path {
            Some(path) => path,
            None => {
                println!("⚠️  Skipping {} - no yaml file found", package_name);
                continue;
            }
        };

        println!("Building package: {}...", package_name);

        // Run pax-builder build command - use the binary directly since we're already in the project
        let bin_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.join("pax-builder")))
            .unwrap_or_else(|| std::path::PathBuf::from("target/debug/pax-builder"));

        let status = tokio::process::Command::new(bin_path)
            .arg("build")
            .arg(&yaml_path)
            .arg("--verbose")
            .arg("--output-dir")
            .arg(output_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await;

        match status {
            Ok(result) => {
                if result.success() {
                    println!("✓ Successfully built {}", package_name);
                    built_count += 1;
                } else {
                    println!("✗ Failed to build {}", package_name);
                    failed_count += 1;
                    failed_packages.push(package_name.clone());
                }
            }
            Err(e) => {
                println!("✗ Failed to build {}: {}", package_name, e);
                failed_count += 1;
                failed_packages.push(package_name.clone());
            }
        }
        println!();
    }

    println!("=== Build Summary ===");
    println!("Total packages: {}", package_count);
    println!("Successfully built: {}", built_count);
    println!("Failed: {}", failed_count);

    if !failed_packages.is_empty() {
        println!();
        println!("Failed packages:");
        for package in &failed_packages {
            println!("  • {}", package);
        }
    }

    Ok(())
}
