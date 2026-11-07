//! Prints the generated golden vectors. Use `scripts/gen-header.sh`.

fn main() {
    print!("{}", detent_protocol::vectors::c_vectors());
}
