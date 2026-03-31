use pm3_daemon::start_application;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let public = args
        .get(1)
        .map(|s| s.to_lowercase() == "public")
        .unwrap_or(false);

    match start_application(public).await {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    };
}
