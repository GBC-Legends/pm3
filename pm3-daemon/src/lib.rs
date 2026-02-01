mod models;
mod process_runner;
mod utils;

use process_runner::runner;

pub async fn start_application() {
    let pm3_runner = runner::ProcessRunner::init();

    pm3_runner.run().await;
}
