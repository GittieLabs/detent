//! Prints the generated C header. Use `scripts/gen-header.sh`, not this directly.

fn main() {
    print!("{}", detent_protocol::header::c_header());
}
