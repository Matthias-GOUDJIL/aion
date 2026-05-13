use assert_cmd::Command;
use insta::assert_snapshot;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_aion_test(path: &str) -> String {
    let root = project_root();

    let output = Command::new("cargo")
        .args(["run", "--quiet", "--", "run", &format!("tests/fixtures/{}.ai", path)])
        .current_dir(&root)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute cargo run");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && !stderr.is_empty() {
        return stderr.trim().to_string();
    }

    let full = format!("{}\n{}", stdout, stderr);
    let lines: Vec<&str> = full.lines().collect();
    let mut between = Vec::new();
    let mut in_block = false;

    for line in &lines {
        if line.starts_with("-------------------------------") {
            if in_block {
                break;
            }
            in_block = true;
            continue;
        }
        if in_block {
            between.push(*line);
        }
    }

    between.join("\n").trim().to_string()
}

// --- Language Tests ---

#[test]
fn test_hello() { assert_snapshot!(run_aion_test("language/hello")); }
#[test]
fn test_duration_literal() { assert_snapshot!(run_aion_test("language/duration_literal")); }
#[test]
fn test_pipeline_operator() { assert_snapshot!(run_aion_test("language/pipeline_operator")); }
#[test]
fn test_date_arithmetic() { assert_snapshot!(run_aion_test("language/date_arithmetic")); }
#[test]
fn test_unsafe_check_fail() { assert_snapshot!(run_aion_test("language/unsafe_check_fail")); }
#[test]
fn test_fstring_interpolation() { assert_snapshot!(run_aion_test("language/fstring_interpolation")); }
#[test]
fn test_operators() { assert_snapshot!(run_aion_test("language/operators")); }
#[test]
fn test_unsafe_block() { assert_snapshot!(run_aion_test("language/unsafe_block")); }
#[test]
fn test_complex_pipeline() { assert_snapshot!(run_aion_test("language/complex_pipeline")); }
#[test]
fn test_string_operations() { assert_snapshot!(run_aion_test("language/string_operations")); }
#[test]
fn test_enum_match() { assert_snapshot!(run_aion_test("language/enum_match")); }
#[test]
fn test_generics_basic() { assert_snapshot!(run_aion_test("language/generics_basic")); }
#[test]
fn test_generics_result() { assert_snapshot!(run_aion_test("language/generics_result")); }
#[test]
fn test_struct_name_resolution() { assert_snapshot!(run_aion_test("language/struct_name_resolution")); }
#[test]
fn test_simple_expression() { assert_snapshot!(run_aion_test("language/simple_expression")); }
#[test]
fn test_generics_local() { assert_snapshot!(run_aion_test("language/generics_local")); }
#[test]
fn test_method_chaining() { assert_snapshot!(run_aion_test("language/method_chaining")); }
#[test]
fn test_short_circuit() { assert_snapshot!(run_aion_test("language/short_circuit")); }
#[test]
fn test_result_basic() { assert_snapshot!(run_aion_test("language/result_basic")); }
#[test]
fn test_result_methods() { assert_snapshot!(run_aion_test("language/result_methods")); }
#[test]
fn test_primitive_methods() { assert_snapshot!(run_aion_test("language/primitive_methods")); }
#[test]
fn test_struct_return() { assert_snapshot!(run_aion_test("language/struct_return")); }
#[test]
fn test_parse_result() { assert_snapshot!(run_aion_test("language/parse_result")); }
#[test]
fn test_char_literal() { assert_snapshot!(run_aion_test("language/char_literal")); }
#[test]
fn test_string_match() { assert_snapshot!(run_aion_test("language/string_match")); }
#[test]
fn test_recursion_deep() { assert_snapshot!(run_aion_test("language/recursion_deep")); }
#[test]
fn test_string_escapes() { assert_snapshot!(run_aion_test("language/string_escapes")); }
#[test]
fn test_break_continue() { assert_snapshot!(run_aion_test("language/break_continue")); }
#[test]
fn test_string_methods() { assert_snapshot!(run_aion_test("stdlib/string_methods")); }
#[test]
fn test_loop_basic() { assert_snapshot!(run_aion_test("language/loop_basic")); }
#[test]
fn test_function_types() { assert_snapshot!(run_aion_test("language/function_types")); }
#[test]
fn test_match_expression() { assert_snapshot!(run_aion_test("language/match_expression")); }
#[test]
fn test_option_result_methods() { assert_snapshot!(run_aion_test("language/option_result_methods")); }
#[test]
fn test_sizeof_variables() { assert_snapshot!(run_aion_test("language/sizeof_variables")); }

