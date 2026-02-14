use clap::{Parser, Subcommand};
use aionc::{compile_file, generate_docs};
use std::fs;
use std::process::Command;
use std::path::Path;

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
    }
}

fn compile_runtime() -> String {
    let runtime_src = "src/runtime.c";
    let runtime_lib = "libruntime.so";
    
    if Path::new(runtime_src).exists() {
        let status = Command::new("gcc")
            .args(&["-shared", "-fPIC", "-o", runtime_lib, runtime_src, "-lpthread"])
            .status();
            
        if let Ok(s) = status {
            if s.success() {
                return runtime_lib.to_string();
            }
        }
        println!("⚠️ Warning: Failed to compile runtime. Concurrency might fail.");
    }
    String::new()
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { input, output } => {
            println!("🚀 Building {}...", input);
            if let Err(e) = compile_file(&input, &output) {
                println!("❌ Error: {}", e);
            } else {
                println!("✨ Success! Generated {}", output);
            }
        }
        Commands::Doc { input, output } => {
            println!("🧠 Generating AI documentation for {}...", input);
            match generate_docs(&input) {
                Ok(doc) => {
                    fs::write(&output, doc).unwrap();
                    println!("✨ Documentation generated in {}", output);
                },
                Err(e) => println!("❌ Error: {}", e),
            }
        }
        Commands::Run { input } => {
            println!("🚀 Running {}...", input);
            
            // 1. Compiler le runtime C (si nécessaire)
            let runtime_lib = compile_runtime();
            
            // 2. Compiler le code Aion en IR
            let temp_ll = "temp.ll";
            if let Err(e) = compile_file(&input, temp_ll) {
                println!("❌ Compilation Error: {}", e);
                return;
            }

            // 3. Exécuter avec lli en chargeant le runtime
            let mut cmd = Command::new("lli-15");
            cmd.arg(temp_ll);
            
            if !runtime_lib.is_empty() {
                // Charger la librairie dynamique contenant aion_spawn
                cmd.arg("-load").arg(format!("./{}", runtime_lib));
            }

            let status = cmd.status();
            
            match status {
                Ok(s) if s.success() => println!("\n✅ Execution finished."),
                _ => println!("\n⚠️ Execution failed."),
            }
            let _ = fs::remove_file(temp_ll);
        }
    }
}
