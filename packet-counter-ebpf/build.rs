fn main() {
    // Re-run the build script whenever the eBPF source changes.
    println!("cargo:rerun-if-changed=src/");
}
