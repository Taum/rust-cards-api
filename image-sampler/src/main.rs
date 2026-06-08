fn main() {
    if let Err(err) = image_sampler::cli::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
