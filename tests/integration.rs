use assert_cmd::Command;
use insta::assert_snapshot;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_aion_test(path: &str) -> String {
    let root = project_root();

    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--",
            "run",
            &format!("tests/fixtures/{}.ai", path),
        ])
        .current_dir(&root)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute cargo run");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Always extract program output from between the dashes first, even when
    // `aion run` propagated a non-zero child exit code (#106). Compile
    // failures (no dashes, empty stdout) fall through to the stderr fallback
    // below; runtime crashes (OOB trap) hit the same fallback.
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

    let extracted = between.join("\n").trim().to_string();

    // Runtime crashes (e.g. array OOB trap) print to stderr and produce no
    // stdout between the dashes — fall back to stderr so the error is
    // captured. #54.
    if extracted.is_empty() && !stderr.trim().is_empty() {
        return stderr.trim().to_string();
    }

    extracted
}

// --- Language Tests ---

#[test]
fn test_run_exit_code_propagation() {
    // #106 — `aion run` must propagate the child process exit code so
    // shell/CI callers detect runtime failures and explicit return codes.
    let root = project_root();
    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--",
            "run",
            "tests/fixtures/language/run_exit_code.ai",
        ])
        .current_dir(&root)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute cargo run");
    assert_eq!(
        output.status.code(),
        Some(42),
        "aion run should exit with the program's return code, not 0"
    );
}

#[test]
fn test_run_exit_code_runtime_crash() {
    // #106 — a runtime trap (OOB) must surface as a non-zero `aion run` exit.
    let root = project_root();
    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--",
            "run",
            "tests/fixtures/language/array_bounds.ai",
        ])
        .current_dir(&root)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute cargo run");
    assert_ne!(
        output.status.code(),
        Some(0),
        "aion run should exit non-zero when the program crashes at runtime"
    );
}

