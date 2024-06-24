use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} PORT ROOT_FOLDER", args[0]);
        return;
    }

    let port = match args[1].parse::<u16>() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Invalid port number");
            return;
        }
    };
    let root_folder = Arc::new(fs::canonicalize(&args[2]).unwrap());

    let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to port {}: {}", port, e);
            return;
        }
    };

    println!("Root folder: {}", root_folder.display());
    println!("Server listening on 0.0.0.0:{}", port);

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to accept incoming connection: {}", e);
                continue;
            }
        };

        let root_folder = Arc::clone(&root_folder);
        thread::spawn(move || {
            handle_connection(stream, &root_folder);
        });
    }
}

fn handle_connection(mut stream: TcpStream, root_folder: &Arc<PathBuf>) {
    let mut buffer = [0; 8192];
    if let Err(e) = stream.read(&mut buffer) {
        eprintln!("Failed to read from connection: {}", e);
        return;
    };

    let request = String::from_utf8_lossy(&buffer[..]);
    let (method, path) = parse_request(&request);

    let response = match method {
        "GET" => handle_get_request(&path, root_folder),
        "POST" => handle_post_request(&request, root_folder),
        _ => format!("HTTP/1.1 405 Method Not Allowed\r\n\r\n"),
    };

    if let Err(e) = stream.write(response.as_bytes()) {
        eprintln!("Failed to write response: {}", e);
    }
    if let Err(e) = stream.flush() {
        eprintln!("Failed to flush stream: {}", e);
    }
}

fn parse_request(request: &str) -> (&str, &str) {
    let lines: Vec<&str> = request.lines().collect();
    if lines.is_empty() {
        return ("", "");
    }

    let first_line = lines[0];
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() != 3 {
        return ("", "");
    }

    (parts[0], parts[1])
}

fn handle_get_request(path: &str, root_folder: &Arc<PathBuf>) -> String {
    let path = root_folder.join(&path[1..]);

    // Check if requested path exists
    if !path.exists() {
        return format!("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
    }

    // Check if requested path is within root_folder
    if !path.starts_with(root_folder) {
        return format!("HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
    }

    // Handle different content types based on file extension
    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("png") => "image/png",
        Some("jpeg") | Some("jpg") => "image/jpeg",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    };

    // Read file contents and generate HTTP response
    match fs::read_to_string(&path) {
        Ok(contents) => {
            format!(
                "HTTP/1.1 200 OK\r\nContent-type: {}\r\nConnection: close\r\n\r\n{}",
                content_type, contents
            )
        }
        Err(_) => {
            format!("HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n")
        }
    }
}

fn handle_post_request(request: &str, root_folder: &Arc<PathBuf>) -> String {
    let lines: Vec<&str> = request.lines().collect();
    let path = lines[0].split_whitespace().nth(1).unwrap();
    let path = root_folder.join(&path[1..]);

    // Check if requested path exists
    if !path.exists() {
        return format!("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
    }

    // Check if requested path is within root_folder/scripts
    if !path.starts_with(root_folder) || !path.starts_with(root_folder.join("scripts")) {
        return format!("HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
    }

    // Execute script and capture output
    match Command::new(&path).stdout(Stdio::piped()).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            format!(
                "HTTP/1.1 200 OK\r\nContent-type: text/plain\r\nConnection: close\r\n\r\n{}",
                stdout
            )
        }
        Err(_) => {
            format!("HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n")
        }
    }
}
