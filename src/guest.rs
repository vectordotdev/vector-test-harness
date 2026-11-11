//! Dependency-free helpers, also compiled directly with rustc on benchmark VMs.
use std::{
    fs::File,
    io::{self, Write},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn socat_loop(args: &[String]) -> io::Result<i32> {
    if args.len() < 3 {
        return Err(invalid(
            "usage: vth-guest socat-loop SECONDS FILE SOCAT_ARGS...",
        ));
    }
    let seconds: f64 = args[0].parse().map_err(|_| invalid("invalid duration"))?;
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 86_400.0 {
        return Err(invalid("duration must be between 0 and 86400 seconds"));
    }
    let until = Instant::now() + Duration::from_secs_f64(seconds);
    loop {
        if Instant::now() >= until {
            return Ok(124);
        }
        let input = File::open(&args[1])?;
        let mut child = Running(
            Command::new("socat")
                .args(&args[2..])
                .stdin(input)
                .spawn()?,
        );
        loop {
            if Instant::now() >= until {
                return Ok(124);
            }
            if let Some(status) = child.0.try_wait()? {
                if !status.success() {
                    return Ok(status.code().unwrap_or(1));
                }
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

fn longlog() -> io::Result<i32> {
    // Match the original Ruby fixture's keys and values, including insertion order.
    let mut message = String::from("{\"message\":\"Log text\"");
    for first in b'a'..=b'b' {
        for second in b'a'..=b'z' {
            for third in b'a'..=b'z' {
                let key = format!("{}{}{}", first as char, second as char, third as char);
                message.push_str(&format!(",\"{key}\":\"{key}_value\""));
            }
        }
    }
    message.push('}');
    if message.len() < 16 * 1024 {
        return Err(invalid(
            "long-log fixture must exceed Docker's splitting threshold",
        ));
    }
    writeln!(io::stdout().lock(), "{message}")?;
    Ok(0)
}

fn parallel(args: &[String]) -> io::Result<i32> {
    let mut children = vec![];
    // Separate argv vectors with :::, so shell evaluation is unnecessary.
    for args in args.split(|arg| arg == ":::") {
        let Some((program, args)) = args.split_first() else {
            return Err(invalid(
                "parallel expects COMMAND [ARGS...] ::: COMMAND [ARGS...]",
            ));
        };
        children.push(Running(
            Command::new(program)
                .args(args)
                .stdin(Stdio::null())
                .spawn()?,
        ));
    }
    let mut code = 0;
    for child in &mut children {
        let status = child.0.wait()?;
        if !status.success() {
            code = status.code().unwrap_or(1);
        }
    }
    Ok(code)
}

pub fn run(args: &[String]) -> io::Result<i32> {
    match args.first().map(String::as_str) {
        Some("socat-loop") => socat_loop(&args[1..]),
        Some("longlog") => longlog(),
        Some("parallel") => parallel(&args[1..]),
        None | Some("--help" | "-h") => {
            println!("vth-guest socat-loop SECONDS FILE SOCAT_ARGS...\nvth-guest longlog\nvth-guest parallel COMMAND [ARGS...] ::: COMMAND [ARGS...]");
            Ok(0)
        }
        _ => Err(invalid("unknown guest helper; use --help")),
    }
}

#[allow(dead_code)]
fn main() {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
