fn main() {
    if let Err(err) = alt_indexer::cli::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
