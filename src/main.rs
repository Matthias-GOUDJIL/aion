use clap::{Parser, Subcommand};
use aionc::{compile_file, generate_docs, transpile_sql};
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
    }
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
        Commands::Doc { input, output } => {
            match generate_docs(&input) {
                Ok(doc) => {
                    if let Err(e) = fs::write(&output, doc) {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Run { input, args } => {
            
            let run_id = std::env::var("AION_RUN_ID").unwrap_or_else(|_| std::process::id().to_string());
            let ir_file = format!("temp_{}.ll", run_id);
            let obj_file = format!("temp_{}.o", run_id);
            let bin_file = format!("./aion_app_{}", run_id);

            if let Err(e) = compile_file(&input, &ir_file) {
                eprintln!("{}", e);
                std::process::exit(1);
            }

            // Fix: Add PIC relocation model for modern Linux compatibility
            let llc_status = Command::new("llc-15")
                .args(["-filetype=obj", "-relocation-model=pic", &ir_file, "-o", &obj_file])
                .status();

            if let Err(e) = llc_status {
                eprintln!("error: llc failed: {}", e);
                std::process::exit(1);
            } else if !llc_status.unwrap().success() {
                eprintln!("error: llc failed with non-zero exit");
                std::process::exit(1);
            }

            let gcc_status = Command::new("gcc")
                .args([&obj_file, "src/runtime.c", "-o", &bin_file, "-lpthread", "-lgc"])
                .status();

            if let Err(e) = gcc_status {
                eprintln!("error: gcc failed: {}", e);
                std::process::exit(1);
            } else if !gcc_status.unwrap().success() {
                eprintln!("error: gcc failed with non-zero exit");
                std::process::exit(1);
            }

            println!("-------------------------------");
            let output = Command::new(&bin_file)
                .args(&args)
                .output();
            
            match output {
                Ok(out) => {
                    print!("{}", String::from_utf8_lossy(&out.stdout));
                    eprint!("{}", String::from_utf8_lossy(&out.stderr));
                    if !out.status.success() {
                        eprintln!("error: process exited with code: {}", out.status.code().unwrap_or(-1));
                    }
                },
                Err(e) => eprintln!("error: execution failed: {}", e),
            }
            println!("-------------------------------");

            let _ = fs::remove_file(ir_file);
            let _ = fs::remove_file(obj_file);
            let _ = fs::remove_file(bin_file);
        }
        Commands::Transpile { input, output, target } => {
            if target != "sql" {
                eprintln!("error: only SQL target is supported for now");
                return;
            }
            
            match transpile_sql(&input) {
                Ok(sql) => {
                    if let Err(e) = fs::write(&output, sql) {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                },
                Err(e) => eprintln!("error: {}", e),
            }
        }
    }
}
