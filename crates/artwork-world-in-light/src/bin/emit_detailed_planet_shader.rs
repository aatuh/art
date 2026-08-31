fn main() {
    match artwork_world_in_light::planet_shader::detailed_fragment_source() {
        Ok(source) => print!("{source}"),
        Err(error) => {
            eprintln!("Could not compose the detailed planetary fragment shader: {error}");
            std::process::exit(1);
        }
    }
}
