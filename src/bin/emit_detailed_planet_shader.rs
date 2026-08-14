fn main() {
    match black_cube_gallery::planet_shader::detailed_fragment_source() {
        Ok(source) => print!("{source}"),
        Err(error) => {
            eprintln!("Could not compose the detailed planetary fragment shader: {error}");
            std::process::exit(1);
        }
    }
}
