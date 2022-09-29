use chprog_lib::ChProg;
use clap::Parser;

/// CH55x UART serial bootloader flash tool
#[derive(Parser, Debug)]
#[clap(version = "0.1.0")]
#[clap(
    author = "Aleksey Evtyushkin <earvest@gmail.com>",
    about,
    long_about = "ChProg is an application for firmware operations using UART with WCH CH55x series microcontrollers"
)]
struct Args {
    /// Serial port name to use
    #[clap(short, long, value_parser)]
    port: String,

    /// Write file to flash, verify and exit the bootloader
    #[clap(short, long, action)]
    write: bool,

    /// Verify flash against the provided file
    #[clap(short, long, action)]
    verify: bool,

    /// Detect chip and bootloader version
    #[clap(short, long, action)]
    detect: bool,

    /// Erase flash
    #[clap(short, long, action)]
    erase: bool,

    /// Reset chip
    #[clap(short, long, action)]
    reset: bool,

    /// Target file to be flashed
    #[clap(short, long, action)]
    file: Option<String>,
}

fn main() {
    let args = Args::parse();

    // Try to open serial port
    if let Ok(mut chprog) = ChProg::new(args.port.clone()) {
        if args.reset {
            // Reset
            println!("Resetting");
            chprog.reset();
        }

        if args.detect {
            // Detect
            println!("Detecting");
            if let Err(err) = chprog.detect() {
                println!("ERROR: Detecting failed: {}", err);
                return;
            }
        }

        if args.erase {
            // Erase
            if let Err(err) = chprog.erase() {
                println!("ERROR: Erasing failed: {}", err);
                return;
            }
        }

        if let Some(filename) = args.file {
            if args.verify && !args.write {
                // Verify
                if let Err(err) = chprog.verify(filename) {
                    println!("ERROR: Verification failed: {}", err);
                } else {
                    println!("Verification OK");
                }
                return;
            }

            if args.write {
                // Write
                if let Err(err) = chprog.flash(filename) {
                    println!("ERROR: Write failed: {}", err);
                } else {
                    println!("Write OK");
                }
            }
        }
    } else {
        // Unsuccessful attempt to open port
        println!("ERROR: Cannot open port: {}", args.port);
    }
}
