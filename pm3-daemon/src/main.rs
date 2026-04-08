use pm3_daemon::start_application;

#[tokio::main]
async fn main() {
    match start_application().await {
        Ok(_) => {}
        Err(e) => eprintln!("Error: {}", e),
    };
}
