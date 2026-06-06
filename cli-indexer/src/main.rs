fn main() {
    if let Err(err) = cli_indexer::cli::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
