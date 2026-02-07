mod models;
mod process_runner;
mod utils;

use process_runner::runner;

pub async fn start_application() {
    let mut pm3_runner = runner::ProcessRunner::init();

    match pm3_runner.run().await {
        Ok(()) => println!("PM3 daemon initialized successfully"),
        Err(e) => eprintln!("PM3 daemon failed to initialize: {e}"),
    }

    pm3_runner.dispatch().await;
}
