fn main() {
    match artwork_world_in_light::planet_shader::fragment_source() {
        Ok(source) => print!("{source}"),
        Err(error) => {
            eprintln!("Could not compose the planetary fragment shader: {error}");
            std::process::exit(1);
        }
    }
}
