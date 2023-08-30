mod interactive;

use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use core_sim::{
    debug_symbol::DebugSymbol,
    io::{BinaryInput, BinaryOutput, EmptyIO, Input, Output},
    ppm::PPMData,
    sim::Simulator,
    sld::SldData,
};

#[cfg(feature = "stat")]
use terminal_size::terminal_size;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// simulate raytracer (with sld file)
    Rt(RtArgs),
    /// simulate core
    Exe(ExeArgs),
}

#[derive(Args, Debug)]
struct CommonArgs {
    /// File path to input assembly
    #[arg(short, long)]
    input: PathBuf,
    /// Enable interactive mode
    #[arg(long)]
    interactive: bool,
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
    /// File path to debug symbol
    #[arg(long = "dbg")]
    debug_symbol: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct RtArgs {
    #[command(flatten)]
    delegate: CommonArgs,
    /// File path to input sld
    #[arg(short, long)]
    sld: PathBuf,
    /// File path to output
    #[arg(short, long)]
    ppm: PathBuf,
}

#[derive(Args, Debug)]
struct ExeArgs {
    #[command(flatten)]
    delegate: CommonArgs,
    /// File path to content of stdin (empty to no input)
    #[arg(long)]
    stdin: Option<PathBuf>,
    /// File path to content of stdout (empty to no output)
    #[arg(long)]
    stdout: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    match args.command {
        Command::Rt(RtArgs {
            delegate:
                CommonArgs {
                    input,
                    interactive,
                    debug_symbol,
                    verbose,
                },
            sld,
            ppm,
        }) => {
            if verbose {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                    .init();
            } else {
                env_logger::init();
            }
            let mem = read_input(input)?;
            let sld = {
                let mut buf = String::new();
                let mut file = File::open(sld)?;
                file.read_to_string(&mut buf)?;
                buf
            };
            let debug_symbol = read_dbg_symb(debug_symbol)?;

            let input = SldData::parse(&sld)?;
            log::info!("finished parsing SLD. # of object: {}", input.num_objects);
            let mut sim = Simulator::new(&mem, input, PPMData::new())?;
            sim.provide_dbg_symb(debug_symbol);
            execute(&mut sim, interactive)?;
            log::info!("finished execution.");
            output_stat(&sim);
            let sim_output = sim.into_output();
            let h = sim_output.cpu_output.verify_header()?;
            log::info!("PPM generated. {h:?}");
            let mut out = File::create(ppm)?;
            out.write_all(&sim_output.cpu_output.into_inner())?;
            Ok(())
        }
        Command::Exe(ExeArgs {
            delegate:
                CommonArgs {
                    input,
                    interactive,
                    debug_symbol,
                    verbose,
                },
            stdin,
            stdout,
        }) => {
            if verbose {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                    .init();
            } else {
                env_logger::init();
            }
            let mem = read_input(input)?;
            let debug_symbol = read_dbg_symb(debug_symbol)?;
            macro_rules! b_in {
                ($input:ident) => {{
                    let input = {
                        let mut buf = Vec::new();
                        let mut file = File::open(&$input)?;
                        file.read_to_end(&mut buf)?;
                        BinaryInput::new(buf)
                    };
                    input
                }};
                () => {
                    EmptyIO::new()
                };
            }
            macro_rules! b_out {
                ($output:ident) => {
                    match stdin {
                        Some(stdin) => {
                            let mut sim = Simulator::new(&mem, b_in!(stdin), $output)?;
                            sim.provide_dbg_symb(debug_symbol);
                            execute(&mut sim, interactive)?;
                            output_stat(&sim);
                            sim.into_output()
                        }
                        None => {
                            let mut sim = Simulator::new(&mem, b_in!(), $output)?;
                            sim.provide_dbg_symb(debug_symbol);
                            execute(&mut sim, interactive)?;
                            output_stat(&sim);
                            sim.into_output()
                        }
                    }
                };
            }
            match stdout {
                Some(stdout) => {
                    let output = BinaryOutput::new();
                    let sim_output = b_out!(output);
                    let mut out = File::create(stdout)?;
                    out.write_all(&sim_output.cpu_output.into_inner())?;
                }
                None => {
                    let output = EmptyIO::new();
                    let _sim_output = b_out!(output);
                }
            }
            Ok(())
        }
    }
}

#[cfg(not(feature = "stat"))]
fn output_stat<I, O>(_: &Simulator<I, O>) {}

#[cfg(feature = "stat")]
fn output_stat<I, O>(sim: &Simulator<I, O>) {
    let max_width = get_terminal_width().unwrap_or(120) as usize;
    log::info!("statistics:\n{}", sim.collect_stat().view(max_width));
}

#[cfg(feature = "stat")]
fn get_terminal_width() -> Option<u16> {
    terminal_size().map(|(w, _)| w.0 - 20)
}

fn read_dbg_symb(debug_symbol: Option<PathBuf>) -> Result<DebugSymbol> {
    let debug_symbol = match debug_symbol {
        Some(p) => {
            let file = File::open(p)?;
            DebugSymbol::deser(file)?
        }
        None => Default::default(),
    };
    Ok(debug_symbol)
}

fn read_input(input: PathBuf) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut file = File::open(input)?;
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn execute<I: Input, O: Output>(sim: &mut Simulator<I, O>, interactive: bool) -> Result<()> {
    if interactive {
        interactive::execute_interactive(sim)
    } else {
        loop {
            let r = sim.single_cycle(&Default::default())?;
            if let Some(c) = r.exit_code() {
                if c.is_success() {
                    break Ok(());
                } else {
                    let how = sim.get_error_msg().unwrap();
                    break Err(anyhow::anyhow!("simulator returns an error: {how}. try executing process with --interactive to debug."));
                }
            }
        }
    }
}
