use std::path::PathBuf;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let symbol = args.next().unwrap_or_else(|| "BTCUSDT".to_owned());
    let root = PathBuf::from(args.next().unwrap_or_else(|| "data".to_owned()));
    let seconds = match args.next() {
        Some(value) => match value.parse::<u64>() {
            Ok(n) => Some(n),
            Err(e) => { eprintln!("SECONDS must be a whole number: {e}"); return ExitCode::FAILURE; }
        },
        None => None,
    };
    match md_binance::run_capture(symbol, root, seconds).await {
        Ok(path) => { println!("capture complete: {}", path.display()); ExitCode::SUCCESS }
        Err(e) => { eprintln!("recorder failed: {e}"); ExitCode::FAILURE }
    }
}
