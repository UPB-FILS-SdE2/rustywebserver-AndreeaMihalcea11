use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} PORT ROOT_FOLDER", args[0]);
        return;
    }

    let port = &args[1];
    let root_folder = &args[2];
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).unwrap();
    let root_folder = Arc::new(fs::canonicalize(root_folder).unwrap());

    println!("Root folder: {}", root_folder.display());
    println!("Server listening on 0.0.0.0:{}", port);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let root_folder = Arc::clone(&root_folder);
        thread::spawn(move || {
            handle_connection(stream, &root_folder);
        });
    }
}

fn handle_connection(mut stream: TcpStream, root_folder: &Path) {
    let mut buffer = [0; 8192];
    let _ = stream.read(&mut buffer);

    let request = String::from_utf8_lossy(&buffer[..]);
    let (method, path) = parse_request(&request);

    let response = if method == "GET" {
        handle_get_request(&path, root_folder)
    } else if method == "POST" {
        handle_post_request(&request, root_folder)
    } else {
        format!("HTTP/1.1 405 Method Not Allowed\r\n\r\n")
    };

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
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

fn handle_get_request(path: &str, root_folder: &Path) -> String {
    let path = root_folder.join(&path[1..]);
    if !path.exists() {
        return format!("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
    }

    if !path.starts_with(root_folder) {
        return format!("HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
    }

    let contents = fs::read_to_string(&path).unwrap_or_else(|_| {
        return format!("HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n");
    });

    let content_type = if path.extension().is_some() {
        match path.extension().unwrap().to_str().unwrap() {
            "html" => "text/html; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "png" => "image/png",
            "jpeg" | "jpg" => "image/jpeg",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
    } else {
        "application/octet-stream"
    };

    format!(
        "HTTP/1.1 200 OK\r\nContent-type: {}\r\nConnection: close\r\n\r\n{}",
        content_type, contents
    )
}

fn handle_post_request(request: &str, root_folder: &Path) -> String {
    let lines: Vec<&str> = request.lines().collect();
    let path = lines[0].split_whitespace().nth(1).unwrap();
    let path = root_folder.join(&path[1..]);
    if !path.exists() {
        return format!("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
    }

    if !path.starts_with(root_folder) || !path.starts_with(root_folder.join("scripts")) {
        return format!("HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
    }

    let output = Command::new(path)
        .envs(parse_headers_as_env_vars(request))
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}",
                    String::from_utf8_lossy(&output.stdout)
                )
            } else {
                format!(
                    "HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n{}",
                    String::from_utf8_lossy(&output.stderr)
                )
            }
        }
        Err(_) => format!("HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n"),
    }
}

fn parse_headers_as_env_vars(request: &str) -> Vec<(&str, &str)> {
    request
        .lines()
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            (parts[0].trim(), parts[1].trim())
        })
        .collect()
}
