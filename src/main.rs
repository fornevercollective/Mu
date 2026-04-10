mod bench;
mod packet;
mod render;
mod stream;

fn main() {
    let headless = std::env::args().any(|a| a == "--headless");

    if headless {
        bench::run_headless_benchmark();
    } else {
        eprintln!("MuTerminal v0.1.0 — M4 Hyperfin Architecture");
        eprintln!("Run with --headless for benchmark mode.");
    }
}
