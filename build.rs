fn main() {
    protobuf_codegen::Codegen::new()
        // Use `protoc` parser, optional.
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        // All inputs and imports from the inputs must reside in `includes` directories.
        .includes(["src/protos"])
        // Inputs must reside in some of include paths.
        .input("src/protos/kv.proto")
        // Specify output directory relative to Cargo output directory.
        .cargo_out_dir("protos")
        .run_from_script();
}
