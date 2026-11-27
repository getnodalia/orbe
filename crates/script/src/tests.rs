use super::*;
use orbe_core::Variant;

fn run(src: &str) -> Result<Vm, ScriptError> {
    let mut vm = Vm::new(Script::parse(src)?);
    vm.run(&mut NoHost)?;
    Ok(vm)
}

fn global(src: &str, name: &str) -> Variant {
    run(src).unwrap().global(name).cloned().unwrap()
}

#[test]
fn recursion_loops_and_arithmetic() {
    let src = "fn fib(n) {\n if n < 2 { return n }\n return fib(n - 1) + fib(n - 2)\n}\n\
               let total = 0\nfor i in range(10) { total += fib(i) }\n";
    assert_eq!(global(src, "total"), Variant::Int(88));
    assert_eq!(global("let x = 7 / 2", "x"), Variant::Int(3));
    assert_eq!(global("let x = 7 / 2.0", "x"), Variant::Float(3.5));
    assert_eq!(global("let x = 2 + 3 * 4 - 1", "x"), Variant::Int(13));
    assert_eq!(global("let x = (2 + 3) * 4 % 7", "x"), Variant::Int(6));
}

#[test]
fn control_flow() {
    let src = "let n = 0\nlet i = 0\nwhile true {\n i += 1\n if i > 10 { break }\n \
               if i % 2 == 0 { continue }\n n += i\n}\n";
    assert_eq!(global(src, "n"), Variant::Int(25));
    let src = "let r = \"\"\nif 1 > 2 { r = \"a\" } else if 2 > 1 { r = \"b\" } else { r = \"c\" }";
    assert_eq!(global(src, "r"), Variant::Str("b".into()));
}

#[test]
fn arrays_dictionaries_and_strings() {
    let src = "let a = [1, 2, 3]\na[1] = 20\na[2] += 5\nlet d = {\"k\": [0, 0]}\n\
               d[\"k\"][1] = 9\nd[\"new\"] = 1\nlet n = len(a) + len(d)\nlet s = \"ab\" + \"c\"";
    let vm = run(src).unwrap();
    assert_eq!(
        vm.global("a"),
        Some(&Variant::Array(vec![
            Variant::Int(1),
            Variant::Int(20),
            Variant::Int(8)
        ]))
    );
    assert_eq!(vm.global("n"), Some(&Variant::Int(5)));
    assert_eq!(vm.global("s"), Some(&Variant::Str("abc".into())));
}

#[test]
fn values_are_copied_not_shared() {
    let src = "let a = [1]\nlet b = a\nb[0] = 2\n";
    let vm = run(src).unwrap();
    assert_eq!(vm.global("a"), Some(&Variant::Array(vec![Variant::Int(1)])));
}

#[test]
fn functions_see_globals_but_not_caller_locals() {
    let src = "let g = 10\nfn f() { return g }\nfn h() { let local = 1\n return f() }\nlet r = h()";
    assert_eq!(global(src, "r"), Variant::Int(10));
    let err = run("fn f() { return local }\nfn h() { let local = 1\n return f() }\nlet r = h()")
        .err()
        .unwrap();
    assert!(err.message.contains("not defined"));
}

#[test]
fn vectors() {
    let src = "let v = vec2(1, 2) * 3 + vec2(1, 1)\nlet x = v[0]";
    assert_eq!(global(src, "x"), Variant::Float(4.0));
}

struct Counter(i64);

impl Host for Counter {
    fn call(&mut self, name: &str, args: &[Variant]) -> Option<Result<Variant, String>> {
        match (name, args) {
            ("bump", [Variant::Int(n)]) => {
                self.0 += n;
                Some(Ok(Variant::Int(self.0)))
            }
            ("fail", _) => Some(Err("host said no".into())),
            _ => None,
        }
    }
}

