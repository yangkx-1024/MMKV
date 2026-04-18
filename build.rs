fn main() {
    buffa_build::Config::new()
        .generate_views(true)
        .files(&["src/protos/kv.proto"])
        .includes(&["src/protos"])
        .compile()
        .unwrap();
}
