fn main() {
    match uniffi::generate_scaffolding("./src/mobile_core.udl") {
        Ok(_) => println!("cargo:rerun-if-changed=./src/mobile_core.udl"),
        Err(e) => {
            eprintln!("❌ UniFFI failed to parse UDL: {e}");
            std::process::exit(1);
        }
    }
}
