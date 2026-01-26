use pm3_daemon::start_application;

#[tokio::main]
async fn main() {
    start_application().await;
}
