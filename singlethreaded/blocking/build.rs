//!
//! Build script for package to generate Protobuf code for protos.
//!

use prost_build;

fn main() {
    prost_build::compile_protos(
        &["src/protos/p2p.proto"],
        &["src"]).unwrap();
}

