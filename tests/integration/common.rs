/// Integration test harness: manage the UnrealIRCd Docker container lifecycle.
use std::net::TcpStream;
use std::process::Command;
use std::time::{Duration, Instant};

pub const IRC_HOST: &str = "127.0.0.1";
/// Plain IRC port (no TLS).
pub const IRC_PORT: u16 = 6667;
/// IRC-over-TLS port.
pub const IRC_TLS_PORT: u16 = 6697;
pub const TEST_NICK: &str = "mercury_test";
pub const COMPOSE_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/docker/docker-compose.yml");

/// Start the UnrealIRCd container (idempotent — safe to call if already running).
///
/// If the IRC port is already accepting connections, this is a no-op.
pub fn start_ircd() {
    // Fast path: if port is already open, assume IRCd is running
    if TcpStream::connect(format!("{}:{}", IRC_HOST, IRC_PORT)).is_ok() {
        return;
    }

    // Try docker-compose v2 plugin first (`docker compose`), then v1 (`docker-compose`)
    let started = try_docker_compose_up_v2()
        .or_else(|_| try_docker_compose_up_v1())
        .is_ok();

    if !started {
        panic!(
            "Could not start UnrealIRCd container.\n\
             Please start it manually:\n\
             docker-compose -f docker/docker-compose.yml up -d\n\
             or:\n\
             docker compose -f docker/docker-compose.yml up -d"
        );
    }

    wait_for_ircd(Duration::from_secs(60));
}

fn try_docker_compose_up_v2() -> Result<(), String> {
    // Docker Compose v2 plugin syntax: `docker compose -f ... up -d`
    // Note: `-f` must come before `compose` sub-command's own flags; v2 uses:
    //   docker -f ... compose up  OR  docker compose -f ...
    // The standard is: docker compose -f <file> up -d
    let output = Command::new("docker")
        .args(["compose", "-f", COMPOSE_FILE, "up", "-d"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn try_docker_compose_up_v1() -> Result<(), String> {
    // Docker Compose v1 standalone binary: `docker-compose -f ... up -d`
    let output = Command::new("docker-compose")
        .args(["-f", COMPOSE_FILE, "up", "-d"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// Stop and remove the UnrealIRCd container.
#[allow(dead_code)]
pub fn stop_ircd() {
    let _ = Command::new("docker")
        .args(["compose", "-f", COMPOSE_FILE, "down"])
        .output();
    let _ = Command::new("docker-compose")
        .args(["-f", COMPOSE_FILE, "down"])
        .output();
}

/// Block until IRC port is accepting TCP connections, or panic after `timeout`.
pub fn wait_for_ircd(timeout: Duration) {
    let addr = format!("{}:{}", IRC_HOST, IRC_PORT);
    let start = Instant::now();
    loop {
        if TcpStream::connect(&addr).is_ok() {
            // Give IRCd a moment to finish its startup handshake
            std::thread::sleep(Duration::from_millis(500));
            return;
        }
        if start.elapsed() >= timeout {
            panic!(
                "UnrealIRCd did not become ready on {} within {:?}",
                addr, timeout
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