#[test]
fn host_functions_and_calling_script_functions() {
    let script = Script::parse("fn tick(n) { return bump(n) * 2 }\nlet unused = 0").unwrap();
    let mut vm = Vm::new(script);
    let mut host = Counter(0);
    vm.run(&mut host).unwrap();
    assert_eq!(
        vm.call("tick", &[Variant::Int(5)], &mut host),
        Ok(Variant::Int(10))
    );
    assert_eq!(host.0, 5);
    assert!(vm.call("nope", &[], &mut host).is_err());
    assert!(vm.call("tick", &[], &mut host).is_err());
    let script = Script::parse("fail()").unwrap();
    let err = Vm::new(script).run(&mut host).unwrap_err();
    assert_eq!(err.message, "host said no");
    let err = run("missing()").err().unwrap();
    assert!(err.message.contains("unknown function"));
}

#[test]
fn errors_name_the_line() {
    let err = run("let a = 1\nlet b = a / 0").err().unwrap();
    assert_eq!(err.line, 2);
    assert!(err.message.contains("division by zero"));
    let err = Script::parse("let = 1").unwrap_err();
    assert_eq!(err.line, 1);
    assert!(run("let a = 9223372036854775807 + 1").is_err());
    assert!(run("let a = [1][5]").is_err());
    assert!(run("x = 1").is_err());
}

#[test]
fn runaway_scripts_are_stopped() {
    let err = run("while true { }").err().unwrap();
    assert!(err.message.contains("too long"));
    let err = run("fn f() { return f() }\nlet x = f()").err().unwrap();
    assert!(err.message.contains("nested too deeply"));
    assert!(run("let a = range(100000000)").is_err());
    let mut src = String::from("let s = \"xxxxxxxxxx\"\n");
    for _ in 0..20 {
        src.push_str("s = s + s\n");
    }
    assert!(run(&src).is_err());
}

#[test]
fn hostile_source_never_panics() {
    let good = "fn f(a, b) {\n let x = [1, {\"k\": 2}]\n if a < b { return x[1][\"k\"] }\n \
                while false { }\n return -a\n}\nlet r = f(1, 2) # comment\n";
    for n in 0..good.len() {
        if good.is_char_boundary(n) {
            let _ = Script::parse(&good[..n]);
        }
    }
    // Deep nesting is refused, not recursed into.
    let deep = format!("let a = {}1{}", "(".repeat(5000), ")".repeat(5000));
    assert!(Script::parse(&deep).is_err());
    let deep = format!("let a = {}1", "-".repeat(5000));
    assert!(Script::parse(&deep).is_err());
    assert!(Script::parse("let a = \"unterminated").is_err());
    assert!(Script::parse("let a = 99999999999999999999").is_err());
    assert!(Script::parse("\u{0}").is_err());
}

#[test]
fn globals_persist_between_calls() {
    let script = Script::parse("let hits = 0\nfn hit() { hits += 1\n return hits }").unwrap();
    let mut vm = Vm::new(script);
    vm.run(&mut NoHost).unwrap();
    vm.call("hit", &[], &mut NoHost).unwrap();
    assert_eq!(vm.call("hit", &[], &mut NoHost), Ok(Variant::Int(2)));
    vm.set_global("hits", Variant::Int(100));
    assert_eq!(vm.call("hit", &[], &mut NoHost), Ok(Variant::Int(101)));
}

#[test]
fn deep_calls_inside_deep_expressions_fit_in_a_small_stack() {
    // The worst case for Rust stack use: every call level nests expressions to the parse limit.
    let nest = 30;
    let src = format!(
        "fn f(n) {{ return {}f(n + 1){} }}\nlet x = f(0)",
        "(-".repeat(nest),
        ")".repeat(nest)
    );
    let handle = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || run(&src).err().map(|e| e.message))
        .unwrap();
    let message = handle.join().unwrap().unwrap();
    assert!(message.contains("nested too deeply"), "{message}");
}
