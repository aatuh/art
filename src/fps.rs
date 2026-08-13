//! Deterministic first-person movement and room collision rules.
//!
//! The browser adapter owns events and rendering; this module owns only the player state.

use serde::{Deserialize, Serialize};

pub const ROOM_HALF_EXTENT: f32 = 8.0;
pub const PLAYER_RADIUS: f32 = 0.35;
pub const STANDING_EYE_HEIGHT: f32 = 1.7;
pub const CROUCHING_EYE_HEIGHT: f32 = 1.1;
const CUBE_HALF_EXTENT: f32 = 1.0;
const WALK_SPEED: f32 = 4.6;
const CROUCH_SPEED: f32 = 2.25;
const CROUCH_TRANSITION_PER_SECOND: f32 = 5.0;
const ACCELERATION: f32 = 32.0;
const DECELERATION: f32 = 38.0;
const GRAVITY: f32 = 18.0;
const JUMP_VELOCITY: f32 = 5.4;
const MAX_FRAME_SECONDS: f32 = 0.05;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub forward: String,
    pub backward: String,
    pub left: String,
    pub right: String,
    pub jump: String,
    pub crouch: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            forward: "KeyW".into(),
            backward: "KeyS".into(),
            left: "KeyA".into(),
            right: "KeyD".into(),
            jump: "Space".into(),
            crouch: "ControlLeft".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FpsSettings {
    pub mouse_sensitivity: f32,
    pub invert_mouse_y: bool,
    pub bindings: KeyBindings,
}

impl Default for FpsSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.14,
            invert_mouse_y: false,
            bindings: KeyBindings::default(),
        }
    }
}

impl FpsSettings {
    pub fn from_storage(value: Option<&str>) -> Self {
        value
            .and_then(|value| serde_json::from_str::<Self>(value).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.mouse_sensitivity = self.mouse_sensitivity.clamp(0.03, 0.8);
        let defaults = KeyBindings::default();
        let mut used = Vec::new();
        for (binding, default) in [
            (&mut self.bindings.forward, defaults.forward),
            (&mut self.bindings.backward, defaults.backward),
            (&mut self.bindings.left, defaults.left),
            (&mut self.bindings.right, defaults.right),
            (&mut self.bindings.jump, defaults.jump),
            (&mut self.bindings.crouch, defaults.crouch),
        ] {
            if !is_supported_key_code(binding) || used.iter().any(|used| used == binding) {
                *binding = first_available_binding(&default, &used).to_owned();
            }
            used.push(binding.clone());
        }
        self
    }
}

fn first_available_binding<'a>(preferred: &'a str, used: &[String]) -> &'a str {
    [
        preferred,
        "KeyW",
        "KeyS",
        "KeyA",
        "KeyD",
        "Space",
        "ControlLeft",
        "KeyQ",
        "KeyE",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
    ]
    .into_iter()
    .find(|candidate| !used.iter().any(|used| used == candidate))
    .unwrap_or("KeyZ")
}

pub fn is_supported_key_code(code: &str) -> bool {
    matches!(
        code,
        "Space" | "ControlLeft" | "ControlRight" | "ShiftLeft" | "ShiftRight"
    ) || (code.len() == 4 && code.starts_with("Key") && code.as_bytes()[3].is_ascii_uppercase())
        || matches!(code, "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight")
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub crouch: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerState {
    pub x: f32,
    pub z: f32,
    pub jump_height: f32,
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    velocity_x: f32,
    velocity_z: f32,
    vertical_velocity: f32,
    pub grounded: bool,
    pub crouching: bool,
    pub crouch_amount: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            x: 0.0,
            z: 5.8,
            jump_height: 0.0,
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
            velocity_x: 0.0,
            velocity_z: 0.0,
            vertical_velocity: 0.0,
            grounded: true,
            crouching: false,
            crouch_amount: 0.0,
        }
    }
}

