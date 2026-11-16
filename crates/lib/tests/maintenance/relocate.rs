#[cfg(target_os = "linux")]
#[path = "relocate/mod.rs"]
mod relocate;

#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = relocate::entry(std::env::args_os().skip(1).collect()).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("member-preserving relocation is qualified only on Linux");
    std::process::exit(1);
}
