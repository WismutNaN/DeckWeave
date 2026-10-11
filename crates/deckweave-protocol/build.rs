// SPDX-License-Identifier: MIT OR Apache-2.0
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::PathBuf;
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let include = protoc_bin_vendored::include_path()?;
    let proto = PathBuf::from("../../protocol/deckweave/v1/control.proto");
    let protocol_root = PathBuf::from("../../protocol");
    println!("cargo:rerun-if-changed={}", proto.display());
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);
    config.compile_protos(&[proto], &[protocol_root, include])?;
    Ok(())
}
