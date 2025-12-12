use chrono::Local;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("build-logger error: {err}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    // Open or create the build log file
    let mut log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("build_errors.log")?;

    // Write build start timestamp
    writeln!(
        log_file,
        "\n[{}] ===== Build started =====",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    )?;

    // Determine the cargo command to run
    let cargo_cmd = if args.is_empty() {
        vec!["build".to_string()]
    } else {
        args
    };

    // Log the command being run
    writeln!(
        log_file,
        "[{}] Running: cargo {}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        cargo_cmd.join(" ")
    )?;

    // Execute cargo with the provided arguments
    let output = Command::new("cargo")
        .args(&cargo_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    // Write stdout to log
    if !output.stdout.is_empty() {
        log_file.write_all(&output.stdout)?;
    }

    // Write stderr to log (this is where errors typically appear)
    if !output.stderr.is_empty() {
        writeln!(
            log_file,
            "[{}] ===== Build Errors =====",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        )?;
        log_file.write_all(&output.stderr)?;
    }

    // Write build completion status
    let status_msg = if output.status.success() {
        format!(
            "[{}] Build completed successfully",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    } else {
        format!(
            "[{}] Build failed with exit code: {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            output.status.code().unwrap_or(-1)
        )
    };
    writeln!(log_file, "{}", status_msg)?;

    // Also print to console
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));

    // Exit with the same status as cargo
    Ok(output.status.code().unwrap_or(1))
}
