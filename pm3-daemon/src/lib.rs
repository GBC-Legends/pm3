mod process_runner;

use process_runner::runner;

pub async fn start_application() {
    runner::ProcessRunner::run().await;
}