#[test]
fn test_run_exit_code_success() {
    // #106 — nominal: hello.ai returns 0 → `aion run` exits 0.
    let root = project_root();
    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--",
            "run",
            "tests/fixtures/language/hello.ai",
        ])
        .current_dir(&root)
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("failed to execute cargo run");
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn test_hello() {
    assert_snapshot!(run_aion_test("language/hello"));
}
#[test]
fn test_duration_literal() {
    assert_snapshot!(run_aion_test("language/duration_literal"));
}
#[test]
fn test_pipeline_operator() {
    assert_snapshot!(run_aion_test("language/pipeline_operator"));
}
#[test]
fn test_date_arithmetic() {
    assert_snapshot!(run_aion_test("language/date_arithmetic"));
}
#[test]
fn test_unsafe_check_fail() {
    assert_snapshot!(run_aion_test("language/unsafe_check_fail"));
}
#[test]
fn test_fstring_interpolation() {
    assert_snapshot!(run_aion_test("language/fstring_interpolation"));
}
#[test]
fn test_fstring_edge() {
    assert_snapshot!(run_aion_test("language/fstring_edge"));
}
#[test]
fn test_fstring_exprs() {
    assert_snapshot!(run_aion_test("language/fstring_exprs"));
}
#[test]
fn test_fstring_nested_braces() {
    assert_snapshot!(run_aion_test("language/fstring_nested_braces"));
}
#[test]
fn test_fstring_many_interpolations() {
    assert_snapshot!(run_aion_test("language/fstring_many_interpolations"));
}
#[test]
fn test_operators() {
    assert_snapshot!(run_aion_test("language/operators"));
}
#[test]
fn test_unsafe_block() {
    assert_snapshot!(run_aion_test("language/unsafe_block"));
}
#[test]
fn test_complex_pipeline() {
    assert_snapshot!(run_aion_test("language/complex_pipeline"));
}
#[test]
fn test_string_operations() {
    assert_snapshot!(run_aion_test("language/string_operations"));
}
#[test]
fn test_enum_match() {
    assert_snapshot!(run_aion_test("language/enum_match"));
}
#[test]
fn test_generics_basic() {
    assert_snapshot!(run_aion_test("language/generics_basic"));
}
#[test]
fn test_generics_result() {
    assert_snapshot!(run_aion_test("language/generics_result"));
}
#[test]
fn test_struct_name_resolution() {
    assert_snapshot!(run_aion_test("language/struct_name_resolution"));
}
#[test]
fn test_simple_expression() {
    assert_snapshot!(run_aion_test("language/simple_expression"));
}
#[test]
fn test_generics_local() {
    assert_snapshot!(run_aion_test("language/generics_local"));
}
#[test]
fn test_generics_substring() {
    assert_snapshot!(run_aion_test("language/generics_substring"));
}
#[test]
fn test_generics_multi_arg() {
    assert_snapshot!(run_aion_test("language/generics_multi_arg"));
}
#[test]
fn test_generics_multi_arg_err() {
    assert_snapshot!(run_aion_test("language/generics_multi_arg_err"));
}
#[test]
fn test_let_type_annotation() {
    assert_snapshot!(run_aion_test("language/let_type_annotation"));
}
#[test]
fn test_method_chaining() {
    assert_snapshot!(run_aion_test("language/method_chaining"));
}
#[test]
fn test_short_circuit() {
    assert_snapshot!(run_aion_test("language/short_circuit"));
}
#[test]
fn test_result_basic() {
    assert_snapshot!(run_aion_test("language/result_basic"));
}
#[test]
fn test_result_methods() {
    assert_snapshot!(run_aion_test("language/result_methods"));
}
#[test]
fn test_primitive_methods() {
    assert_snapshot!(run_aion_test("language/primitive_methods"));
}
#[test]
fn test_struct_return() {
    assert_snapshot!(run_aion_test("language/struct_return"));
}
#[test]
fn test_parse_result() {
    assert_snapshot!(run_aion_test("language/parse_result"));
}
#[test]
fn test_char_literal() {
    assert_snapshot!(run_aion_test("language/char_literal"));
}
#[test]
fn test_string_match() {
    assert_snapshot!(run_aion_test("language/string_match"));
}
#[test]
fn test_recursion_deep() {
    assert_snapshot!(run_aion_test("language/recursion_deep"));
}
#[test]
fn test_string_escapes() {
    assert_snapshot!(run_aion_test("language/string_escapes"));
}
#[test]
fn test_break_continue() {
    assert_snapshot!(run_aion_test("language/break_continue"));
}
#[test]
fn test_string_methods() {
    assert_snapshot!(run_aion_test("stdlib/string_methods"));
}
#[test]
fn test_loop_basic() {
    assert_snapshot!(run_aion_test("language/loop_basic"));
}
#[test]
fn test_function_types() {
    assert_snapshot!(run_aion_test("language/function_types"));
}
#[test]
fn test_match_expression() {
    assert_snapshot!(run_aion_test("language/match_expression"));
}
#[test]
fn test_option_result_methods() {
    assert_snapshot!(run_aion_test("language/option_result_methods"));
}
#[test]
fn test_sizeof_variables() {
    assert_snapshot!(run_aion_test("language/sizeof_variables"));
}
#[test]
fn test_integer_sizes() {
    assert_snapshot!(run_aion_test("language/integer_sizes"));
}
#[test]
fn test_integer_mismatch() {
    assert_snapshot!(run_aion_test("language/integer_mismatch"));
}
#[test]
fn test_tuple_basic() {
    assert_snapshot!(run_aion_test("language/tuple_basic"));
}
#[test]
fn test_tuple_return() {
    assert_snapshot!(run_aion_test("language/tuple_return"));
}
#[test]
fn test_tuple_nested() {
    assert_snapshot!(run_aion_test("language/tuple_nested"));
}
#[test]
fn test_array_basic() {
    assert_snapshot!(run_aion_test("language/array_basic"));
}
#[test]
fn test_array_bounds() {
    assert_snapshot!(run_aion_test("language/array_bounds"));
}
#[test]
fn test_array_as_param() {
    assert_snapshot!(run_aion_test("language/array_as_param"));
}
#[test]
fn test_array_return_error() {
    // #107 — returning a stack-allocated local array is a compile error.
    assert_snapshot!(run_aion_test("language/array_return_error"));
}
#[test]
fn test_array_return_literal() {
    // #107 — returning an array literal directly is the same dangling hole.
    assert_snapshot!(run_aion_test("language/array_return_literal"));
}

