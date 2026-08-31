//! Deterministic first-person movement for the authored room.

use crate::scene::{BLACK_CUBE_SCENE, RoomScene};

pub const PLAYER_RADIUS: f32 = 0.35;
pub const STANDING_EYE_HEIGHT: f32 = 1.7;
pub const CROUCHING_EYE_HEIGHT: f32 = 1.1;
const WALK_SPEED: f32 = 4.6;
const CROUCH_SPEED: f32 = 2.25;
const CROUCH_TRANSITION_PER_SECOND: f32 = 5.0;
const ACCELERATION: f32 = 32.0;
const DECELERATION: f32 = 38.0;
const GRAVITY: f32 = 18.0;
const JUMP_VELOCITY: f32 = 5.4;
const MAX_FRAME_SECONDS: f32 = 0.05;

/// Logical room actions supplied by an adapter.
///
/// This type intentionally knows nothing about keyboard codes or browser storage.
/// `jump_pressed` is an edge-triggered action; crouch is a held action.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump_pressed: bool,
    pub crouch: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerState {
    x: f32,
    z: f32,
    jump_height: f32,
    yaw_degrees: f32,
    pitch_degrees: f32,
    velocity_x: f32,
    velocity_z: f32,
    vertical_velocity: f32,
    grounded: bool,
    crouching: bool,
    crouch_amount: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::at_spawn(&BLACK_CUBE_SCENE)
    }
}

