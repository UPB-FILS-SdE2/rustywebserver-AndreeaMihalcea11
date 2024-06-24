use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

const DEFAULT_PORT: &str = "8000";

fn main() {
    // Verifica argumentele liniei de comandă
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: rustwebserver <PORT> <ROOT_FOLDER>");
        std::process::exit(1);
    }

    let port = args[1].clone();
    let root_folder = Arc::new(PathBuf::from(args[2].clone()));

    // Creează un listener TCP pe adresa 0.0.0.0:PORT
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).expect("Failed to bind to port");

    println!("Root folder: {:?}", root_folder);
    println!("Server listening on 0.0.0.0:{}", port);

    // Așteaptă conexiuni, acceptă fiecare conexiune într-un fir de execuție nou
    for stream in listener.incoming() {
        let root_folder = Arc::clone(&root_folder);
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream, &root_folder);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream, root_folder: &PathBuf) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    // Parsează cererea HTTP
    let request = String::from_utf8_lossy(&buffer[..]);
    let mut lines = request.lines();

    if let Some(first_line) = lines.next() {
        let mut parts = first_line.split_whitespace();
        let request_type = parts.next().unwrap();
        let path = parts.next().unwrap();
        let http_version = parts.next().unwrap();

        // Parsează header-ele
        let mut headers = HashMap::new();
        while let Some(line) = lines.next() {
            if line.is_empty() {
                break;
            }
            let mut header_parts = line.splitn(2, ':');
            let key = header_parts.next().unwrap().trim();
            let value = header_parts.next().unwrap().trim();
            headers.insert(key.to_lowercase(), value.to_string());
        }

        // Verifică tipul cererii HTTP
        match request_type {
            "GET" => {
                // Găsește calea absolută a fișierului cerut
                let file_path = root_folder.join(&path[1..]);
                let response = if let Some(file_name) = file_path.file_name() {
                    if file_name == "" || path.contains("..") {
                        create_response(403, "Forbidden", "text/plain; charset=utf-8", Vec::new())
                    } else if file_path.exists() {
                        if file_path.is_file() {
                            // Încarcă și returnează fișierul
                            match fs::read(&file_path) {
                                Ok(contents) => {
                                    let content_type = get_content_type(&file_path);
                                    create_response(200, "OK", &content_type, contents)
                                }
                                Err(_) => create_response(500, "Internal Server Error", "text/plain; charset=utf-8", Vec::new()),
                            }
                        } else {
                            // Calea este un director
                            let index_file = file_path.join("index.html");
                            if index_file.exists() && index_file.is_file() {
                                match fs::read(&index_file) {
                                    Ok(contents) => {
                                        let content_type = get_content_type(&index_file);
                                        create_response(200, "OK", &content_type, contents)
                                    }
                                    Err(_) => create_response(500, "Internal Server Error", "text/plain; charset=utf-8", Vec::new()),
                                }
                            } else {
                                create_response(403, "Forbidden", "text/plain; charset=utf-8", Vec::new())
                            }
                        }
                    } else {
                        create_response(404, "Not Found", "text/plain; charset=utf-8", Vec::new())
                    }
                } else {
                    create_response(403, "Forbidden", "text/plain; charset=utf-8", Vec::new())
                };

                if let Err(e) = stream.write_all(&response) {
                    eprintln!("Error writing to stream: {}", e);
                }
            }
            "POST" => {
                // Verifică dacă este o solicitare POST validă pentru execuția unui script
                if path.starts_with("/scripts/") {
                    let script_path = root_folder.join(&path[1..]);
                    if script_path.exists() && script_path.is_file() {
                        let script_name = script_path.file_name().unwrap().to_string_lossy().to_string();
                        let script_args = vec![script_name.clone()];
                        let script_output = execute_script(&script_path, &script_args, &headers);

                        if let Err(e) = stream.write_all(&script_output) {
                            eprintln!("Error writing script output to stream: {}", e);
                        }
                    } else {
                        let response = create_response(404, "Not Found", "text/plain; charset=utf-8", Vec::new());
                        if let Err(e) = stream.write_all(&response) {
                            eprintln!("Error writing to stream: {}", e);
                        }
                    }
                } else {
                    let response = create_response(403, "Forbidden", "text/plain; charset=utf-8", Vec::new());
                    if let Err(e) = stream.write_all(&response) {
                        eprintln!("Error writing to stream: {}", e);
                    }
                }
            }
            _ => {
                // Alte tipuri de cereri nu sunt acceptate în acest server
                eprintln!("Unsupported request type: {}", request_type);
            }
        }
    }
}

fn create_response(status_code: u16, status_text: &str, content_type: &str, body: Vec<u8>) -> Vec<u8> {
    let mut response = format!("HTTP/1.1 {} {}\r\n", status_code, status_text).into_bytes();
    response.extend(format!("Content-Type: {}\r\n", content_type).as_bytes());
    response.extend(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    response.extend(b"Connection: close\r\n\r\n");
    response.extend(body);
    response
}

fn get_content_type(file_path: &Path) -> String {
    match file_path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8".to_string(),
        Some("txt") => "text/plain; charset=utf-8".to_string(),
        Some("css") => "text/css; charset=utf-8".to_string(),
        Some("js") => "text/javascript; charset=utf-8".to_string(),
        Some("jpeg") | Some("jpg") => "image/jpeg".to_string(),
        Some("png") => "image/png".to_string(),
        Some("zip") => "application/zip".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

fn execute_script(script_path: &Path, args: &[String], headers: &HashMap<String, String>) -> Vec<u8> {
    let mut command = Command::new(&script_path);
    command.args(args)
           .env_clear()
           .envs(headers.iter().map(|(k, v)| (format!("HTTP_{}", k.to_uppercase()), v)))
           .env("Method", "POST")
           .env("Path", script_path.to_string_lossy());

    if let Ok(output) = command.output() {
        if output.status.success() {
            return output.stdout;
        } else {
            let mut error_response = create_response(500, "Internal Server Error", "text/plain; charset=utf-8", Vec::new());
            error_response.extend(output.stderr);
            return error_response;
        }
    } else {
        return create_response(500, "Internal Server Error", "text/plain; charset=utf-8", Vec::new());
    }
}
