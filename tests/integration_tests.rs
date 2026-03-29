/// Integration test suite for the cipr compiler.
///
/// Architecture
/// ────────────
/// Each test calls `run_cipr(source)`, which writes a temporary `.cipr` file,
/// invokes the compiled `cipr` binary as a subprocess (with CWD set to the
/// project root so relative includes work), captures stdout + stderr, cleans
/// up the temp file, and returns a `CiprOutput`.
///
/// Two assertion helpers keep individual tests concise:
///   • `assert_output(src, expected_stdout)` — compile + run must succeed and
///     stdout must match exactly.
///   • `assert_compile_error(src)` — the binary must exit non-zero (compile
///     or type error before any code runs).
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

// ── Test infrastructure ──────────────────────────────────────────────────────

/// Path to the cipr binary, resolved at compile time by Cargo.
const BINARY: &str = env!("CARGO_BIN_EXE_cipr");

/// Project root; used as CWD so relative `include` paths resolve correctly.
const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct CiprOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

/// Write `source` to a unique temp file, invoke cipr, capture output, clean up.
///
/// Files are written into `PROJECT_ROOT` using a relative filename so that
/// `core::link_and_emit`'s `"./{}"` binary-path construction resolves
/// correctly when the cipr process runs with `current_dir = PROJECT_ROOT`.
fn run_cipr(source: &str) -> CiprOutput {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Relative filename; cipr + the OS both resolve it against PROJECT_ROOT.
    let filename = format!("cipr_test_{}_{}.cipr", std::process::id(), n);
    let full_path = format!("{}/{}", PROJECT_ROOT, &filename);

    std::fs::write(&full_path, source).expect("failed to write temp .cipr file");

    let output = Command::new(BINARY)
        .arg(&filename) // relative → out_bin is a bare name, not an absolute path
        .current_dir(PROJECT_ROOT)
        .output()
        .unwrap_or_else(|e| panic!("failed to invoke cipr binary: {}", e));

    // The compiled binary + .ll + .o are cleaned up by core::link_and_emit.
    // We only need to remove the source file.
    let _ = std::fs::remove_file(&full_path);

    CiprOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    }
}

/// Run a fixture file by name (from `tests/fixtures/`).
fn run_fixture(name: &str) -> CiprOutput {
    let path = format!("{}/tests/fixtures/{}", PROJECT_ROOT, name);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("fixture not found: {}", path));
    run_cipr(&source)
}

/// Assert the program compiles, runs successfully, and produces exactly
/// `expected` on stdout.
#[track_caller]
fn assert_output(source: &str, expected: &str) {
    let out = run_cipr(source);
    assert!(
        out.success,
        "program failed to compile/run.\nstderr:\n{}\nstdout:\n{}",
        out.stderr, out.stdout
    );
    assert_eq!(
        out.stdout, expected,
        "stdout mismatch.\nstderr:\n{}",
        out.stderr
    );
}

/// Assert that cipr exits with a non-zero status code (compile or type error).
#[track_caller]
fn assert_compile_error(source: &str) {
    let out = run_cipr(source);
    assert!(
        !out.success,
        "expected a compile error but the program succeeded.\nstdout:\n{}",
        out.stdout
    );
}

// ── variables ────────────────────────────────────────────────────────────────

mod variables {
    use super::*;

    #[test]
    fn let_int_with_annotation() {
        assert_output("let x: int = 42; print(x);", "42\n");
    }

    #[test]
    fn let_float_with_annotation() {
        assert_output("let y: float = 3.14; print(y);", "3.140000\n");
    }

