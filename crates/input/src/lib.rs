// SPDX-License-Identifier: MIT
//! Orbe's input layer: devices report [`Input`]s, games ask about named actions.
//!
//! An [`InputMap`] binds each action name to any number of inputs. [`InputState`] receives
//! [`Event`]s from the platform and answers `pressed`, `just_pressed`, `just_released` and
//! `strength` for any action. Call [`InputState::begin_frame`] once per frame, before feeding that
//! frame's events, so "just" states last exactly one frame.
//!
//! Hostile values (NaN or infinite axis readings, an enormous number of actions or bindings) are
//! ignored or refused; nothing here panics.

use orbe_math::Vec2;
use std::collections::{BTreeMap, BTreeSet};

/// Most actions a map may hold.
pub const MAX_ACTIONS: usize = 4096;
/// Most bindings one action may hold.
pub const MAX_BINDINGS: usize = 64;

/// A physical input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Input {
    /// A keyboard key by its platform-independent code.
    Key(u32),
    /// A mouse button.
    MouseButton(u8),
    /// A gamepad button.
    PadButton(u8),
    /// One direction of a gamepad axis: `negative` is the below-zero half.
    PadAxis {
        /// Axis number.
        axis: u8,
        /// Whether this is the negative direction.
        negative: bool,
    },
}

/// Something a device reported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A button or key went down or up.
    Button {
        /// Which one; never an axis direction.
        input: Input,
        /// Whether it is now held.
        down: bool,
    },
    /// A gamepad axis moved to `value` (-1 to 1).
    Axis {
        /// Axis number.
        axis: u8,
        /// Position.
        value: f32,
    },
    /// The mouse moved to this position in window coordinates.
    MousePosition(Vec2),
}

/// Why a binding was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindError {
    /// The action name is empty.
    EmptyName,
    /// The map already has [`MAX_ACTIONS`] actions.
    TooManyActions,
    /// The action already has [`MAX_BINDINGS`] bindings.
    TooManyBindings,
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BindError::EmptyName => "an action needs a name",
            BindError::TooManyActions => "too many actions",
            BindError::TooManyBindings => "too many bindings for one action",
        })
    }
}

impl std::error::Error for BindError {}

/// Which inputs trigger which actions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputMap {
    actions: BTreeMap<String, Vec<Input>>,
    deadzone: f32,
}

impl InputMap {
    /// An empty map with a deadzone of 0.2.
    pub fn new() -> InputMap {
        InputMap {
            actions: BTreeMap::new(),
            deadzone: 0.2,
        }
    }

    /// Sets the axis deadzone, from 0 up to but not including 1; other values are ignored.
    pub fn set_deadzone(&mut self, deadzone: f32) {
        if (0.0..1.0).contains(&deadzone) {
            self.deadzone = deadzone;
        }
    }

    /// Binds `input` to `action`, creating the action when needed. Binding twice is harmless.
    pub fn bind(&mut self, action: &str, input: Input) -> Result<(), BindError> {
        if action.is_empty() {
            return Err(BindError::EmptyName);
        }
        if !self.actions.contains_key(action) && self.actions.len() >= MAX_ACTIONS {
            return Err(BindError::TooManyActions);
        }
        let list = self.actions.entry(action.to_string()).or_default();
        if list.contains(&input) {
            return Ok(());
        }
        if list.len() >= MAX_BINDINGS {
            return Err(BindError::TooManyBindings);
        }
        list.push(input);
        Ok(())
    }

    /// Removes one binding. Returns whether it existed.
    pub fn unbind(&mut self, action: &str, input: Input) -> bool {
        match self.actions.get_mut(action) {
            Some(list) => {
                let before = list.len();
                list.retain(|i| *i != input);
                list.len() != before
            }
            None => false,
        }
    }

    /// Removes an action and all its bindings.
    pub fn remove_action(&mut self, action: &str) -> bool {
        self.actions.remove(action).is_some()
    }

    /// The inputs bound to an action.
    pub fn bindings(&self, action: &str) -> &[Input] {
        self.actions.get(action).map_or(&[], Vec::as_slice)
    }

