use super::*;
use orbe_input::Input;

fn scene(script: &str) -> SceneTree {
    let mut t = SceneTree::new("Root", "Node2D").unwrap();
    let root = t.root();
    t.set_property(root, "script", Variant::Str(script.into()))
        .unwrap();
    t.set_property(root, "position", Variant::Vec2(Vec2::new(0.0, 0.0)))
        .unwrap();
    t
}

#[test]
fn ready_runs_once_then_process_every_update() {
    let t = scene(
        "let n = 0\nfn _ready() { print(\"ready\") }\n\
         fn _process(dt) { n += 1\n set(\".\", \"frames\", n) }",
    );
    let mut g = Game::new(t, InputMap::new());
    g.update(0.5);
    g.update(0.5);
    assert_eq!(g.log(), ["ready"]);
    let root = g.tree().root();
    assert_eq!(g.tree().property(root, "frames"), Some(&Variant::Int(2)));
    assert_eq!(g.time(), 1.0);
}

#[test]
fn scripts_move_nodes_from_input() {
    let mut map = InputMap::new();
    map.bind("right", Input::Key(1)).unwrap();
    map.bind("left", Input::Key(2)).unwrap();
    let t = scene(
        "fn _process(dt) {\n let p = get(\".\", \"position\")\n \
         set(\".\", \"position\", p + vec2(axis(\"left\", \"right\") * 100 * dt, 0))\n}",
    );
    let mut g = Game::new(t, map);
    g.handle(Event::Button {
        input: Input::Key(1),
        down: true,
    });
    g.update(0.5);
    g.update(0.5);
    let root = g.tree().root();
    assert_eq!(
        g.tree().property(root, "position"),
        Some(&Variant::Vec2(Vec2::new(100.0, 0.0)))
    );
    g.handle(Event::Button {
        input: Input::Key(1),
        down: false,
    });
    g.update(1.0);
    assert_eq!(
        g.tree().property(root, "position"),
        Some(&Variant::Vec2(Vec2::new(100.0, 0.0)))
    );
}

#[test]
fn just_pressed_lasts_one_update() {
    let mut map = InputMap::new();
    map.bind("jump", Input::Key(9)).unwrap();
    let t = scene("fn _process(dt) { if just_pressed(\"jump\") { print(\"jump\") } }");
    let mut g = Game::new(t, map);
    g.handle(Event::Button {
        input: Input::Key(9),
        down: true,
    });
    g.update(0.1);
    g.update(0.1);
    assert_eq!(g.log(), ["jump"]);
}

#[test]
fn failing_and_broken_scripts_are_contained() {
    let mut t = scene("fn _process(dt) { print(\"ok\") }");
    let root = t.root();
    let bad = t.add_child(root, "Bad", "Node").unwrap();
    t.set_property(
        bad,
        "script",
        Variant::Str("fn _process(dt) { 1 / 0 }".into()),
    )
    .unwrap();
    let broken = t.add_child(root, "Broken", "Node").unwrap();
    t.set_property(broken, "script", Variant::Str("let = ".into()))
        .unwrap();
    let mut g = Game::new(t, InputMap::new());
    assert_eq!(g.script_count(), 2);
    g.update(0.1);
    g.update(0.1);
    assert_eq!(g.script_count(), 1);
    assert_eq!(g.log(), ["ok", "ok"]);
    assert_eq!(g.failures().len(), 2);
    assert!(g.failures().iter().any(|f| f.node == "Bad"));
}

#[test]
fn hostile_calls_and_time_do_not_panic() {
    let t = scene("fn _process(dt) {\n print(get(1, 2))\n }\n");
    let mut g = Game::new(t, InputMap::new());
    g.update(f64::NAN);
    assert_eq!(g.time(), 0.0);
    assert_eq!(g.failures().len(), 1);
    let t = scene("fn _process(dt) { set(\"Nope\", \"x\", 1) }");
    let mut g = Game::new(t, InputMap::new());
    g.update(-5.0);
    assert!(g.failures()[0].error.message.contains("no node"));
    // A script whose node was removed stops quietly.
    let mut t = scene("");
    let root = t.root();
    let c = t.add_child(root, "C", "Node").unwrap();
    t.set_property(c, "script", Variant::Str("fn _process(dt) { }".into()))
        .unwrap();
    let mut g = Game::new(t, InputMap::new());
    g.tree_mut().remove(c).unwrap();
    g.update(0.1);
    assert_eq!(g.script_count(), 1);
}

#[test]
fn the_log_is_bounded() {
    let t = scene("fn _process(dt) { for i in range(50) { print(i) } }");
    let mut g = Game::new(t, InputMap::new());
    for _ in 0..40 {
        g.update(0.1);
    }
    assert_eq!(g.log().len(), MAX_LOG_LINES);
}
