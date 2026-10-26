use pm3_daemon::start_application;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--version"] {
        println!("pm3-daemon {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if !args.is_empty() {
        eprintln!("Usage: pm3-daemon [--version]");
        std::process::exit(2);
    }
    match start_application().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {e:#}");
            std::process::exit(1);
        }
    };
}
