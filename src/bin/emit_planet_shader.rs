fn main() {
    match black_cube_gallery::planet_shader::fragment_source() {
        Ok(source) => print!("{source}"),
        Err(error) => {
            eprintln!("Could not compose the planetary fragment shader: {error}");
            std::process::exit(1);
        }
    }
}
