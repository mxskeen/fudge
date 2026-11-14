//! `cargo run -p fudge-providers --example query -- "12 * 18"` prints results as JSON.
fn main() {
    let q = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let (engine, _platform) = fudge_providers::build_engine();
    // Give background scans a moment (apps, file index).
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let t = std::time::Instant::now();
    let r = engine.search(&q, engine.config().max_results);
    eprintln!("{} results in {:?}", r.len(), t.elapsed());
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
}
