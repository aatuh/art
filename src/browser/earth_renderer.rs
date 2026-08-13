//! Self-contained WebGL renderer for the World in Light installation.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, WebGlBuffer, WebGlProgram, WebGlRenderingContext as Gl, WebGlUniformLocation,
};

use super::{
    compile_shader, link_program, planetarium_room_vertices, required_uniform, resize_canvas,
    sphere_vertices, star_vertices, upload_vertices,
};
use crate::{
    fps::PlayerState,
    math::{Mat4, cross, normalize},
};

/// A procedural room-scale planetary installation. All surface detail is synthesized in
/// the fragment shader: no image, model, or network asset is required.
pub(super) struct PlanetRenderer {
    canvas: HtmlCanvasElement,
    gl: Gl,
    program: WebGlProgram,
    sphere_buffer: WebGlBuffer,
    room_buffer: WebGlBuffer,
    star_buffer: WebGlBuffer,
    model_uniform: WebGlUniformLocation,
    view_projection_uniform: WebGlUniformLocation,
    camera_uniform: WebGlUniformLocation,
    sun_uniform: WebGlUniformLocation,
    moon_uniform: WebGlUniformLocation,
    kind_uniform: WebGlUniformLocation,
    sphere_count: i32,
    room_count: i32,
    star_count: i32,
}

