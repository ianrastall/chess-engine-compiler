use clap::{Parser, Subcommand};
use compiler_core::{BuildOptions, Compiler};

#[derive(Parser)]
#[command(version, about = "Compile chess engines using the Jupiter build core")]
struct Args {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Print detected MSYS2 UCRT64 tools and default options.
    Doctor,
    /// Validate a JSON build configuration and print its steps.
    Plan { config: std::path::PathBuf },
    /// Compile and verify an engine from a JSON build configuration.
    Build { config: std::path::PathBuf },
}
fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    match Args::parse().command {
        Action::Doctor => println!(
            "{}",
            serde_json::to_string_pretty(&BuildOptions::default())?
        ),
        Action::Plan { config } => {
            let options = serde_json::from_slice(&std::fs::read(config)?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&compiler_core::plan(&options)?)?
            );
        }
        Action::Build { config } => {
            let options = serde_json::from_slice(&std::fs::read(config)?)?;
            let core = Compiler::default();
            core.start(options)?;
            let mut previous_phase = String::new();
            loop {
                let snapshot = core.snapshot();
                if snapshot.phase != previous_phase {
                    println!("{}", snapshot.phase);
                    previous_phase = snapshot.phase.clone();
                }
                if !matches!(snapshot.state.as_str(), "Running" | "Cancelling") {
                    println!(
                        "{}\nLog: {}\nOutput: {}",
                        snapshot.message, snapshot.log_path, snapshot.output
                    );
                    if snapshot.state != "Completed" {
                        return Err(snapshot.message.into());
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    }
    Ok(())
}
