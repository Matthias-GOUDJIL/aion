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
            println!("🚀 Compiling and Running {}...", input);
            
            let ir_file = "temp.ll";
            let obj_file = "temp.o";
            let bin_file = "./aion_app";

            if let Err(e) = compile_file(&input, ir_file) {
                println!("❌ Aion Compilation Error: {}", e);
                return;
            }

            // Correction: Ajout du modèle de relocation PIC pour compatibilité Linux moderne
            let llc_status = Command::new("llc-15")
                .args(&["-filetype=obj", "-relocation-model=pic", ir_file, "-o", obj_file])
                .status();

            if llc_status.is_err() || !llc_status.unwrap().success() {
                println!("❌ LLVM Backend Error (llc failed)");
                return;
            }

            let gcc_status = Command::new("gcc")
                .args(&[obj_file, "src/runtime.c", "-o", bin_file, "-lpthread"])
                .status();

            if gcc_status.is_err() || !gcc_status.unwrap().success() {
                println!("❌ Linking Error (gcc failed)");
                return;
            }

            println!("✨ Execution Output:");
            println!("-------------------------------");
            let output = Command::new(bin_file).output();
            
            match output {
                Ok(out) => {
                    print!("{}", String::from_utf8_lossy(&out.stdout));
                    if !out.status.success() {
                        println!("⚠️ Process exited with code: {}", out.status.code().unwrap_or(-1));
                    }
                },
                Err(e) => println!("❌ Execution Error: {}", e),
            }
            println!("-------------------------------");

            let _ = fs::remove_file(ir_file);
            let _ = fs::remove_file(obj_file);
            let _ = fs::remove_file(bin_file);
        }
    }
}
