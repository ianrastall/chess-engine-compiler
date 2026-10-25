use clap::{Parser, Subcommand};
use compiler_core::Compiler;

#[derive(Parser)]
#[command(version, about = "Compile chess engines using the Jupiter build core")]
struct Args {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Print detected MSYS2 UCRT64 tools and default options.
    Doctor {
        #[arg(long, default_value = "stockfish", value_parser = ["stockfish", "lc0", "berserk", "obsidian", "reckless"])]
        engine: String,
    },
    /// Validate a JSON build configuration and print its steps.
    Plan { config: std::path::PathBuf },
    /// Compile and verify an engine from a JSON build configuration.
    Build { config: std::path::PathBuf },
    /// Print this computer's compatible CPU targets and recommendation.
    Hardware,
    /// Browse upstream GitHub versions/downloads. Credentials never enter command arguments.
    Github {
        #[arg(value_parser = ["stockfish", "lc0", "berserk", "obsidian", "reckless"])]
        engine: String,
        #[arg(long, default_value="catalog", value_parser=["catalog","assets","artifacts","download-release","download-artifact","account"])]
        action: String,
        #[arg(long, default_value="anonymous", value_parser=["anonymous","cli"])]
        auth: String,
        #[arg(long, default_value_t = 1)]
        page: u32,
        #[arg(long, default_value_t = 0)]
        id: u64,
        #[arg(long, default_value = "")]
        output: String,
    },
}
fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    match Args::parse().command {
        Action::Hardware => println!(
            "{}",
            serde_json::to_string_pretty(&compiler_core::hardware::detect())?
        ),
        Action::Github {
            engine,
            action,
            auth,
            page,
            id,
            output,
        } => {
            let action = match action.as_str() {
                "assets" => 1,
                "artifacts" => 2,
                "download-release" => 3,
                "download-artifact" => 4,
                "account" => 5,
                _ => 0,
            };
            let mut worker = compiler_core::github::Worker::default();
            worker.start(compiler_core::github::Query {
                action,
                engine,
                auth_mode: auth,
                page,
                id,
                output_folder: output,
                ..Default::default()
            })?;
            loop {
                let result = worker.snapshot();
                if result.state != "Running" {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                    if result.state != "Completed" {
                        return Err(result.message.into());
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
        Action::Doctor { engine } => println!(
            "{}",
            serde_json::to_string_pretty(&compiler_core::defaults(&engine))?
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
