use clap::{Parser, Subcommand};
use aionc::{compile_file, generate_docs};
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
    /// Compile et exécute immédiatement (Interprétation JIT)
    Run {
        input: String,
    }
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
            let temp_ll = "temp.ll";
            if let Err(e) = compile_file(&input, temp_ll) {
                println!("❌ Compilation Error: {}", e);
                return;
            }

            // Exécution via l'interpréteur LLVM (lli)
            let status = Command::new("lli-15")
                .arg(temp_ll)
                .status();
            
            match status {
                Ok(s) if s.success() => println!("\n✅ Execution finished."),
                _ => println!("\n⚠️ Execution failed or lli-15 not found."),
            }
            let _ = fs::remove_file(temp_ll);
        }
    }
}
