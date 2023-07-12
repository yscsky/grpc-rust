use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    tonic_build::compile_protos("proto/helloworld/helloworld.proto")?;
    tonic_build::compile_protos("proto/routeguide/route_guide.proto")?;
    tonic_build::compile_protos("proto/echo/echo.proto")?;
    Ok(())
}
