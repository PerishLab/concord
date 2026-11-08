#[path = "cutover/mod.rs"]
mod cutover;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = cutover::entry(std::env::args_os().skip(1).collect()).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