// --- Stdlib Tests ---

#[test]
fn test_sql_transpile() {
    assert_snapshot!(run_aion_test("stdlib/sql_transpile"));
}
#[test]
fn test_fs_write_read() {
    assert_snapshot!(run_aion_test("stdlib/fs_write_read"));
}
#[test]
fn test_env_args() {
    assert_snapshot!(run_aion_test("stdlib/env_args"));
}
#[test]
fn test_fs_result_error() {
    assert_snapshot!(run_aion_test("stdlib/fs_result_error"));
}
#[test]
fn test_std_fs_read_write() {
    assert_snapshot!(run_aion_test("stdlib/std_fs_read_write"));
}
#[test]
fn test_env_var() {
    assert_snapshot!(run_aion_test("stdlib/env_var"));
}
#[test]
fn test_env_args_cli() {
    assert_snapshot!(run_aion_test("stdlib/env_args_cli"));
}
#[test]
fn test_vector_basic() {
    assert_snapshot!(run_aion_test("stdlib/vector_basic"));
}
#[test]
fn test_env_vector_args() {
    assert_snapshot!(run_aion_test("stdlib/env_vector_args"));
}
#[test]
fn test_vector_generic() {
    assert_snapshot!(run_aion_test("stdlib/vector_generic"));
}
#[test]
fn test_vector_push_pop() {
    assert_snapshot!(run_aion_test("stdlib/vector_push_pop"));
}
#[test]
fn test_vector_utils() {
    assert_snapshot!(run_aion_test("stdlib/vector_utils"));
}
#[test]
fn test_vector_contains() {
    assert_snapshot!(run_aion_test("stdlib/vector_contains"));
}
#[test]
fn test_vector_edge() {
    assert_snapshot!(run_aion_test("stdlib/vector_edge"));
}
#[test]
fn test_hashmap_basic() {
    assert_snapshot!(run_aion_test("stdlib/hashmap_basic"));
}
#[test]
fn test_hashmap_resize_hashset() {
    assert_snapshot!(run_aion_test("stdlib/hashmap_resize_hashset"));
}
#[test]
fn test_hashmap_utils() {
    assert_snapshot!(run_aion_test("stdlib/hashmap_utils"));
}
#[test]
fn test_ordered_map_basic() {
    // #136 — insertion-ordered associative array with deterministic iteration.
    assert_snapshot!(run_aion_test("stdlib/ordered_map_basic"));
}
#[test]
fn test_string_escape_llvm() {
    // #139 — escapes a String for inclusion inside an LLVM `c"..."` literal.
    assert_snapshot!(run_aion_test("stdlib/string_escape_llvm"));
}
#[test]
fn test_string_join() {
    // #140 — concatenates a Vector<String> with a separator between elements.
    assert_snapshot!(run_aion_test("stdlib/string_join"));
}
#[test]
fn test_fmt_s() {
    // #137 — placeholder formatter with explicit numeric `{N}` indices.
    assert_snapshot!(run_aion_test("stdlib/fmt_s"));
}
#[test]
fn test_tensor_basic() {
    assert_snapshot!(run_aion_test("stdlib/tensor_basic"));
}
#[test]
fn test_fmt_format() {
    assert_snapshot!(run_aion_test("stdlib/fmt_format"));
}
#[test]
fn test_path_operations() {
    assert_snapshot!(run_aion_test("stdlib/path_operations"));
}
#[test]
fn test_dataframe_basic() {
    assert_snapshot!(run_aion_test("stdlib/dataframe_basic"));
}
#[test]
fn test_sql_advanced() {
    assert_snapshot!(run_aion_test("stdlib/sql_advanced"));
}
#[test]
fn test_json_parse_basic() {
    assert_snapshot!(run_aion_test("stdlib/json_parse_basic"));
}
#[test]
fn test_memzero_struct() {
    assert_snapshot!(run_aion_test("stdlib/memzero_struct"));
}
#[test]
fn test_memzero_typed() {
    assert_snapshot!(run_aion_test("stdlib/memzero_typed"));
}
#[test]
fn test_ptr_deref_member() {
    assert_snapshot!(run_aion_test("stdlib/ptr_deref_member"));
}
#[test]
fn test_ptr_deref_member_err() {
    assert_snapshot!(run_aion_test("stdlib/ptr_deref_member_err"));
}
#[test]
fn test_ptr_memzero_field() {
    assert_snapshot!(run_aion_test("stdlib/ptr_memzero_field"));
}