// --- Stdlib Tests ---

#[test]
fn test_sql_transpile() { assert_snapshot!(run_aion_test("stdlib/sql_transpile")); }
#[test]
fn test_fs_write_read() { assert_snapshot!(run_aion_test("stdlib/fs_write_read")); }
#[test]
fn test_env_args() { assert_snapshot!(run_aion_test("stdlib/env_args")); }
#[test]
fn test_fs_result_error() { assert_snapshot!(run_aion_test("stdlib/fs_result_error")); }
#[test]
fn test_std_fs_read_write() { assert_snapshot!(run_aion_test("stdlib/std_fs_read_write")); }
#[test]
fn test_env_var() { assert_snapshot!(run_aion_test("stdlib/env_var")); }
#[test]
fn test_env_args_cli() { assert_snapshot!(run_aion_test("stdlib/env_args_cli")); }
#[test]
fn test_vector_basic() { assert_snapshot!(run_aion_test("stdlib/vector_basic")); }
#[test]
fn test_env_vector_args() { assert_snapshot!(run_aion_test("stdlib/env_vector_args")); }
#[test]
fn test_vector_generic() { assert_snapshot!(run_aion_test("stdlib/vector_generic")); }
#[test]
fn test_vector_push_pop() { assert_snapshot!(run_aion_test("stdlib/vector_push_pop")); }
#[test]
fn test_hashmap_basic() { assert_snapshot!(run_aion_test("stdlib/hashmap_basic")); }
#[test]
fn test_hashmap_resize_hashset() { assert_snapshot!(run_aion_test("stdlib/hashmap_resize_hashset")); }
#[test]
fn test_tensor_basic() { assert_snapshot!(run_aion_test("stdlib/tensor_basic")); }
#[test]
fn test_fmt_format() { assert_snapshot!(run_aion_test("stdlib/fmt_format")); }
#[test]
fn test_path_operations() { assert_snapshot!(run_aion_test("stdlib/path_operations")); }
#[test]
fn test_dataframe_basic() { assert_snapshot!(run_aion_test("stdlib/dataframe_basic")); }
#[test]
fn test_sql_advanced() { assert_snapshot!(run_aion_test("stdlib/sql_advanced")); }
#[test]
fn test_json_parse_basic() { assert_snapshot!(run_aion_test("stdlib/json_parse_basic")); }

// --- Compiler Tests ---

#[test]
fn test_debug_output() { assert_snapshot!(run_aion_test("compiler/debug_output")); }
#[test]
fn test_malloc_test() { assert_snapshot!(run_aion_test("compiler/malloc_test")); }
#[test]
fn test_gc_leak_test() { assert_snapshot!(run_aion_test("compiler/gc_leak_test")); }
#[test]
fn test_extern_ffi() { assert_snapshot!(run_aion_test("compiler/extern_ffi")); }
#[test]
fn test_self_lexer() { assert_snapshot!(run_aion_test("compiler/self_lexer")); }
#[test]
fn test_optimization_check() { assert_snapshot!(run_aion_test("compiler/optimization_check")); }
#[test]
fn test_self_lexer_loop() { assert_snapshot!(run_aion_test("compiler/self_lexer_loop")); }
#[test]
fn test_self_parser() { assert_snapshot!(run_aion_test("compiler/self_parser")); }

// --- Example Tests ---

#[test]
fn test_example_hello() { assert_snapshot!(run_aion_test("examples/hello")); }
#[test]
fn test_example_types() { assert_snapshot!(run_aion_test("examples/types")); }
#[test]
fn test_example_logic() { assert_snapshot!(run_aion_test("examples/logic")); }
#[test]
fn test_example_generics() { assert_snapshot!(run_aion_test("examples/generics")); }
#[test]
fn test_example_interface_impl() { assert_snapshot!(run_aion_test("examples/interface_impl")); }

// --- Doc Generation Tests ---

fn run_aion_doc(path: &str) -> String {
    let root = project_root();
    let fixture = format!("tests/fixtures/{}.ai", path);
    let output_file = format!("tests/fixtures/{}.ai.doc", path);

    let output = Command::new("cargo")
        .args(["run", "--quiet", "--", "doc", &fixture, "-o", &output_file])
        .current_dir(&root)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute cargo run doc");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return stderr.trim().to_string();
    }

    let doc = std::fs::read_to_string(&output_file)
        .unwrap_or_else(|e| format!("Failed to read doc output: {}", e));
    let _ = std::fs::remove_file(&output_file);
    doc
}

#[test]
fn test_doc_gen() { assert_snapshot!(run_aion_doc("compiler/doc_gen")); }