    /// The action names, in order.
    pub fn actions(&self) -> impl Iterator<Item = &str> {
        self.actions.keys().map(String::as_str)
    }
}

/// What is held now and what changed this frame.
#[derive(Clone, Debug, Default)]
pub struct InputState {
    held: BTreeSet<Input>,
    pressed_now: BTreeSet<Input>,
    released_now: BTreeSet<Input>,
    axes: BTreeMap<u8, f32>,
    mouse: Vec2,
}

impl InputState {
    /// A state with nothing held.
    pub fn new() -> InputState {
        InputState::default()
    }

    /// Starts a frame: forgets which inputs went down or up during the previous one.
    pub fn begin_frame(&mut self) {
        self.pressed_now.clear();
        self.released_now.clear();
    }

    /// Feeds one event.
    pub fn handle(&mut self, event: Event) {
        match event {
            Event::Button { input, down } => {
                if matches!(input, Input::PadAxis { .. }) {
                    return;
                }
                if down {
                    if self.held.insert(input) {
                        self.pressed_now.insert(input);
                    }
                } else if self.held.remove(&input) {
                    self.released_now.insert(input);
                }
            }
            Event::Axis { axis, value } => {
                if value.is_finite() {
                    self.axes.insert(axis, value.clamp(-1.0, 1.0));
                }
            }
            Event::MousePosition(p) => {
                if p.is_finite() {
                    self.mouse = p;
                }
            }
        }
    }

