//! Prints the app picker's list and how long it took: `cargo run --example list_apps`.

fn main() {
    let start = std::time::Instant::now();
    let apps = detent_lib::apps::list();
    let took = start.elapsed();
    for a in &apps {
        println!(
            "{}{}  {}",
            if a.running { "* " } else { "  " },
            a.name,
            a.bundle_id
        );
    }
    println!("{} apps in {took:?}", apps.len());
}
