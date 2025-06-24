// In build.rs
fn main() {
    uniffi::generate_scaffolding("./src/mobile_core.udl").unwrap();
}