fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../proto/compiler.proto");
    let descriptors = protox::compile(["compiler.proto"], ["../../proto"])?;
    prost_build::Config::new().compile_fds(descriptors)?;
    Ok(())
}
