fn main() {
    tonic_build::compile_protos("../proto-schema/schema.proto").unwrap();
}
