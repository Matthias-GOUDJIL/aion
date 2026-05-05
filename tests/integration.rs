use assert_cmd::Command;
use insta::assert_snapshot;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_aion_test(fixture: &str) -> String {
    let root = project_root();

    let output = Command::new("cargo")
        .args(["run", "--quiet", "--", "run", &format!("tests/fixtures/{}.ai", fixture)])
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
        if line.starts_with("--------------------------------") {
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

// --- Tests ---

#[test]
fn test_001_hello() {
    assert_snapshot!(run_aion_test("001_hello"));
}

#[test]
fn test_002_duration() {
    assert_snapshot!(run_aion_test("002_duration"));
}

#[test]
fn test_003_pipeline() {
    assert_snapshot!(run_aion_test("003_pipeline"));
}

#[test]
fn test_004_date_arithmetic() {
    assert_snapshot!(run_aion_test("004_date_arithmetic"));
}

#[test]
fn test_006_sql() {
    assert_snapshot!(run_aion_test("006_sql"));
}

#[test]
fn test_007_fstring() {
    assert_snapshot!(run_aion_test("007_fstring"));
}

#[test]
fn test_008_operators() {
    assert_snapshot!(run_aion_test("008_operators"));
}

#[test]
fn test_009_unsafe_success() {
    assert_snapshot!(run_aion_test("009_unsafe_success"));
}

#[test]
fn test_010_complex_pipeline() {
    assert_snapshot!(run_aion_test("010_complex_pipeline"));
}

#[test]
fn test_011_fs_test() {
    assert_snapshot!(run_aion_test("011_fs_test"));
}

#[test]
fn test_012_env_test() {
    assert_snapshot!(run_aion_test("012_env_test"));
}

#[test]
fn test_013_string_test() {
    assert_snapshot!(run_aion_test("013_string_test"));
}

#[test]
fn test_014_enum_match() {
    assert_snapshot!(run_aion_test("014_enum_match"));
}

#[test]
fn test_015_result_fs() {
    assert_snapshot!(run_aion_test("015_result_fs"));
}

#[test]
fn test_016_generics_test() {
    assert_snapshot!(run_aion_test("016_generics_test"));
}

#[test]
fn test_017_generic_result() {
    assert_snapshot!(run_aion_test("017_generic_result"));
}

#[test]
fn test_018_struct_ambiguity() {
    assert_snapshot!(run_aion_test("018_struct_ambiguity"));
}

#[test]
fn test_019_simple() {
    assert_snapshot!(run_aion_test("019_simple"));
}

#[test]
fn test_019_std_fs() {
    assert_snapshot!(run_aion_test("019_std_fs"));
}

#[test]
fn test_020_env_var() {
    assert_snapshot!(run_aion_test("020_env_var"));
}

#[test]
fn test_021_env_args() {
    assert_snapshot!(run_aion_test("021_env_args"));
}

#[test]
fn test_022_vector_repro() {
    assert_snapshot!(run_aion_test("022_vector_repro"));
}

#[test]
fn test_023_debug() {
    assert_snapshot!(run_aion_test("023_debug"));
}

#[test]
fn test_024_env_vector() {
    assert_snapshot!(run_aion_test("024_env_vector"));
}

#[test]
fn test_025_vector_generic() {
    assert_snapshot!(run_aion_test("025_vector_generic"));
}

#[test]
fn test_026_simple_vector() {
    assert_snapshot!(run_aion_test("026_simple_vector"));
}

#[test]
fn test_027_malloc_test() {
    assert_snapshot!(run_aion_test("027_malloc_test"));
}

#[test]
fn test_028_local_generic() {
    assert_snapshot!(run_aion_test("028_local_generic"));
}

#[test]
fn test_029_hashmap() {
    assert_snapshot!(run_aion_test("029_hashmap"));
}

#[test]
fn test_030_collections_extra() {
    assert_snapshot!(run_aion_test("030_collections_extra"));
}

#[test]
fn test_031_method_chaining() {
    assert_snapshot!(run_aion_test("031_method_chaining"));
}

#[test]
fn test_032_short_circuit() {
    assert_snapshot!(run_aion_test("032_short_circuit"));
}

#[test]
fn test_033_tensor_basic() {
    assert_snapshot!(run_aion_test("033_tensor_basic"));
}

#[test]
fn test_034_gc_leak() {
    assert_snapshot!(run_aion_test("034_gc_leak"));
}

#[test]
fn test_035_extern_ffi() {
    assert_snapshot!(run_aion_test("035_extern_ffi"));
}

#[test]
fn test_036_fmt_test() {
    assert_snapshot!(run_aion_test("036_fmt_test"));
}

#[test]
fn test_037_path_test() {
    assert_snapshot!(run_aion_test("037_path_test"));
}

#[test]
fn test_038_dataframe_basic() {
    assert_snapshot!(run_aion_test("038_dataframe_basic"));
}

#[test]
fn test_039_result_basic() {
    assert_snapshot!(run_aion_test("039_result_basic"));
}

#[test]
fn test_040_result_methods() {
    assert_snapshot!(run_aion_test("040_result_methods"));
}

#[test]
fn test_041_self_lexer() {
    assert_snapshot!(run_aion_test("041_self_lexer"));
}

#[test]
fn test_042_sql_advanced() {
    assert_snapshot!(run_aion_test("042_sql_advanced"));
}

#[test]
fn test_043_optimization_check() {
    assert_snapshot!(run_aion_test("043_optimization_check"));
}

#[test]
fn test_044_self_lexer_loop() {
    assert_snapshot!(run_aion_test("044_self_lexer_loop"));
}

#[test]
fn test_045_self_parser() {
    assert_snapshot!(run_aion_test("045_self_parser"));
}

#[test]
fn test_046_primitive_methods() {
    assert_snapshot!(run_aion_test("046_primitive_methods"));
}
