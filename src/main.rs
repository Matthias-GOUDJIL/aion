use aionc::{compile_file, generate_docs, transpile_sql};
use clap::{Parser, Subcommand};
use std::fs;
use std::process::Command;

#[derive(Parser)]
#[command(name = "aion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        input: String,
        #[arg(short, default_value = "output.ll")]
        output: String,
    },
    Doc {
        input: String,
        #[arg(short, default_value = "API.md")]
        output: String,
    },
    Run {
        input: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Transpile {
        input: String,
        #[arg(short, default_value = "output.sql")]
        output: String,
        #[arg(short, long, default_value = "sql")]
        target: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { input, output } => {
            if let Err(e) = compile_file(&input, &output) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Doc { input, output } => match generate_docs(&input) {
            Ok(doc) => {
                if let Err(e) = fs::write(&output, doc) {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Run { input, args } => {
            let run_id =
                std::env::var("AION_RUN_ID").unwrap_or_else(|_| std::process::id().to_string());
            let ir_file = format!("temp_{}.ll", run_id);
            let obj_file = format!("temp_{}.o", run_id);
            let bin_file = format!("./aion_app_{}", run_id);

            if let Err(e) = compile_file(&input, &ir_file) {
                let _ = fs::remove_file(&ir_file);
                eprintln!("{}", e);
                std::process::exit(1);
            }

            // Fix: Add PIC relocation model for modern Linux compatibility
            let llc_status = Command::new("llc-15")
                .args([
                    "-filetype=obj",
                    "-relocation-model=pic",
                    &ir_file,
                    "-o",
                    &obj_file,
                ])
                .status();

            if let Err(e) = llc_status {
                let _ = fs::remove_file(&ir_file);
                let _ = fs::remove_file(&obj_file);
                eprintln!("error: llc failed: {}", e);
                std::process::exit(1);
            } else if !llc_status.unwrap().success() {
                let _ = fs::remove_file(&ir_file);
                let _ = fs::remove_file(&obj_file);
                eprintln!("error: llc failed with non-zero exit");
                std::process::exit(1);
            }

            // Link the object with the pre-compiled C runtime bitcode using
            // lld-15 (driven by clang-15, an LLVM tool — no gcc in the link
            // path). Falls back to the legacy gcc link if clang-15 is absent
            // (e.g. bare-metal dev without the LLVM toolchain). #73.
            let link_ok = link_with_lld(&obj_file, &bin_file, &args);
            match link_ok {
                Ok(()) => {}
                Err(LinkError::GccFallback) => {
                    let gcc_status = Command::new("gcc")
                        .args([
                            &obj_file,
                            "src/runtime.c",
                            "-o",
                            &bin_file,
                            "-lpthread",
                            "-lgc",
                            "-lm",
                        ])
                        .status();
                    if let Err(e) = gcc_status {
                        let _ = fs::remove_file(&ir_file);
                        let _ = fs::remove_file(&obj_file);
                        eprintln!("error: gcc failed: {}", e);
                        std::process::exit(1);
                    } else if !gcc_status.unwrap().success() {
                        let _ = fs::remove_file(&ir_file);
                        let _ = fs::remove_file(&obj_file);
                        eprintln!("error: gcc failed with non-zero exit");
                        std::process::exit(1);
                    }
                }
                Err(LinkError::Failed(e)) => {
                    let _ = fs::remove_file(&ir_file);
                    let _ = fs::remove_file(&obj_file);
                    eprintln!("error: link failed: {}", e);
                    std::process::exit(1);
                }
            }

            println!("-------------------------------");
            let output = Command::new(&bin_file).args(&args).output();

            // Propagate the child exit code so shell/CI callers detect
            // runtime failures (OOB traps, non-zero returns). #106.
            let child_code: i32;
            match output {
                Ok(out) => {
                    print!("{}", String::from_utf8_lossy(&out.stdout));
                    eprint!("{}", String::from_utf8_lossy(&out.stderr));
                    child_code = out.status.code().unwrap_or(1);
                    if !out.status.success() {
                        eprintln!("error: process exited with code: {}", child_code);
                    }
                }
                Err(e) => {
                    eprintln!("error: execution failed: {}", e);
                    child_code = 1;
                }
            }
            println!("-------------------------------");

            let _ = fs::remove_file(ir_file);
            let _ = fs::remove_file(obj_file);
            let _ = fs::remove_file(bin_file);

            std::process::exit(child_code);
        }
        Commands::Transpile {
            input,
            output,
            target,
        } => {
            if target != "sql" {
                eprintln!("error: only SQL target is supported for now");
                std::process::exit(1);
            }

            match transpile_sql(&input) {
                Ok(result) => {
                    // Diagnostics carry their own spans/snippets; errors
                    // mean no SQL was generated and nothing is written.
                    for d in &result.diagnostics {
                        eprint!("{}", d.report());
                    }
                    if result.has_errors() {
                        eprintln!(
                            "error: {} transpile error(s) — no SQL was generated",
                            result
                                .diagnostics
                                .iter()
                                .filter(|d| !d.is_warning())
                                .count()
                        );
                        std::process::exit(1);
                    }
                    if let Err(e) = fs::write(&output, &result.sql) {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Outcome of the lld link attempt. `GccFallback` signals that clang-15 is
/// unavailable and the caller should retry with the legacy gcc link path. #73.
enum LinkError {
    GccFallback,
    Failed(String),
}

/// Link `obj_file` + the C runtime bitcode into `bin_file` using lld-15
/// (driven by `clang-15 -fuse-ld=lld`). The runtime bitcode is resolved as:
/// `AION_RUNTIME_BC` env → `/opt/aion_runtime.bc` (Docker image) → on-the-fly
/// compile of `src/runtime.c`. Returns `GccFallback` if clang-15 is missing
/// so the caller can fall back to the gcc path. #73.
fn link_with_lld(obj_file: &str, bin_file: &str, _args: &[String]) -> Result<(), LinkError> {
    let clang = which("clang-15").or_else(|| which("clang"));
    let clang = match clang {
        Some(c) => c,
        None => return Err(LinkError::GccFallback),
    };

    // Resolve the pre-compiled runtime bitcode, unless src/runtime.c has been
    // modified more recently than the pre-baked bitcode (#204).
    let src_runtime = std::path::Path::new("src/runtime.c");
    let src_mtime = std::fs::metadata(src_runtime)
        .and_then(|m| m.modified())
        .ok();
    let is_fresh = |bc_path: &std::path::Path| -> bool {
        if !bc_path.exists() {
            return false;
        }
        match (
            src_mtime,
            std::fs::metadata(bc_path).and_then(|m| m.modified()).ok(),
        ) {
            (Some(s), Some(b)) => b >= s,
            _ => true,
        }
    };

    let cached_tmp = std::path::Path::new("/tmp/aion_runtime_cached.bc");
    let runtime_bc = std::env::var("AION_RUNTIME_BC")
        .ok()
        .filter(|p| is_fresh(std::path::Path::new(p)))
        .or_else(|| {
            let opt = std::path::Path::new("/opt/aion_runtime.bc");
            if is_fresh(opt) {
                Some(opt.to_string_lossy().into_owned())
            } else if is_fresh(cached_tmp) {
                Some(cached_tmp.to_string_lossy().into_owned())
            } else {
                None
            }
        });

    let runtime_bc = match runtime_bc {
        Some(p) => p,
        None => {
            // On-the-fly bitcode compile (non-Docker dev or updated runtime.c).
            // Compile to a PID-scoped temp file then atomically rename to
            // `/tmp/aion_runtime_cached.bc` so concurrent/subsequent invocations
            // reuse the compiled bitcode (#206).
            let pid_tmp = format!("/tmp/aion_runtime_{}.bc", std::process::id());
            let status = Command::new(&clang)
                .args([
                    "-c",
                    "-emit-llvm",
                    "-O2",
                    "-I/usr/include",
                    "src/runtime.c",
                    "-o",
                    &pid_tmp,
                ])
                .status()
                .map_err(|e| LinkError::Failed(format!("clang bitcode compile: {}", e)))?;
            if !status.success() {
                let _ = std::fs::remove_file(&pid_tmp);
                return Err(LinkError::Failed(format!(
                    "clang bitcode compile exited with {}",
                    status.code().unwrap_or(-1)
                )));
            }
            if std::fs::rename(&pid_tmp, cached_tmp).is_ok() {
                cached_tmp.to_string_lossy().into_owned()
            } else {
                pid_tmp
            }
        }
    };

    let status = Command::new(&clang)
        .args([
            "-fuse-ld=lld",
            obj_file,
            &runtime_bc,
            "-o",
            bin_file,
            "-lpthread",
            "-lgc",
            "-lm",
        ])
        .status()
        .map_err(|e| LinkError::Failed(format!("lld link: {}", e)))?;

    if !status.success() {
        return Err(LinkError::Failed(format!(
            "lld link exited with {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

/// Minimal `which` lookup — returns the first PATH hit for `name`. We avoid
/// pulling in the `which` crate for a one-shot PATH scan. #73.
fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}