impl PlanetRenderer {
    pub(super) fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl = canvas
            .get_context("webgl")?
            .ok_or_else(|| JsValue::from_str("WebGL is unavailable."))?
            .dyn_into::<Gl>()?;
        let vertex = compile_shader(
            &gl,
            Gl::VERTEX_SHADER,
            r#"attribute vec3 a_position;
               attribute vec3 a_normal;
               uniform mat4 u_model;
               uniform mat4 u_view_projection;
               uniform float u_kind;
               varying vec3 v_world;
               varying vec3 v_normal;
               varying vec3 v_local_normal;
               float hash(vec3 p) { return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453123); }
               float noise(vec3 p) {
                 vec3 i = floor(p); vec3 f = fract(p); f = f * f * (3.0 - 2.0 * f);
                 return mix(mix(mix(hash(i), hash(i + vec3(1.0,0.0,0.0)), f.x),
                                mix(hash(i + vec3(0.0,1.0,0.0)), hash(i + vec3(1.0,1.0,0.0)), f.x), f.y),
                            mix(mix(hash(i + vec3(0.0,0.0,1.0)), hash(i + vec3(1.0,0.0,1.0)), f.x),
                                mix(hash(i + vec3(0.0,1.0,1.0)), hash(i + vec3(1.0,1.0,1.0)), f.x), f.y), f.z);
               }
               float fbm(vec3 p) {
                 float value = 0.0; float amplitude = 0.5;
                 for (int octave = 0; octave < 5; octave++) { value += amplitude * noise(p); p = p * 2.03 + 11.7; amplitude *= 0.5; }
                 return value;
               }
               float terrain_elevation(vec3 local) {
                 vec3 terrain_point = normalize(local);
                 float continents = fbm(terrain_point * 1.65);
                 float detail = fbm(terrain_point * 7.0);
                 float ridges = 1.0 - abs(2.0 * fbm(terrain_point * 4.0 + 3.1) - 1.0);
                 float land = smoothstep(0.48, 0.58, continents + detail * 0.12);
                 float pacific_field = dot(terrain_point, normalize(vec3(0.14, 0.02, 0.99)))
                   + (fbm(terrain_point * 2.4 + 31.0) - 0.5) * 0.15;
                 float pacific = smoothstep(0.34, 0.52, pacific_field);
                 float islands = smoothstep(0.83, 0.94, detail + ridges * 0.25) * pacific;
                 // Relief is deliberately readable but no longer stylized as a spiky world.
                 return land * (0.006 + detail * 0.008 + ridges * ridges * 0.035) * (1.0 - pacific + islands);
               }
               float cloud_density(vec3 local) {
                 vec3 cloud_point = normalize(local);
                 float broad = fbm(cloud_point * 3.0 + 7.0);
                 float wisps = fbm(cloud_point * 12.0 - 3.0);
                 return smoothstep(0.58, 0.70, broad + wisps * 0.18);
               }
               void main() {
                 vec3 local_position = a_position;
                 vec3 local_normal = a_normal;
                 if (u_kind > -0.5 && u_kind < 0.5) {
                   vec3 radial = normalize(a_position);
                   vec3 tangent = cross(vec3(0.0, 1.0, 0.0), radial);
                   if (length(tangent) < 0.01) tangent = cross(vec3(1.0, 0.0, 0.0), radial);
                   tangent = normalize(tangent);
                   vec3 bitangent = normalize(cross(radial, tangent));
                   float sample_step = 0.018;
                   vec3 point = radial * (1.0 + terrain_elevation(radial));
                   vec3 tangent_point = normalize(radial + tangent * sample_step);
                   tangent_point *= 1.0 + terrain_elevation(tangent_point);
                   vec3 bitangent_point = normalize(radial + bitangent * sample_step);
                   bitangent_point *= 1.0 + terrain_elevation(bitangent_point);
                   local_position = point;
                   local_normal = normalize(cross(tangent_point - point, bitangent_point - point));
                 } else if (u_kind > 0.5 && u_kind < 1.5) {
                   // Displace only dense cloud bodies, leaving gaps between them instead of a shell.
                   local_position = normalize(a_position) * (1.015 + cloud_density(a_position) * 0.07);
                 }
                 vec4 world = u_model * vec4(local_position, 1.0);
                 v_world = world.xyz;
                 v_normal = mat3(u_model) * local_normal;
                 v_local_normal = a_normal;
                 gl_Position = u_view_projection * world;
                 if (u_kind > 4.5) gl_PointSize = 1.0 + fract(a_position.x * 17.0) * 1.8;
               }"#,
        )?;
        let fragment = compile_shader(
            &gl,
            Gl::FRAGMENT_SHADER,
            r#"precision highp float;
               uniform vec3 u_sun;
               uniform vec3 u_camera;
               uniform vec3 u_moon;
               uniform float u_kind;
               varying vec3 v_world;
               varying vec3 v_normal;
               varying vec3 v_local_normal;

               float hash(vec3 p) { return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453123); }
               float noise(vec3 p) {
                 vec3 i = floor(p); vec3 f = fract(p); f = f * f * (3.0 - 2.0 * f);
                 return mix(mix(mix(hash(i), hash(i + vec3(1.0,0.0,0.0)), f.x),
                                mix(hash(i + vec3(0.0,1.0,0.0)), hash(i + vec3(1.0,1.0,0.0)), f.x), f.y),
                            mix(mix(hash(i + vec3(0.0,0.0,1.0)), hash(i + vec3(1.0,0.0,1.0)), f.x),
                                mix(hash(i + vec3(0.0,1.0,1.0)), hash(i + vec3(1.0,1.0,1.0)), f.x), f.y), f.z);
               }
               float fbm(vec3 p) {
                 float value = 0.0; float amplitude = 0.5;
                 for (int octave = 0; octave < 5; octave++) { value += amplitude * noise(p); p = p * 2.03 + 11.7; amplitude *= 0.5; }
                 return value;
               }
               float terrain_elevation(vec3 local) {
                 vec3 terrain_point = normalize(local);
                 float continents = fbm(terrain_point * 1.65);
                 float detail = fbm(terrain_point * 7.0);
                 float ridges = 1.0 - abs(2.0 * fbm(terrain_point * 4.0 + 3.1) - 1.0);
                 float land = smoothstep(0.48, 0.58, continents + detail * 0.12);
                 float pacific_field = dot(terrain_point, normalize(vec3(0.14, 0.02, 0.99)))
                   + (fbm(terrain_point * 2.4 + 31.0) - 0.5) * 0.15;
                 float pacific = smoothstep(0.34, 0.52, pacific_field);
                 float islands = smoothstep(0.83, 0.94, detail + ridges * 0.25) * pacific;
                 return land * (0.006 + detail * 0.008 + ridges * ridges * 0.035) * (1.0 - pacific + islands);
               }
               float cloud_density(vec3 local) {
                 vec3 cloud_point = normalize(local);
                 float broad = fbm(cloud_point * 3.0 + 7.0);
                 float wisps = fbm(cloud_point * 12.0 - 3.0);
                 return smoothstep(0.58, 0.70, broad + wisps * 0.18);
               }
               float rayleigh_phase(float cosine) { return 0.0596831 * (1.0 + cosine * cosine); }
               float mie_phase(float cosine, float anisotropy) {
                 float g2 = anisotropy * anisotropy;
                 return (1.0 - g2) / (12.56637 * pow(max(0.001, 1.0 + g2 - 2.0 * anisotropy * cosine), 1.5));
               }
               // Tests whether a body sits between this point and the directional sun.
               // The feather is deliberate: the sun is not a mathematical point, so an
               // eclipse must retain a narrow penumbra instead of a pixel-sharp edge.
               float eclipse_shadow(vec3 point, vec3 light_direction, vec3 body, float radius, float feather) {
                 float distance_to_body_along_light = dot(body - point, light_direction);
                 vec3 nearest = point + light_direction * max(0.0, distance_to_body_along_light);
                 float miss_distance = length(nearest - body);
                 float blocked = 1.0 - smoothstep(radius - feather, radius + feather, miss_distance);
                 return blocked * step(0.0001, distance_to_body_along_light);
               }
               void main() {
                 if (u_kind > 4.5) { gl_FragColor = vec4(vec3(0.65 + fract(v_world.x * 9.0) * 0.35), 1.0); return; }
                 vec3 normal = normalize(v_normal);
                 vec3 light_direction = normalize(u_sun - v_world);
                 vec3 view_direction = normalize(u_camera - v_world);
                 float sunlight = max(dot(normal, light_direction), 0.0);
                 float lunar_shadow = eclipse_shadow(v_world, light_direction, u_moon, 0.34, 0.026);
                 if (u_kind > 3.5) {
                   vec3 room = vec3(0.035, 0.05, 0.085) * (0.35 + sunlight * 0.65);
                   gl_FragColor = vec4(room, 1.0); return;
                 }
                 vec3 local = normalize(v_local_normal);
                 if (u_kind > 1.5 && u_kind < 2.5) {
                   // A compact approximation of single-scattering: blue molecular light is
                   // broad around the limb, while aerosols make a bright, forward sun halo.
                   float view_cosine = max(dot(normal, view_direction), 0.0);
                   float optical_depth = pow(1.0 - view_cosine, 3.6);
                   float scattering_angle = dot(-light_direction, view_direction);
                   float rayleigh = rayleigh_phase(scattering_angle);
                   float mie = mie_phase(scattering_angle, 0.76);
                   float horizon_sun = smoothstep(-0.18, 0.32, dot(normal, light_direction));
                   vec3 blue_scatter = vec3(0.14, 0.42, 1.0) * rayleigh * (0.55 + horizon_sun * 0.75);
                   vec3 warm_mie = vec3(1.0, 0.54, 0.20) * mie * (1.0 - horizon_sun) * 0.38;
                   vec3 atmosphere = (blue_scatter + warm_mie) * optical_depth * 5.2;
                   float alpha = clamp(optical_depth * (0.24 + mie * 0.7), 0.0, 0.42);
                   gl_FragColor = vec4(atmosphere, alpha); return;
                 }
                 if (u_kind > 0.5 && u_kind < 1.5) {
                   float cloud = cloud_density(local);
                   float forward_scatter = mie_phase(dot(-light_direction, view_direction), 0.78) * 0.55;
                   float cloud_top = pow(max(dot(normal, light_direction), 0.0), 0.7);
                   vec3 cloud_light = vec3(0.09, 0.12, 0.18) + vec3(0.88, 0.92, 0.98) * cloud_top * (1.0 - lunar_shadow);
                   cloud_light += vec3(1.0, 0.72, 0.42) * forward_scatter;
                   gl_FragColor = vec4(cloud_light, cloud * (0.16 + cloud_top * 0.42)); return;
                 }
                 if (u_kind > -0.5 && u_kind < 0.5) {
                   vec3 terrain_point = local;
                   float continents = fbm(terrain_point * 1.65);
                   float detail = fbm(terrain_point * 7.0);
                   float ridges = 1.0 - abs(2.0 * fbm(terrain_point * 4.0 + 3.1) - 1.0);
                   float elevation = continents * 0.78 + detail * 0.08 + ridges * ridges * 0.16;
                   float terrain_height = terrain_elevation(local);
                   float pacific_field = dot(terrain_point, normalize(vec3(0.14, 0.02, 0.99)))
                     + (fbm(terrain_point * 2.4 + 31.0) - 0.5) * 0.15;
                   float pacific = smoothstep(0.34, 0.52, pacific_field);
                   float islands = smoothstep(0.83, 0.94, detail + ridges * 0.25) * pacific;
                   float ocean = max(1.0 - smoothstep(0.47, 0.53, elevation), pacific * (1.0 - islands));
                   float lakes = smoothstep(0.52, 0.57, elevation) * (1.0 - smoothstep(0.59, 0.65, elevation)) * smoothstep(0.58, 0.76, detail);
                   float water_depth = clamp((0.70 - elevation) * 1.55, 0.0, 1.0);
                   float wave = fbm(terrain_point * 34.0 + 9.0);
                   vec3 deep_water = vec3(0.004, 0.028, 0.10);
                   vec3 shallow_water = vec3(0.015, 0.34, 0.52);
                   vec3 water = mix(shallow_water, deep_water, water_depth);
                   water += (wave - 0.5) * vec3(0.018, 0.045, 0.060);
                   vec3 lowland = mix(vec3(0.035, 0.16, 0.075), vec3(0.24, 0.28, 0.08), detail);
                   vec3 mountain = mix(vec3(0.22, 0.21, 0.17), vec3(0.43, 0.38, 0.30), ridges);
                   vec3 land = mix(lowland, mountain, smoothstep(0.59, 0.80, elevation));
                   vec3 base = mix(water, land, 1.0 - ocean);
                   base = mix(base, shallow_water, lakes * (1.0 - ocean));
                   float latitude = abs(terrain_point.y);
                   float snow = smoothstep(0.70, 0.90, latitude + max(0.0, elevation - 0.56) * 1.85);
                   // Permanent ice sheets at both poles have irregular coastlines. Their
                   // tongues follow high relief and extend downslope beyond the cap edge.
                   float north_margin = 0.64 + (fbm(terrain_point * 7.0 + 17.0) - 0.5) * 0.15;
                   float south_margin = 0.61 + (fbm(terrain_point * 7.0 - 23.0) - 0.5) * 0.17;
                   float north_cap = smoothstep(north_margin, north_margin + 0.075, terrain_point.y);
                   float south_cap = smoothstep(south_margin, south_margin + 0.075, -terrain_point.y);
                   float longitude = atan(terrain_point.z, terrain_point.x);
                   float tongue_lanes = 0.5 + 0.5 * sin(longitude * 17.0 + fbm(terrain_point * 10.0) * 8.0);
                   float glacier_tongues = smoothstep(0.68, 0.88, latitude + (tongue_lanes - 0.5) * 0.20 + terrain_height * 1.9);
                   glacier_tongues *= smoothstep(0.055, 0.11, terrain_height) * (1.0 - ocean);
                   float glacier = max(max(north_cap, south_cap), glacier_tongues);
                   float crevasse = fbm(terrain_point * 28.0 + vec3(4.0, 0.0, -8.0));
                   vec3 ice = mix(vec3(0.40, 0.70, 0.88), vec3(0.96, 0.985, 1.0), smoothstep(0.34, 0.72, crevasse));
                   base = mix(base, vec3(0.93, 0.96, 1.0), snow);
                   base = mix(base, ice, glacier);
                   float terminator = smoothstep(-0.19, 0.25, dot(normal, light_direction));
                   vec3 reflected = reflect(-light_direction, normal);
                   float fresnel = pow(1.0 - max(dot(normal, view_direction), 0.0), 5.0);
                   base = mix(base, mix(water, vec3(0.20, 0.43, 0.67), fresnel * 0.60), ocean);
                   float specular = pow(max(dot(reflected, view_direction), 0.0), 76.0) * ocean * (0.45 + wave * 0.75);
                   float slope_shadow = 0.62 + 0.38 * max(dot(normal, light_direction), 0.0);
                   vec3 lit = base * (0.010 + terminator * 1.16 * (1.0 - lunar_shadow)) * slope_shadow;
                   lit += vec3(0.5, 0.72, 1.0) * specular * (1.0 - lunar_shadow);
                   lit += vec3(0.10, 0.28, 0.75) * pow(1.0 - max(dot(normal, view_direction), 0.0), 4.0) * (0.25 + terminator * 0.45);
                   gl_FragColor = vec4(lit, 1.0); return;
                 }
                 float crater = fbm(local * 9.0);
                 vec3 planet_center = vec3(0.0, 1.55, 0.0);
                 float earth_shadow = eclipse_shadow(v_world, light_direction, planet_center, 1.62, 0.085);
                 float earthshine = pow(max(dot(normal, normalize(planet_center - v_world)), 0.0), 0.75) * 0.045;
                 vec3 moon = mix(vec3(0.16), vec3(0.48), crater) * (0.008 + sunlight * (1.0 - earth_shadow) * 1.05 + earthshine);
                 gl_FragColor = vec4(moon, 1.0);
               }"#,
        )?;
        let program = link_program(&gl, &vertex, &fragment)?;
        // The density supports subtle relief and distinct cloud-body silhouettes.
        let sphere = sphere_vertices(96, 64);
        let room = planetarium_room_vertices();
        let stars = star_vertices();
        let sphere_buffer = upload_vertices(&gl, &sphere)?;
        let room_buffer = upload_vertices(&gl, &room)?;
        let star_buffer = upload_vertices(&gl, &stars)?;
        let model_uniform = required_uniform(&gl, &program, "u_model")?;
        let view_projection_uniform = required_uniform(&gl, &program, "u_view_projection")?;
        let camera_uniform = required_uniform(&gl, &program, "u_camera")?;
        let sun_uniform = required_uniform(&gl, &program, "u_sun")?;
        let moon_uniform = required_uniform(&gl, &program, "u_moon")?;
        let kind_uniform = required_uniform(&gl, &program, "u_kind")?;
        gl.enable(Gl::DEPTH_TEST);
        gl.depth_func(Gl::LEQUAL);
        Ok(Self {
            canvas: canvas.clone(),
            gl,
            program,
            sphere_buffer,
            room_buffer,
            star_buffer,
            model_uniform,
            view_projection_uniform,
            camera_uniform,
            sun_uniform,
            moon_uniform,
            kind_uniform,
            sphere_count: (sphere.len() / 6) as i32,
            room_count: (room.len() / 6) as i32,
            star_count: (stars.len() / 6) as i32,
        })
    }

    pub(super) fn render(&self, player: PlayerState, now: f64) {
        let (width, height) = resize_canvas(&self.canvas);
        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.clear_color(0.002, 0.004, 0.012, 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        self.gl.use_program(Some(&self.program));
        let (forward_x, forward_z) = player.forward_vector();
        let pitch = player.pitch_degrees.to_radians();
        let eye = [player.x, player.camera_y(), player.z];
        let center = [
            eye[0] + forward_x * pitch.cos(),
            eye[1] - pitch.sin(),
            eye[2] + forward_z * pitch.cos(),
        ];
        let view_projection = Mat4::perspective(
            72.0_f32.to_radians(),
            width as f32 / height as f32,
            0.03,
            80.0,
        )
        .multiply(Mat4::look_at(eye, center, [0.0, 1.0, 0.0]));
        let time = now as f32 / 1000.0;
        self.gl
            .uniform3f(Some(&self.camera_uniform), eye[0], eye[1], eye[2]);
        let sun = [-6.0, 5.0, 3.0];
        let planet_center = [0.0, 1.55, 0.0];
        self.gl
            .uniform3f(Some(&self.sun_uniform), sun[0], sun[1], sun[2]);
        // Keep the lunar orbit in the virtual sun's plane so visitors can witness the
        // planet's umbra crossing the Moon and the reciprocal lunar shadow on the planet.
        let sun_axis = normalize([
            sun[0] - planet_center[0],
            sun[1] - planet_center[1],
            sun[2] - planet_center[2],
        ]);
        let orbit_axis = normalize([-sun_axis[2], 0.0, sun_axis[0]]);
        let orbit_perpendicular = cross(sun_axis, orbit_axis);
        let moon_angle = time * 0.45;
        let moon = [
            planet_center[0]
                + (orbit_axis[0] * moon_angle.cos() + orbit_perpendicular[0] * moon_angle.sin())
                    * 2.65,
            planet_center[1]
                + (orbit_axis[1] * moon_angle.cos() + orbit_perpendicular[1] * moon_angle.sin())
                    * 2.65,
            planet_center[2]
                + (orbit_axis[2] * moon_angle.cos() + orbit_perpendicular[2] * moon_angle.sin())
                    * 2.65,
        ];
        self.gl
            .uniform3f(Some(&self.moon_uniform), moon[0], moon[1], moon[2]);
        self.draw(
            &self.star_buffer,
            self.star_count,
            Gl::POINTS,
            Mat4::identity(),
            5.0,
            view_projection,
        );
        self.draw(
            &self.room_buffer,
            self.room_count,
            Gl::TRIANGLES,
            Mat4::identity(),
            4.0,
            view_projection,
        );
        let planet = Mat4::translation_scale([0.0, 1.55, 0.0], 1.35);
        self.draw(
            &self.sphere_buffer,
            self.sphere_count,
            Gl::TRIANGLES,
            planet,
            0.0,
            view_projection,
        );
        let moon = Mat4::translation_scale(moon, 0.34);
        self.draw(
            &self.sphere_buffer,
            self.sphere_count,
            Gl::TRIANGLES,
            moon,
            3.0,
            view_projection,
        );
        self.gl.enable(Gl::BLEND);
        self.gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);
        self.gl.depth_mask(false);
        self.draw(
            &self.sphere_buffer,
            self.sphere_count,
            Gl::TRIANGLES,
            // Only cloud cells have geometry and they sit just above the restrained terrain.
            Mat4::translation_scale([0.0, 1.55, 0.0], 1.40),
            1.0,
            view_projection,
        );
        self.gl.depth_mask(true);
        self.gl.disable(Gl::BLEND);
    }

    fn draw(
        &self,
        buffer: &WebGlBuffer,
        count: i32,
        primitive: u32,
        model: Mat4,
        kind: f32,
        view_projection: Mat4,
    ) {
        self.gl.bind_buffer(Gl::ARRAY_BUFFER, Some(buffer));
        let position = self.gl.get_attrib_location(&self.program, "a_position") as u32;
        let normal = self.gl.get_attrib_location(&self.program, "a_normal") as u32;
        self.gl.enable_vertex_attrib_array(position);
        self.gl
            .vertex_attrib_pointer_with_i32(position, 3, Gl::FLOAT, false, 24, 0);
        self.gl.enable_vertex_attrib_array(normal);
        self.gl
            .vertex_attrib_pointer_with_i32(normal, 3, Gl::FLOAT, false, 24, 12);
        self.gl
            .uniform_matrix4fv_with_f32_array(Some(&self.model_uniform), false, &model.0);
        self.gl.uniform_matrix4fv_with_f32_array(
            Some(&self.view_projection_uniform),
            false,
            &view_projection.0,
        );
        self.gl.uniform1f(Some(&self.kind_uniform), kind);
        self.gl.draw_arrays(primitive, 0, count);
    }
}
