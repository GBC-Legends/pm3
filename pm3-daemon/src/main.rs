use pm3_daemon::start_application;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let public_args = args
        .get(1)
        .map(|s| s.to_lowercase() == "public")
        .unwrap_or(false);

    let public_env = std::env::var("PM3_PUBLIC")
        .map(|s| s == "1" || s == "public" || s == "true")
        .unwrap_or(false);

    match start_application(public_args || public_env).await {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    };
}