    /// The last mouse position.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse
    }

    /// How strongly one input is active, 0 to 1. Buttons are 0 or 1; axis directions are rescaled
    /// so the deadzone reads as 0 and full deflection as 1.
    fn input_strength(&self, input: Input, deadzone: f32) -> f32 {
        match input {
            Input::PadAxis { axis, negative } => {
                let raw = self.axes.get(&axis).copied().unwrap_or(0.0);
                let v = if negative { -raw } else { raw };
                if v <= deadzone {
                    0.0
                } else {
                    ((v - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
                }
            }
            other => f32::from(u8::from(self.held.contains(&other))),
        }
    }

    /// The strongest activation among the action's inputs, 0 to 1.
    pub fn strength(&self, map: &InputMap, action: &str) -> f32 {
        map.bindings(action)
            .iter()
            .map(|&i| self.input_strength(i, map.deadzone))
            .fold(0.0, f32::max)
    }

    /// Whether the action is active.
    pub fn pressed(&self, map: &InputMap, action: &str) -> bool {
        self.strength(map, action) > 0.0
    }

    /// Whether a button bound to the action went down this frame.
    pub fn just_pressed(&self, map: &InputMap, action: &str) -> bool {
        map.bindings(action)
            .iter()
            .any(|i| self.pressed_now.contains(i))
    }

    /// Whether a button bound to the action went up this frame.
    pub fn just_released(&self, map: &InputMap, action: &str) -> bool {
        map.bindings(action)
            .iter()
            .any(|i| self.released_now.contains(i))
    }

    /// A signed reading from two actions: `positive` minus `negative`, from -1 to 1.
    pub fn axis(&self, map: &InputMap, negative: &str, positive: &str) -> f32 {
        self.strength(map, positive) - self.strength(map, negative)
    }

    /// A direction from four actions, with diagonals limited to length 1.
    pub fn vector(&self, map: &InputMap, left: &str, right: &str, up: &str, down: &str) -> Vec2 {
        let v = Vec2::new(self.axis(map, left, right), self.axis(map, up, down));
        if v.length() > 1.0 {
            v.normalized().unwrap_or(Vec2::ZERO)
        } else {
            v
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPACE: Input = Input::Key(32);

    fn down(i: Input) -> Event {
        Event::Button {
            input: i,
            down: true,
        }
    }
    fn up(i: Input) -> Event {
        Event::Button {
            input: i,
            down: false,
        }
    }

    #[test]
    fn buttons_have_press_and_release_edges() {
        let mut map = InputMap::new();
        map.bind("jump", SPACE).unwrap();
        map.bind("jump", Input::PadButton(0)).unwrap();
        let mut s = InputState::new();
        s.begin_frame();
        s.handle(down(SPACE));
        assert!(s.pressed(&map, "jump") && s.just_pressed(&map, "jump"));
        s.begin_frame();
        s.handle(down(SPACE)); // key repeat does not count as a new press
        assert!(s.pressed(&map, "jump") && !s.just_pressed(&map, "jump"));
        s.begin_frame();
        s.handle(up(SPACE));
        assert!(!s.pressed(&map, "jump") && s.just_released(&map, "jump"));
        s.begin_frame();
        assert!(!s.just_released(&map, "jump"));
        assert!(!s.pressed(&map, "unknown"));
    }

    #[test]
    fn axes_apply_the_deadzone() {
        let mut map = InputMap::new();
        map.set_deadzone(0.5);
        map.bind(
            "right",
            Input::PadAxis {
                axis: 0,
                negative: false,
            },
        )
        .unwrap();
        map.bind(
            "left",
            Input::PadAxis {
                axis: 0,
                negative: true,
            },
        )
        .unwrap();
        let mut s = InputState::new();
        s.handle(Event::Axis {
            axis: 0,
            value: 0.4,
        });
        assert_eq!(s.strength(&map, "right"), 0.0);
        s.handle(Event::Axis {
            axis: 0,
            value: 0.75,
        });
        assert!((s.strength(&map, "right") - 0.5).abs() < 1e-6);
        s.handle(Event::Axis {
            axis: 0,
            value: -1.0,
        });
        assert_eq!(s.strength(&map, "left"), 1.0);
        assert_eq!(s.axis(&map, "left", "right"), -1.0);
        s.handle(Event::Axis {
            axis: 0,
            value: f32::NAN,
        });
        assert_eq!(
            s.axis(&map, "left", "right"),
            -1.0,
            "NaN readings are ignored"
        );
        s.handle(Event::Axis {
            axis: 0,
            value: 50.0,
        });
        assert_eq!(s.strength(&map, "right"), 1.0);
    }

    #[test]
    fn vectors_are_limited_to_unit_length() {
        let mut map = InputMap::new();
        for (a, k) in [("l", 1), ("r", 2), ("u", 3), ("d", 4)] {
            map.bind(a, Input::Key(k)).unwrap();
        }
        let mut s = InputState::new();
        s.handle(down(Input::Key(2)));
        s.handle(down(Input::Key(4)));
        let v = s.vector(&map, "l", "r", "u", "d");
        assert!((v.length() - 1.0).abs() < 1e-5 && v.x > 0.0 && v.y > 0.0);
        s.handle(up(Input::Key(4)));
        assert_eq!(s.vector(&map, "l", "r", "u", "d"), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn map_editing_and_limits() {
        let mut map = InputMap::new();
        assert_eq!(map.bind("", SPACE), Err(BindError::EmptyName));
        map.bind("a", SPACE).unwrap();
        map.bind("a", SPACE).unwrap();
        assert_eq!(map.bindings("a").len(), 1);
        assert!(map.unbind("a", SPACE) && !map.unbind("a", SPACE) && !map.unbind("zzz", SPACE));
        for k in 0..MAX_BINDINGS as u32 {
            map.bind("a", Input::Key(k)).unwrap();
        }
        assert_eq!(
            map.bind("a", Input::Key(9999)),
            Err(BindError::TooManyBindings)
        );
        for n in 0..MAX_ACTIONS {
            let _ = map.bind(&format!("act{n}"), SPACE);
        }
        assert_eq!(map.bind("one more", SPACE), Err(BindError::TooManyActions));
        assert!(map.remove_action("a"));
        map.set_deadzone(f32::NAN);
        map.set_deadzone(1.0);
        assert_eq!(map.actions().count(), MAX_ACTIONS - 1);
        let mut s = InputState::new();
        s.handle(Event::Button {
            input: Input::PadAxis {
                axis: 0,
                negative: false,
            },
            down: true,
        });
        s.handle(Event::MousePosition(Vec2::new(f32::NAN, 0.0)));
        assert_eq!(s.mouse_position(), Vec2::ZERO);
        s.handle(Event::MousePosition(Vec2::new(3.0, 4.0)));
        assert_eq!(s.mouse_position(), Vec2::new(3.0, 4.0));
    }
}