impl PlayerState {
    pub fn eye_height(self) -> f32 {
        STANDING_EYE_HEIGHT + (CROUCHING_EYE_HEIGHT - STANDING_EYE_HEIGHT) * self.crouch_amount
    }

    pub fn camera_y(self) -> f32 {
        self.eye_height() + self.jump_height
    }

    pub fn look(&mut self, delta_x: f32, delta_y: f32, settings: &FpsSettings) {
        self.yaw_degrees =
            (self.yaw_degrees + delta_x * settings.mouse_sensitivity).rem_euclid(360.0);
        let vertical_direction = if settings.invert_mouse_y { -1.0 } else { 1.0 };
        self.pitch_degrees = (self.pitch_degrees
            + delta_y * settings.mouse_sensitivity * vertical_direction)
            .clamp(-89.0, 89.0);
    }

    pub fn request_jump(&mut self) {
        if self.grounded {
            self.grounded = false;
            self.vertical_velocity = JUMP_VELOCITY;
        }
    }

    pub fn tick(&mut self, frame_seconds: f32, input: FpsInput) {
        let dt = frame_seconds.clamp(0.0, MAX_FRAME_SECONDS);
        self.crouching = input.crouch;
        self.crouch_amount = approach(
            self.crouch_amount,
            if input.crouch { 1.0 } else { 0.0 },
            CROUCH_TRANSITION_PER_SECOND * dt,
        );

        let forward_axis = axis(input.forward, input.backward);
        let right_axis = axis(input.right, input.left);
        let (forward_x, forward_z) = self.forward_vector();
        let right_x = -forward_z;
        let right_z = forward_x;
        let mut desired_x = forward_x * forward_axis + right_x * right_axis;
        let mut desired_z = forward_z * forward_axis + right_z * right_axis;
        let direction_length = desired_x.hypot(desired_z);
        if direction_length > 1.0 {
            desired_x /= direction_length;
            desired_z /= direction_length;
        }
        let speed = WALK_SPEED + (CROUCH_SPEED - WALK_SPEED) * self.crouch_amount;
        desired_x *= speed;
        desired_z *= speed;
        let response = if direction_length > 0.0 {
            ACCELERATION
        } else {
            DECELERATION
        };
        self.velocity_x = approach(self.velocity_x, desired_x, response * dt);
        self.velocity_z = approach(self.velocity_z, desired_z, response * dt);
        self.move_horizontally(self.velocity_x * dt, self.velocity_z * dt);

        if !self.grounded {
            self.vertical_velocity -= GRAVITY * dt;
            self.jump_height += self.vertical_velocity * dt;
            if self.jump_height <= 0.0 {
                self.jump_height = 0.0;
                self.vertical_velocity = 0.0;
                self.grounded = true;
            }
        }
    }

    pub fn forward_vector(self) -> (f32, f32) {
        let yaw = self.yaw_degrees.to_radians();
        (yaw.sin(), -yaw.cos())
    }

    fn move_horizontally(&mut self, delta_x: f32, delta_z: f32) {
        let limit = ROOM_HALF_EXTENT - PLAYER_RADIUS;
        let next_x = (self.x + delta_x).clamp(-limit, limit);
        if !intersects_cube(next_x, self.z) {
            self.x = next_x;
        }
        let next_z = (self.z + delta_z).clamp(-limit, limit);
        if !intersects_cube(self.x, next_z) {
            self.z = next_z;
        }
    }
}

fn axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

fn approach(current: f32, target: f32, delta: f32) -> f32 {
    if current < target {
        (current + delta).min(target)
    } else {
        (current - delta).max(target)
    }
}

fn intersects_cube(x: f32, z: f32) -> bool {
    x > -CUBE_HALF_EXTENT - PLAYER_RADIUS
        && x < CUBE_HALF_EXTENT + PLAYER_RADIUS
        && z > -CUBE_HALF_EXTENT - PLAYER_RADIUS
        && z < CUBE_HALF_EXTENT + PLAYER_RADIUS
}
