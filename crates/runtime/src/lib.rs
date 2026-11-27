// SPDX-License-Identifier: MIT
//! Orbe's game runtime: the glue that makes a scene alive.
//!
//! A node whose `script` property holds script source gets its own [`orbe_script::Vm`]. A [`Game`]
//! calls `_ready()` on every scripted node once, then `_process(delta)` every frame, feeding
//! scripts the input state. A script that fails is stopped and its error recorded; it never takes
//! the game down.
//!
//! Functions a script can call, besides the language's built-ins:
//!
//! | Function | Does |
//! |---|---|
//! | `get(path, key)` | A property of the node at `path` (`"."` is this node, `".."` its parent, `"/A/B"` from the root); `nil` when missing |
//! | `set(path, key, value)` | Sets a property; fails the script when the node or key is invalid |
//! | `exists(path)` | Whether that node exists |
//! | `pressed(action)`, `just_pressed(action)`, `just_released(action)` | Action state |
//! | `strength(action)` | 0 to 1 |
//! | `axis(negative, positive)` | -1 to 1 |
//! | `vector(left, right, up, down)` | A `vec2` |
//! | `mouse()` | The mouse position as a `vec2` |
//! | `time()` | Seconds since the game started |
//! | `print(value)` | Appends to [`Game::log`] |

use orbe_core::{Handle, Variant};
use orbe_input::{Event, InputMap, InputState};
use orbe_math::Vec2;
use orbe_scene::{Node, SceneTree};
use orbe_script::{Host, Script, ScriptError, Vm};

/// Most scripted nodes a game may run.
pub const MAX_SCRIPTS: usize = 10_000;
/// Most lines the log keeps; older ones are dropped.
pub const MAX_LOG_LINES: usize = 1_000;

/// A script that stopped because it failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptFailure {
    /// Path of the node the script belongs to.
    pub node: String,
    /// What went wrong.
    pub error: ScriptError,
}

struct Scripted {
    node: Handle<Node>,
    vm: Vm,
    ready_done: bool,
}

/// A running game: scene, scripts, input and time.
pub struct Game {
    tree: SceneTree,
    scripts: Vec<Scripted>,
    actions: InputMap,
    input: InputState,
    time: f64,
    log: Vec<String>,
    failures: Vec<ScriptFailure>,
}

/// The view of the game a running script gets.
struct NodeHost<'a> {
    tree: &'a mut SceneTree,
    actions: &'a InputMap,
    input: &'a InputState,
    log: &'a mut Vec<String>,
    node: Handle<Node>,
    time: f64,
}

fn text(v: &Variant) -> String {
    match v {
        Variant::Str(s) => s.clone(),
        Variant::Nil => "nil".to_string(),
        Variant::Bool(b) => b.to_string(),
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => f.to_string(),
        Variant::Vec2(v) => format!("({}, {})", v.x, v.y),
        other => format!("<{}>", other.type_name()),
    }
}

impl Host for NodeHost<'_> {
    fn call(&mut self, name: &str, args: &[Variant]) -> Option<Result<Variant, String>> {
        let action = |i: usize| match args.get(i) {
            Some(Variant::Str(s)) => Some(s.as_str()),
            _ => None,
        };
        let bad = || Some(Err(format!("{name}: wrong arguments")));
        match (name, args.len()) {
            ("get", 2) => {
                let (Some(Variant::Str(path)), Some(Variant::Str(key))) =
                    (args.first(), args.get(1))
                else {
                    return bad();
                };
                let value = self
                    .tree
                    .find(self.node, path)
                    .and_then(|h| self.tree.property(h, key))
                    .cloned()
                    .unwrap_or(Variant::Nil);
                Some(Ok(value))
            }
            ("set", 3) => {
                let (Some(Variant::Str(path)), Some(Variant::Str(key)), Some(value)) =
                    (args.first(), args.get(1), args.get(2))
                else {
                    return bad();
                };
                let Some(h) = self.tree.find(self.node, path) else {
                    return Some(Err(format!("set: no node at `{path}`")));
                };
                Some(
                    self.tree
                        .set_property(h, key, value.clone())
                        .map(|_| Variant::Nil)
                        .map_err(|e| format!("set: {e}")),
                )
            }
            ("exists", 1) => match args.first() {
                Some(Variant::Str(path)) => {
                    Some(Ok(Variant::Bool(self.tree.find(self.node, path).is_some())))
                }
                _ => bad(),
            },
            ("pressed", 1) => Some(Ok(Variant::Bool(
                self.input.pressed(self.actions, action(0)?),
            ))),
            ("just_pressed", 1) => Some(Ok(Variant::Bool(
                self.input.just_pressed(self.actions, action(0)?),
            ))),
            ("just_released", 1) => Some(Ok(Variant::Bool(
                self.input.just_released(self.actions, action(0)?),
            ))),
            ("strength", 1) => Some(Ok(Variant::Float(f64::from(
                self.input.strength(self.actions, action(0)?),
            )))),
            ("axis", 2) => Some(Ok(Variant::Float(f64::from(self.input.axis(
                self.actions,
                action(0)?,
                action(1)?,
            ))))),
            ("vector", 4) => Some(Ok(Variant::Vec2(self.input.vector(
                self.actions,
                action(0)?,
                action(1)?,
                action(2)?,
                action(3)?,
            )))),
            ("mouse", 0) => Some(Ok(Variant::Vec2(self.input.mouse_position()))),
            ("time", 0) => Some(Ok(Variant::Float(self.time))),
            ("print", 1) => {
                if self.log.len() >= MAX_LOG_LINES {
                    self.log.remove(0);
                }
                let line = args.first().map(text).unwrap_or_default();
                self.log.push(line.chars().take(1_000).collect());
                Some(Ok(Variant::Nil))
            }
            _ => None,
        }
    }
}

