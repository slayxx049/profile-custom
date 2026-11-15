use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: verify <capture-file>");
        return ExitCode::FAILURE;
    };
    let path = PathBuf::from(path);
    match md_verify::verify(&path) {
        Ok(r) => {
            println!("records: {}", r.records);
            println!("gap records: {}", r.gaps);
            println!("clean trailer: {}", r.complete);
            println!("record count matches: {}", r.count_matches);
            println!("truncated bytes: {}", r.truncated_bytes);
            for (previous, current) in &r.sequence_holes {
                println!("ERROR sequence discontinuity: {previous} -> {current}");
            }
            if r.is_clean() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(e) => { eprintln!("verification failed: {e}"); ExitCode::FAILURE }
    }
}
