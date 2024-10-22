//!
//! Build script for package to generate Protobuf code for protos.
//!

use prost_build;

fn main() {
    prost_build::compile_protos(&["src/protos/messages.proto"], &["src"]).unwrap();
}