impl PlayerState {
    pub fn at_spawn(room: &RoomScene) -> Self {
        Self {
            x: room.spawn_x,
            z: room.spawn_z,
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

    pub fn position(&self) -> (f32, f32) {
        (self.x, self.z)
    }

    pub fn jump_height(&self) -> f32 {
        self.jump_height
    }

    pub fn yaw_degrees(&self) -> f32 {
        self.yaw_degrees
    }

    pub fn pitch_degrees(&self) -> f32 {
        self.pitch_degrees
    }

    pub fn is_grounded(&self) -> bool {
        self.grounded
    }

    pub fn is_crouching(&self) -> bool {
        self.crouching
    }

    pub fn crouch_amount(&self) -> f32 {
        self.crouch_amount
    }

    pub fn eye_height(&self) -> f32 {
        STANDING_EYE_HEIGHT + (CROUCHING_EYE_HEIGHT - STANDING_EYE_HEIGHT) * self.crouch_amount
    }

    pub fn camera_y(&self) -> f32 {
        self.eye_height() + self.jump_height
    }

    pub fn look(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        mouse_sensitivity: f32,
        invert_mouse_y: bool,
    ) {
        if !delta_x.is_finite() || !delta_y.is_finite() || !mouse_sensitivity.is_finite() {
            return;
        }

        let sensitivity = mouse_sensitivity.clamp(0.03, 0.8);
        self.yaw_degrees = (self.yaw_degrees + delta_x * sensitivity).rem_euclid(360.0);
        let vertical_direction = if invert_mouse_y { -1.0 } else { 1.0 };
        self.pitch_degrees =
            (self.pitch_degrees + delta_y * sensitivity * vertical_direction).clamp(-89.0, 89.0);
    }

    pub fn request_jump(&mut self) {
        if self.grounded {
            self.grounded = false;
            self.vertical_velocity = JUMP_VELOCITY;
        }
    }

    /// Advances player physics against the supplied room rather than hidden global geometry.
    pub fn tick(&mut self, room: &RoomScene, frame_seconds: f32, input: PlayerInput) {
        let Some(dt) = finite_frame_seconds(frame_seconds) else {
            return;
        };
        if !room.is_valid() {
            return;
        }
        if input.jump_pressed {
            self.request_jump();
        }

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
        self.move_horizontally(room, self.velocity_x * dt, self.velocity_z * dt);

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

    pub fn forward_vector(&self) -> (f32, f32) {
        let yaw = self.yaw_degrees.to_radians();
        (yaw.sin(), -yaw.cos())
    }

    fn move_horizontally(&mut self, room: &RoomScene, delta_x: f32, delta_z: f32) {
        if !room.is_valid() || room.half_extent <= PLAYER_RADIUS {
            return;
        }

        let limit = room.half_extent - PLAYER_RADIUS;
        let next_x = (self.x + delta_x).clamp(-limit, limit);
        if !room.intersects_cube(next_x, self.z, PLAYER_RADIUS) {
            self.x = next_x;
        }
        let next_z = (self.z + delta_z).clamp(-limit, limit);
        if !room.intersects_cube(self.x, next_z, PLAYER_RADIUS) {
            self.z = next_z;
        }
    }
}

fn finite_frame_seconds(frame_seconds: f32) -> Option<f32> {
    if frame_seconds.is_finite() {
        Some(frame_seconds.clamp(0.0, MAX_FRAME_SECONDS))
    } else {
        None
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

#[cfg(test)]
mod tests {
    use super::{
        CROUCHING_EYE_HEIGHT, MAX_FRAME_SECONDS, PLAYER_RADIUS, PlayerInput, PlayerState,
        STANDING_EYE_HEIGHT,
    };
    use crate::{BLACK_CUBE_SCENE, RoomScene};

    fn advance(player: &mut PlayerState, room: &RoomScene, input: PlayerInput, ticks: usize) {
        for _ in 0..ticks {
            player.tick(room, MAX_FRAME_SECONDS, input);
        }
    }

    #[test]
    fn player_spawns_at_the_authored_clear_destination() {
        let room = BLACK_CUBE_SCENE;
        let player = PlayerState::at_spawn(&room);

        assert_eq!(player.position(), (room.spawn_x, room.spawn_z));
        assert!(room.contains_player(player.position().0, player.position().1, PLAYER_RADIUS));
        assert!(!room.intersects_cube(player.position().0, player.position().1, PLAYER_RADIUS));
        assert!(player.is_grounded());
    }

    #[test]
    fn logical_wasd_actions_move_in_expected_directions() {
        let room = BLACK_CUBE_SCENE;
        let start = PlayerState::at_spawn(&room).position();
        let cases = [
            (
                PlayerInput {
                    forward: true,
                    ..PlayerInput::default()
                },
                (0.0, -1.0),
            ),
            (
                PlayerInput {
                    backward: true,
                    ..PlayerInput::default()
                },
                (0.0, 1.0),
            ),
            (
                PlayerInput {
                    left: true,
                    ..PlayerInput::default()
                },
                (-1.0, 0.0),
            ),
            (
                PlayerInput {
                    right: true,
                    ..PlayerInput::default()
                },
                (1.0, 0.0),
            ),
        ];

        for (input, expected_sign) in cases {
            let mut player = PlayerState::at_spawn(&room);
            advance(&mut player, &room, input, 6);
            let moved = (player.position().0 - start.0, player.position().1 - start.1);
            assert!(moved.0 * expected_sign.0 + moved.1 * expected_sign.1 > 0.0);
        }
    }

    #[test]
    fn jump_has_gravity_landing_and_no_midair_retrigger() {
        let room = BLACK_CUBE_SCENE;
        let mut player = PlayerState::at_spawn(&room);
        player.tick(
            &room,
            0.016,
            PlayerInput {
                jump_pressed: true,
                ..PlayerInput::default()
            },
        );
        let first_height = player.jump_height();
        let first_vertical_velocity = player.vertical_velocity;
        assert!(first_height > 0.0);
        assert!(!player.is_grounded());

        player.tick(
            &room,
            0.016,
            PlayerInput {
                jump_pressed: true,
                ..PlayerInput::default()
            },
        );
        assert!(player.jump_height() > first_height);
        assert!(player.vertical_velocity < first_vertical_velocity);

        advance(&mut player, &room, PlayerInput::default(), 100);
        assert!(player.is_grounded());
        assert_eq!(player.jump_height(), 0.0);
    }

    #[test]
    fn crouching_transitions_smoothly_and_recovers() {
        let room = BLACK_CUBE_SCENE;
        let mut player = PlayerState::at_spawn(&room);
        player.tick(
            &room,
            MAX_FRAME_SECONDS,
            PlayerInput {
                crouch: true,
                ..PlayerInput::default()
            },
        );

        assert!(player.is_crouching());
        assert!(player.crouch_amount() > 0.0 && player.crouch_amount() < 1.0);
        assert!(player.eye_height() < STANDING_EYE_HEIGHT);
        assert!(player.eye_height() > CROUCHING_EYE_HEIGHT);

        advance(
            &mut player,
            &room,
            PlayerInput {
                crouch: true,
                ..PlayerInput::default()
            },
            8,
        );
        assert_eq!(player.crouch_amount(), 1.0);
        assert_eq!(player.eye_height(), CROUCHING_EYE_HEIGHT);

        advance(&mut player, &room, PlayerInput::default(), 8);
        assert!(!player.is_crouching());
        assert_eq!(player.crouch_amount(), 0.0);
        assert_eq!(player.eye_height(), STANDING_EYE_HEIGHT);
    }

    #[test]
    fn tick_uses_the_injected_room_for_wall_collision() {
        let room = RoomScene {
            half_extent: 2.5,
            ceiling_height: 3.0,
            cube_half_extent: 0.2,
            cube_height: 1.0,
            spawn_x: 0.0,
            spawn_z: 1.5,
        };
        let mut player = PlayerState::at_spawn(&room);
        advance(
            &mut player,
            &room,
            PlayerInput {
                right: true,
                ..PlayerInput::default()
            },
            100,
        );

        assert_eq!(player.position().0, room.half_extent - PLAYER_RADIUS);
        assert!(room.contains_player(player.position().0, player.position().1, PLAYER_RADIUS));
    }

    #[test]
    fn player_cannot_walk_through_the_injected_cube_collider() {
        let room = RoomScene {
            half_extent: 4.0,
            ceiling_height: 3.0,
            cube_half_extent: 0.5,
            cube_height: 1.0,
            spawn_x: 0.0,
            spawn_z: 2.0,
        };
        let mut player = PlayerState::at_spawn(&room);
        advance(
            &mut player,
            &room,
            PlayerInput {
                forward: true,
                ..PlayerInput::default()
            },
            100,
        );

        assert!(player.position().1 >= room.cube_half_extent + PLAYER_RADIUS);
        assert!(!room.intersects_cube(player.position().0, player.position().1, PLAYER_RADIUS));
    }

    #[test]
    fn non_finite_frame_time_cannot_poison_player_state() {
        let room = BLACK_CUBE_SCENE;
        let mut player = PlayerState::at_spawn(&room);
        let before = player;

        player.tick(
            &room,
            f32::NAN,
            PlayerInput {
                forward: true,
                jump_pressed: true,
                crouch: true,
                ..PlayerInput::default()
            },
        );

        assert_eq!(player, before);
    }
}
