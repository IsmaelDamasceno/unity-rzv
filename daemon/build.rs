

fn main() {
    prost_build::compile_protos(&["proto/schema.proto"], &["proto/"]).unwrap();
}
