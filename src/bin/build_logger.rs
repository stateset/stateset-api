use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::{Command, Stdio};
use chrono::Local;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    
    // Open or create the build log file
    let mut log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("build_errors.log")
        .expect("Failed to open build_errors.log");

    // Write build start timestamp
    writeln!(
        log_file,
        "\n[{}] ===== Build started =====",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ).unwrap();

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
    ).unwrap();

    // Execute cargo with the provided arguments
    let child = Command::new("cargo")
        .args(&cargo_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute cargo");

    // Capture and log output
    let output = child.wait_with_output().expect("Failed to wait on cargo");
    
    // Write stdout to log
    if !output.stdout.is_empty() {
        log_file.write_all(&output.stdout).unwrap();
    }
    
    // Write stderr to log (this is where errors typically appear)
    if !output.stderr.is_empty() {
        writeln!(
            log_file,
            "[{}] ===== Build Errors =====",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ).unwrap();
        log_file.write_all(&output.stderr).unwrap();
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
    writeln!(log_file, "{}", status_msg).unwrap();
    
    // Also print to console
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    
    // Exit with the same status as cargo
    std::process::exit(output.status.code().unwrap_or(1));
} 