    #[test]
    fn let_str_with_annotation() {
        assert_output(r#"let s: str = "hello"; print(s);"#, "hello\n");
    }

    #[test]
    fn let_bool_true() {
        assert_output("let b: bool = true; print(b);", "true\n");
    }

    #[test]
    fn let_bool_false() {
        assert_output("let b: bool = false; print(b);", "false\n");
    }

    #[test]
    fn let_int_type_inference() {
        assert_output("let x = 100; print(x);", "100\n");
    }

    #[test]
    fn let_float_type_inference() {
        assert_output("let y = 2.5; print(y);", "2.500000\n");
    }

    #[test]
    fn let_str_type_inference() {
        assert_output(r#"let s = "world"; print(s);"#, "world\n");
    }

    #[test]
    fn let_bool_type_inference() {
        assert_output("let b = true; print(b);", "true\n");
    }

    #[test]
    fn let_reassignment() {
        assert_output("let x: int = 1; x = 99; print(x);", "99\n");
    }

    #[test]
    fn multiple_let_bindings() {
        assert_output(
            "let a: int = 1; let b: int = 2; let c: int = 3; print(a); print(b); print(c);",
            "1\n2\n3\n",
        );
    }
}

// ── arithmetic ───────────────────────────────────────────────────────────────

mod arithmetic {
    use super::*;

    #[test]
    fn int_addition() {
        assert_output("print(2 + 3);", "5\n");
    }

    #[test]
    fn int_subtraction() {
        assert_output("print(10 - 4);", "6\n");
    }

    #[test]
    fn int_multiplication() {
        assert_output("print(3 * 4);", "12\n");
    }

    #[test]
    fn int_division_truncates_toward_zero() {
        assert_output("print(10 / 3);", "3\n");
    }

    #[test]
    fn operator_precedence_mul_before_add() {
        assert_output("print(2 + 3 * 4);", "14\n");
    }

    #[test]
    fn operator_precedence_grouping() {
        assert_output("print((2 + 3) * 4);", "20\n");
    }

    #[test]
    fn unary_minus_literal() {
        assert_output("print(-7);", "-7\n");
    }

    #[test]
    fn unary_minus_variable() {
        assert_output("let x: int = 5; print(-x);", "-5\n");
    }

    #[test]
    fn float_addition() {
        assert_output("print(1.5 + 2.5);", "4.000000\n");
    }

    #[test]
    fn float_multiplication() {
        assert_output("print(2.0 * 3.0);", "6.000000\n");
    }

    #[test]
    fn float_subtraction() {
        assert_output("print(5.0 - 1.5);", "3.500000\n");
    }

    #[test]
    fn int_comparison_less_than_true() {
        assert_output("print(3 < 5);", "true\n");
    }

    #[test]
    fn int_comparison_less_than_false() {
        assert_output("print(5 < 3);", "false\n");
    }

    #[test]
    fn int_comparison_greater_than() {
        assert_output("print(5 > 3);", "true\n");
    }

    #[test]
    fn int_comparison_less_equal_exact() {
        assert_output("print(3 <= 3);", "true\n");
    }

    #[test]
    fn int_comparison_greater_equal_false() {
        assert_output("print(4 >= 5);", "false\n");
    }

    #[test]
    fn int_equality() {
        assert_output("print(7 == 7);", "true\n");
    }

    #[test]
    fn int_inequality() {
        assert_output("print(2 != 3);", "true\n");
    }
}

// ── booleans ─────────────────────────────────────────────────────────────────

mod booleans {
    use super::*;

    #[test]
    fn bool_literal_true() {
        assert_output("print(true);", "true\n");
    }

    #[test]
    fn bool_literal_false() {
        assert_output("print(false);", "false\n");
    }

    #[test]
    fn and_true_true() {
        assert_output("print(true and true);", "true\n");
    }

    #[test]
    fn and_true_false() {
        assert_output("print(true and false);", "false\n");
    }

    #[test]
    fn and_false_true() {
        assert_output("print(false and true);", "false\n");
    }

    #[test]
    fn or_false_false() {
        assert_output("print(false or false);", "false\n");
    }

    #[test]
    fn or_false_true() {
        assert_output("print(false or true);", "true\n");
    }

    #[test]
    fn or_true_false() {
        assert_output("print(true or false);", "true\n");
    }

    #[test]
    fn not_true() {
        assert_output("print(!true);", "false\n");
    }

    #[test]
    fn not_false() {
        assert_output("print(!false);", "true\n");
    }

    #[test]
    fn short_circuit_and_skips_rhs() {
        // If `and` short-circuits, the RHS 0 == 1 is never evaluated but the
        // overall result is just false — this verifies the value is correct.
        assert_output("print(false and 0 == 1);", "false\n");
    }

    #[test]
    fn short_circuit_or_skips_rhs() {
        assert_output("print(true or 0 == 1);", "true\n");
    }
}

// ── strings ───────────────────────────────────────────────────────────────────

mod strings {
    use super::*;

    #[test]
    fn print_string_literal() {
        assert_output(r#"print("hello world");"#, "hello world\n");
    }

    #[test]
    fn print_empty_string() {
        assert_output(r#"print("");"#, "\n");
    }

    #[test]
    fn string_concat_with_plus() {
        assert_output(r#"print("foo" + "bar");"#, "foobar\n");
    }

    #[test]
    fn string_concat_via_variables() {
        assert_output(
            r#"let a: str = "hello"; let b: str = " world"; print(a + b);"#,
            "hello world\n",
        );
    }

    #[test]
    fn string_in_function_argument() {
        assert_output(
            r#"
fn greet(name: str): void {
    print("Hello, " + name);
}
greet("Cipr");
"#,
            "Hello, Cipr\n",
        );
    }
}

// ── control_flow ─────────────────────────────────────────────────────────────

mod control_flow {
    use super::*;

    #[test]
    fn if_true_takes_then_branch() {
        assert_output("if (true) { print(1); } else { print(0); }", "1\n");
    }

    #[test]
    fn if_false_takes_else_branch() {
        assert_output("if (false) { print(1); } else { print(0); }", "0\n");
    }

    #[test]
    fn if_condition_from_comparison() {
        assert_output(
            "let x: int = 5; if (x > 3) { print(1); } else { print(0); }",
            "1\n",
        );
    }

    #[test]
    fn nested_if_else() {
        assert_output(
            r#"
let x: int = 2;
if (x == 1) {
    print(100);
} else {
    if (x == 2) {
        print(200);
    } else {
        print(300);
    }
}
"#,
            "200\n",
        );
    }

    #[test]
    fn while_loop_three_iterations() {
        assert_output(
            r#"
let i: int = 0;
while (i < 3) {
    print(i);
    i = i + 1;
}
"#,
            "0\n1\n2\n",
        );
    }

    #[test]
    fn while_loop_zero_iterations() {
        // Body never executes; stdout must be empty.
        assert_output("while (false) { print(99); }", "");
    }

    #[test]
    fn for_loop_basic() {
        assert_output(
            "for (let i: int = 0; i < 4; i = i + 1) { print(i); }",
            "0\n1\n2\n3\n",
        );
    }

    #[test]
    fn for_loop_accumulator() {
        assert_output(
            r#"
let sum: int = 0;
for (let i: int = 1; i <= 5; i = i + 1) {
    sum = sum + i;
}
print(sum);
"#,
            "15\n",
        );
    }

    #[test]
    fn early_return_from_if() {
        assert_output(
            r#"
fn first_positive(a: int, b: int): int {
    if (a > 0) {
        return a;
    }
    return b;
}
print(first_positive(-1, 7));
print(first_positive(3, 9));
"#,
            "7\n3\n",
        );
    }

    #[test]
    fn if_without_else() {
        assert_output(
            r#"
let x: int = 10;
if (x > 5) {
    print(x);
}
print(0);
"#,
            "10\n0\n",
        );
    }
}

// ── functions ─────────────────────────────────────────────────────────────────

mod functions {
    use super::*;

    #[test]
    fn basic_function_with_return() {
        assert_output(
            r#"
fn square(n: int): int {
    return n * n;
}
print(square(5));
"#,
            "25\n",
        );
    }

    #[test]
    fn multiple_parameters() {
        assert_output(
            r#"
fn add(a: int, b: int): int {
    return a + b;
}
print(add(10, 32));
"#,
            "42\n",
        );
    }

    #[test]
    fn multiple_functions_called_in_sequence() {
        assert_output(
            r#"
fn double(x: int): int { return x * 2; }
fn triple(x: int): int { return x * 3; }
print(double(5));
print(triple(5));
"#,
            "10\n15\n",
        );
    }

    #[test]
    fn void_function_side_effect() {
        assert_output(
            r#"
fn say(msg: str): void {
    print(msg);
}
say("hi");
say("bye");
"#,
            "hi\nbye\n",
        );
    }

    #[test]
    fn function_calls_other_function() {
        assert_output(
            r#"
fn add(a: int, b: int): int { return a + b; }
fn sum3(a: int, b: int, c: int): int { return add(add(a, b), c); }
print(sum3(1, 2, 3));
"#,
            "6\n",
        );
    }

    #[test]
    fn recursion_fibonacci_fixture() {
        let out = run_fixture("recursion.cipr");
        assert!(
            out.success,
            "recursion fixture failed.\nstderr:\n{}",
            out.stderr
        );
        assert_eq!(out.stdout, "0\n1\n5\n55\n");
    }

    #[test]
    fn callback_variable_call() {
        assert_output(
            r#"
fn double(n: int): int {
    return n * 2;
}

let cb: fn(int): int = double;
print(cb(21));
"#,
            "42\n",
        );
    }

    #[test]
    fn callback_parameter_call() {
        assert_output(
            r#"
fn square(n: int): int {
    return n * n;
}

fn apply(f: fn(int): int, x: int): int {
    return f(x);
}

print(apply(square, 9));
"#,
            "81\n",
        );
    }

    #[test]
    fn callback_nested_type_annotation() {
        assert_output(
            r#"
fn inc(n: int): int { return n + 1; }
fn compose_apply(g: fn(int): int, f: fn(int): int, x: int): int {
    return g(f(x));
}
print(compose_apply(inc, inc, 5));
"#,
            "7\n",
        );
    }
}

// ── structs ───────────────────────────────────────────────────────────────────

mod structs {
    use super::*;

    #[test]
    fn stack_init_and_field_read() {
        assert_output(
            r#"
struct Vec2 {
    x: int,
    y: int
}
let v: Vec2 = Vec2 { x: 10, y: 20 };
print(v.x);
print(v.y);
"#,
            "10\n20\n",
        );
    }

    #[test]
    fn field_mutation() {
        assert_output(
            r#"
struct Counter {
    val: int
}
let c: Counter = Counter { val: 0 };
c.val = 42;
print(c.val);
"#,
            "42\n",
        );
    }

    #[test]
    fn struct_with_float_field() {
        assert_output(
            r#"
struct Pt {
    x: float,
    y: float
}
let p: Pt = Pt { x: 1.5, y: 2.5 };
print(p.x);
print(p.y);
"#,
            "1.500000\n2.500000\n",
        );
    }

    #[test]
    fn pointer_to_struct_mutation_through_fn() {
        assert_output(
            r#"
struct Point {
    x: int,
    y: int
}
fn translate(p: @Point, dx: int, dy: int): void {
    p.x = p.x + dx;
    p.y = p.y + dy;
}
let pt: Point = Point { x: 5, y: 10 };
let ptr: @Point = @pt;
translate(ptr, 3, 4);
print(pt.x);
print(pt.y);
"#,
            "8\n14\n",
        );
    }

    #[test]
    fn deref_then_field_assign() {
        assert_output(
            r#"
struct Box {
    val: int
}
let b: Box = Box { val: 1 };
let p: @Box = @b;
p@.val = 99;
print(b.val);
"#,
            "99\n",
        );
    }
}

// ── heap ──────────────────────────────────────────────────────────────────────

mod heap {
    use super::*;

    #[test]
    fn new_and_field_access() {
        assert_output(
            r#"
struct Node {
    val: int
}
let n: @Node = new Node(99);
print(n.val);
delete n;
"#,
            "99\n",
        );
    }

    #[test]
    fn multiple_heap_allocations() {
        assert_output(
            r#"
struct Pair {
    a: int,
    b: int
}
let p1: @Pair = new Pair(1, 2);
let p2: @Pair = new Pair(10, 20);
print(p1.a);
print(p2.b);
delete p1;
delete p2;
"#,
            "1\n20\n",
        );
    }

    #[test]
    fn heap_field_mutation() {
        assert_output(
            r#"
struct Cell {
    val: int
}
let c: @Cell = new Cell(0);
c.val = 77;
print(c.val);
delete c;
"#,
            "77\n",
        );
    }
}

// ── pointers ─────────────────────────────────────────────────────────────────

mod pointers {
    use super::*;

    #[test]
    fn address_of_and_assign_through_pointer() {
        assert_output(
            r#"
let x: int = 10;
let p: @int = @x;
print(x);
p@ = 25;
print(x);
"#,
            "10\n25\n",
        );
    }

    #[test]
    fn pointer_passed_to_function_mutates_original() {
        assert_output(
            r#"
fn set_val(p: @int, v: int): void {
    p@ = v;
}
let x: int = 0;
set_val(@x, 42);
print(x);
"#,
            "42\n",
        );
    }

    #[test]
    fn two_pointers_to_same_variable() {
        assert_output(
            r#"
let x: int = 1;
let p1: @int = @x;
let p2: @int = @x;
p1@ = 100;
print(x);
p2@ = 200;
print(x);
"#,
            "100\n200\n",
        );
    }
}

// ── arrays ────────────────────────────────────────────────────────────────────

mod arrays {
    use super::*;

    #[test]
    fn array_literal_and_index_read() {
        assert_output(
            r#"
let arr = [10, 20, 30];
print(arr[0]);
print(arr[1]);
print(arr[2]);
"#,
            "10\n20\n30\n",
        );
    }

    #[test]
    fn array_of_floats() {
        assert_output(
            r#"
let arr = [1.0, 2.0, 3.0];
print(arr[0]);
"#,
            "1.000000\n",
        );
    }
}

// ── stdlib_string ─────────────────────────────────────────────────────────────

mod stdlib_string {
    use super::*;

    const INCLUDE: &str = r#"include "src/lib/std/string.cipr";"#;

    fn with_string_stdlib(body: &str) -> String {
        format!("{}\n{}", INCLUDE, body)
    }

    #[test]
    fn str_len() {
        assert_output(&with_string_stdlib(r#"print(str_len("hello"));"#), "5\n");
    }

    #[test]
    fn str_concat() {
        assert_output(
            &with_string_stdlib(r#"print(str_concat("foo", "bar"));"#),
            "foobar\n",
        );
    }

    #[test]
    fn str_eq_equal_strings() {
        assert_output(
            &with_string_stdlib(r#"let eq: bool = str_eq("abc", "abc"); print(eq);"#),
            "true\n",
        );
    }

    #[test]
    fn str_eq_unequal_strings() {
        assert_output(
            &with_string_stdlib(r#"let neq: bool = str_eq("abc", "xyz"); print(neq);"#),
            "false\n",
        );
    }

    #[test]
    fn str_slice() {
        assert_output(
            &with_string_stdlib(r#"print(str_slice("Hello World", 0, 5));"#),
            "Hello\n",
        );
    }

    #[test]
    fn str_to_int() {
        assert_output(&with_string_stdlib(r#"print(str_to_int("42"));"#), "42\n");
    }

    #[test]
    fn str_to_float() {
        assert_output(
            &with_string_stdlib(r#"print(str_to_float("2.5"));"#),
            "2.500000\n",
        );
    }

    #[test]
    fn int_to_str() {
        assert_output(&with_string_stdlib(r#"print(int_to_str(100));"#), "100\n");
    }

    #[test]
    fn float_to_str() {
        // cipr_float_to_str uses "%g" which strips trailing zeros.
        assert_output(
            &with_string_stdlib(r#"print(float_to_str(2.718));"#),
            "2.718\n",
        );
    }

    #[test]
    fn str_contains_true() {
        assert_output(
            &with_string_stdlib(r#"let b: bool = str_contains("Hello World", "World"); print(b);"#),
            "true\n",
        );
    }

    #[test]
    fn str_contains_false() {
        assert_output(
            &with_string_stdlib(r#"let b: bool = str_contains("Hello World", "xyz"); print(b);"#),
            "false\n",
        );
    }

    #[test]
    fn str_starts_with_true() {
        assert_output(
            &with_string_stdlib(r#"let b: bool = str_starts_with("Hello", "He"); print(b);"#),
            "true\n",
        );
    }

    #[test]
    fn str_starts_with_false() {
        assert_output(
            &with_string_stdlib(r#"let b: bool = str_starts_with("Hello", "lo"); print(b);"#),
            "false\n",
        );
    }

    #[test]
    fn str_free_does_not_crash() {
        let out = run_cipr(&with_string_stdlib(
            r#"let s: str = str_concat("hello", " world"); str_free(s);"#,
        ));
        assert!(out.success, "str_free crashed.\nstderr:\n{}", out.stderr);
    }

    #[test]
    fn multi_include_fixture() {
        let out = run_fixture("multi_include.cipr");
        assert!(
            out.success,
            "multi_include fixture failed.\nstderr:\n{}",
            out.stderr
        );
        assert_eq!(out.stdout, "Hello World\n11\n");
    }
}

// ── stdlib_file ───────────────────────────────────────────────────────────────

mod stdlib_file {
    use super::*;

    #[test]
    fn write_exists_read_append() {
        // Use a per-test path (pid-unique) so parallel runs don't collide.
        let tmp = format!("/tmp/cipr_file_test_{}.txt", std::process::id());
        let _ = std::fs::remove_file(&tmp); // clean slate

        // No trailing \n in the written content so cipr_print_str emits exactly
        // one newline, keeping the output lines predictable.
        let source = format!(
            r#"
include "src/lib/std/file.cipr";
let path: str = "{}";
let w: int = file_write(path, "hello cipr");
print(w);
let e: bool = file_exists(path);
print(e);
let content: str = file_read(path);
print(content);
let a: int = file_append(path, "second line");
print(a);
"#,
            tmp
        );

        let out = run_cipr(&source);
        let _ = std::fs::remove_file(&tmp);

        assert!(out.success, "file test failed.\nstderr:\n{}", out.stderr);

        let lines: Vec<&str> = out.stdout.lines().collect();
        assert_eq!(lines[0], "0", "file_write should return 0 on success");
        assert_eq!(lines[1], "true", "file should exist after write");
        assert!(
            lines[2].contains("hello cipr"),
            "file_read content mismatch"
        );
        assert_eq!(lines[3], "0", "file_append should return 0 on success");
    }

    #[test]
    fn file_exists_returns_false_for_missing_file() {
        assert_output(
            r#"
include "src/lib/std/file.cipr";
let e: bool = file_exists("/tmp/this_file_definitely_does_not_exist_cipr.txt");
print(e);
"#,
            "false\n",
        );
    }
}

// ── stdlib_time ───────────────────────────────────────────────────────────────

mod stdlib_time {
    use super::*;

    #[test]
    fn time_returns_positive_float() {
        let out = run_cipr(
            r#"
include "src/lib/std/time.cipr";
let t: float = time();
print(t);
"#,
        );
        assert!(out.success, "time() failed.\nstderr:\n{}", out.stderr);

        let value: f64 =
            out.stdout.trim().parse().unwrap_or_else(|_| {
                panic!("could not parse time() output: '{}'", out.stdout.trim())
            });

        assert!(
            value > 0.0,
            "time() returned a non-positive value: {}",
            value
        );
    }
}

// ── error_detection ───────────────────────────────────────────────────────────

mod error_detection {
    use super::*;

    #[test]
    fn undefined_variable_is_compile_error() {
        assert_compile_error("print(undefined_variable);");
    }

    #[test]
    fn type_mismatch_in_let_is_compile_error() {
        assert_compile_error(r#"let x: int = "not an int"; print(x);"#);
    }

    #[test]
    fn undefined_function_call_is_compile_error() {
        assert_compile_error("let x: int = totally_made_up(1, 2); print(x);");
    }

    #[test]
    fn wrong_number_of_args_is_compile_error() {
        assert_compile_error(
            r#"
fn add(a: int, b: int): int { return a + b; }
print(add(1));
"#,
        );
    }
}

// ── Server Test Infrastructure ────────────────────────────────────────────────

/// Run a cipr script in the background (which should start a server), wait for it
/// to boot, send a TCP request, capture the response, and wait for the process to exit.
/// The script MUST eventually call `http_stop()` or `net_close()` and exit for this to return.
fn run_cipr_server(source: &str, port: u16, request_bytes: &[u8]) -> (CiprOutput, String) {
    use std::io::{Read, Write};

    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let filename = format!("cipr_test_srv_{}_{}.cipr", std::process::id(), n);
    let full_path = format!("{}/{}", PROJECT_ROOT, &filename);

    std::fs::write(&full_path, source).expect("failed to write temp .cipr file");

    let mut child = Command::new(BINARY)
        .arg(&filename)
        .current_dir(PROJECT_ROOT)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to invoke cipr binary: {}", e));

    // Stream output in background so we don't block
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(n) = std::io::Read::read(&mut stdout, &mut buf) {
            if n == 0 { break; }
            print!("{}", String::from_utf8_lossy(&buf[..n]));
        }
    });
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(n) = std::io::Read::read(&mut stderr, &mut buf) {
            if n == 0 { break; }
            eprint!("{}", String::from_utf8_lossy(&buf[..n]));
        }
    });

    // Retry loop: The Cipr compiler takes time to compile the C runtime before the server starts.
    let mut stream_opt = None;
    for _ in 0..50 {
        if let Ok(mut stream) = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)) {
            stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
            let _ = stream.write_all(request_bytes);
            stream_opt = Some(stream);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let mut response = String::new();
    if let Some(mut stream) = stream_opt {
        let _ = stream.read_to_string(&mut response);
    } else {
        // If we couldn't connect, kill the child so we don't hang the test suite
        let _ = child.kill();
        panic!("Failed to connect to Cipr server on port {} after 10 seconds", port);
    }

    let status = child.wait().unwrap();
    let _ = std::fs::remove_file(&full_path);

    (
        CiprOutput {
            stdout: String::new(), // Output already streamed to real stdout
            stderr: String::new(),
            success: status.success(),
        },
        response,
    )
}

// ── stdlib_http ───────────────────────────────────────────────────────────────

mod stdlib_http {
    use super::*;

    #[test]
    fn basic_http_get() {
        let source = r#"
include "src/lib/std/http.cipr";

fn handler(): void {
    http_ok("Hello from Cipr HTTP!");
    http_stop(); // Stop the server so the test completes
}

http_get("/test", handler);
http_start(8081);
"#;
        let request = b"GET /test HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        let (out, response) = run_cipr_server(source, 8081, request);
        
        assert!(out.success, "HTTP server failed.\nstderr:\n{}", out.stderr);
        assert!(response.contains("HTTP/1.1 200 OK"), "Missing 200 OK in response: {}", response);
        assert!(response.contains("Hello from Cipr HTTP!"), "Missing body in response: {}", response);
    }

    #[test]
    fn http_post_body() {
        let source = r#"
include "src/lib/std/http.cipr";
fn handler(): void {
    let body: str = http_body();
    http_ok("Received: " + body);
    http_stop();
}
http_post("/submit", handler);
http_start(8085);
"#;
        let request = b"POST /submit HTTP/1.1\r\nHost: localhost\r\nContent-Length: 9\r\nConnection: close\r\n\r\nMY_DATA_1";
        let (_, response) = run_cipr_server(source, 8085, request);
        assert!(response.contains("Received: MY_DATA_1"), "POST body failed: {}", response);
    }

    #[test]
    fn http_json_response() {
        let source = r#"
include "src/lib/std/http.cipr";
fn handler(): void {
    http_json(201, "{\"status\":\"ok\"}");
    http_stop();
}
http_get("/json", handler);
http_start(8086);
"#;
        let request = b"GET /json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        let (_, response) = run_cipr_server(source, 8086, request);
        assert!(response.contains("HTTP/1.1 201 Created") || response.contains("201"), "Status code failed");
        assert!(response.contains("Content-Type: application/json"), "Content-Type header failed");
        assert!(response.contains("{\"status\":\"ok\"}"), "JSON body failed");
    }
}

// ── stdlib_net ────────────────────────────────────────────────────────────────

mod stdlib_net {
    use super::*;

    #[test]
    fn basic_tcp_echo() {
        let source = r#"
include "src/lib/std/net.cipr";

let server_fd: int = net_listen(8082, false);
if (server_fd >= 0) {
    let client_fd: int = net_accept(server_fd, false);
    if (client_fd >= 0) {
        let data: str = net_read(client_fd, 1024);
        net_write(client_fd, "ECHO: ");
        net_write(client_fd, data);
        net_close(client_fd);
    }
    net_close(server_fd);
}
"#;
        let request = b"Hello TCP";
        let (out, response) = run_cipr_server(source, 8082, request);
        
        assert!(out.success, "TCP server failed.\nstderr:\n{}", out.stderr);
        assert_eq!(response, "ECHO: Hello TCP");
    }

    #[test]
    fn basic_tcp_client() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::Write;
                let _ = stream.write_all(b"HELLO FROM RUST");
            }
        });

        let source = format!(r#"
include "src/lib/std/net.cipr";

let client_fd: int = net_connect("127.0.0.1", {}, false);
if (client_fd >= 0) {{
    let data: str = net_read(client_fd, 1024);
    print(data);
    net_close(client_fd);
}} else {{
    print("Failed to connect");
}}
"#, port);

        assert_output(&source, "HELLO FROM RUST\n");
    }
}