// --- Compiler Tests ---

#[test]
fn test_debug_output() {
    assert_snapshot!(run_aion_test("compiler/debug_output"));
}
#[test]
fn test_malloc_test() {
    assert_snapshot!(run_aion_test("compiler/malloc_test"));
}
#[test]
fn test_gc_leak_test() {
    assert_snapshot!(run_aion_test("compiler/gc_leak_test"));
}
#[test]
fn test_extern_ffi() {
    assert_snapshot!(run_aion_test("compiler/extern_ffi"));
}
#[test]
fn test_self_lexer() {
    assert_snapshot!(run_aion_test("compiler/self_lexer"));
}
#[test]
fn test_optimization_check() {
    assert_snapshot!(run_aion_test("compiler/optimization_check"));
}
#[test]
fn test_self_lexer_loop() {
    assert_snapshot!(run_aion_test("compiler/self_lexer_loop"));
}
#[test]
fn test_self_parser() {
    assert_snapshot!(run_aion_test("compiler/self_parser"));
}
#[test]
fn test_error_undefined_function() {
    assert_snapshot!(run_aion_test("compiler/error_undefined_function"));
}
#[test]
fn test_error_undefined_field() {
    assert_snapshot!(run_aion_test("compiler/error_undefined_field"));
}
#[test]
fn test_error_undefined_method() {
    assert_snapshot!(run_aion_test("compiler/error_undefined_method"));
}
#[test]
fn test_error_internal() {
    assert_snapshot!(run_aion_test("compiler/error_internal"));
}
#[test]
fn test_mut_use() {
    // #141 — verifies two-way cross-module `use` (mutual imports) compiles
    // and runs correctly between sibling files in `compiler/`.
    assert_snapshot!(run_aion_test("compiler/mut_use"));
}
#[test]
fn test_mut_use_missing() {
    // #141 — error path: importing a non-existent module surfaces the
    // "Import not found" resolver error rather than crashing silently.
    assert_snapshot!(run_aion_test("compiler/mut_use_missing"));
}

// --- Example Tests ---

#[test]
fn test_example_hello() {
    assert_snapshot!(run_aion_test("examples/hello"));
}
#[test]
fn test_example_types() {
    assert_snapshot!(run_aion_test("examples/types"));
}
#[test]
fn test_example_logic() {
    assert_snapshot!(run_aion_test("examples/logic"));
}
#[test]
fn test_example_generics() {
    assert_snapshot!(run_aion_test("examples/generics"));
}
#[test]
fn test_example_interface_impl() {
    assert_snapshot!(run_aion_test("examples/interface_impl"));
}

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
fn test_doc_gen() {
    assert_snapshot!(run_aion_doc("compiler/doc_gen"));
}

#[test]
fn test_function_pointer() {
    assert_snapshot!(run_aion_test("language/function_pointer"));
}

#[test]
fn test_function_as_param() {
    assert_snapshot!(run_aion_test("language/function_as_param"));
}
