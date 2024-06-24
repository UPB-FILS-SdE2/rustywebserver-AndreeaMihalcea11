use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::fs;
use std::path::Path;

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let request = String::from_utf8_lossy(&buffer[..]);
    let first_line = request.lines().next().unwrap();
    let path = first_line.split_whitespace().nth(1).unwrap().trim_start_matches('/');

    let mut file_path = Path::new("./public/").join(path);

    if path.contains("..") || !file_path.exists() {
        // Handle forbidden access or file not found
        let response = format!("HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n");
        stream.write(response.as_bytes()).unwrap();
        return;
    }

    if file_path.is_dir() {
        file_path = file_path.join("index.html");
    }

    let content_type = match file_path.extension() {
        Some(ext) => {
            match ext.to_str().unwrap() {
                "html" => "text/html; charset=utf-8",
                "txt" => "text/plain; charset=utf-8",
                "jpeg" | "jpg" => "image/jpeg",
                "png" => "image/png",
                "zip" => "application/zip",
                _ => "application/octet-stream",
            }
        },
        None => "application/octet-stream",
    };

    let content = fs::read(file_path).unwrap();

    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n", content_type, content.len());
    stream.write(response.as_bytes()).unwrap();
    stream.write(&content).unwrap();
    stream.flush().unwrap();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8000").unwrap();
    println!("Server listening on port 8000...");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        std::thread::spawn(|| {
            handle_client(stream);
        });
    }
}
