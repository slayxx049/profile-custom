use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(input) = args.next() else {
        eprintln!("usage: normalize <capture-file> <output-root>");
        return ExitCode::FAILURE;
    };
    let Some(output) = args.next() else {
        eprintln!("usage: normalize <capture-file> <output-root>");
        return ExitCode::FAILURE;
    };
    match md_normalize::normalize_file(&PathBuf::from(input), &PathBuf::from(output)) {
        Ok(paths) => {
            for path in paths { println!("wrote {}", path.display()); }
            ExitCode::SUCCESS
        }
        Err(e) => { eprintln!("normalization failed: {e}"); ExitCode::FAILURE }
    }
}