impl Game {
    /// Starts a game from a scene. Every node with a string `script` property gets a script;
    /// a script that does not parse is recorded in [`Game::failures`] and skipped.
    pub fn new(tree: SceneTree, actions: InputMap) -> Game {
        let mut game = Game {
            tree,
            scripts: Vec::new(),
            actions,
            input: InputState::new(),
            time: 0.0,
            log: Vec::new(),
            failures: Vec::new(),
        };
        for h in game.tree.subtree(game.tree.root()) {
            let Some(Variant::Str(source)) = game.tree.property(h, "script") else {
                continue;
            };
            if game.scripts.len() >= MAX_SCRIPTS {
                break;
            }
            match Script::parse(source) {
                Ok(script) => game.scripts.push(Scripted {
                    node: h,
                    vm: Vm::new(script),
                    ready_done: false,
                }),
                Err(error) => {
                    let node = game.tree.path_of(h).unwrap_or_default();
                    game.failures.push(ScriptFailure { node, error });
                }
            }
        }
        game
    }

    /// The scene.
    pub fn tree(&self) -> &SceneTree {
        &self.tree
    }

    /// The scene, for the editor or tests to change.
    pub fn tree_mut(&mut self) -> &mut SceneTree {
        &mut self.tree
    }

    /// Seconds simulated so far.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// What scripts printed, oldest first.
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Scripts that failed, in the order they did. A failed script no longer runs.
    pub fn failures(&self) -> &[ScriptFailure] {
        &self.failures
    }

    /// How many scripts are still running.
    pub fn script_count(&self) -> usize {
        self.scripts.len()
    }

    /// Feeds one input event; it is visible to scripts from the next [`Game::update`].
    pub fn handle(&mut self, event: Event) {
        self.input.handle(event);
    }

    /// Advances the game by `dt` seconds: new `_ready()` calls first, then `_process(dt)` on every
    /// script in tree order. Non-finite or negative `dt` counts as zero.
    pub fn update(&mut self, dt: f64) {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        self.time += dt;
        let Game {
            tree,
            scripts,
            actions,
            input,
            time,
            log,
            failures,
        } = self;
        let mut i = 0;
        while let Some(s) = scripts.get_mut(i) {
            if tree.node(s.node).is_none() {
                scripts.remove(i);
                continue;
            }
            let mut host = NodeHost {
                tree,
                actions,
                input,
                log,
                node: s.node,
                time: *time,
            };
            let mut result = Ok(());
            if !s.ready_done {
                s.ready_done = true;
                if let Err(e) = s.vm.run(&mut host) {
                    result = Err(e);
                } else if s.vm.has_function("_ready") {
                    result = s.vm.call("_ready", &[], &mut host).map(|_| ());
                }
            }
            if result.is_ok() && s.vm.has_function("_process") {
                result =
                    s.vm.call("_process", &[Variant::Float(dt)], &mut host)
                        .map(|_| ());
            }
            match result {
                Ok(()) => i += 1,
                Err(error) => {
                    let node = tree.path_of(s.node).unwrap_or_default();
                    failures.push(ScriptFailure { node, error });
                    scripts.remove(i);
                }
            }
        }
        // "Just pressed" lasts one update: clear it once scripts have seen it.
        self.input.begin_frame();
    }

    /// The mouse position, for a front end to display.
    pub fn mouse_position(&self) -> Vec2 {
        self.input.mouse_position()
    }
}

#[cfg(test)]
mod tests;
