//! # PasteBEAM Service
//!
//! Reference implementation.
//!
//! ## Logging Notes
//!
//! We are currently logging with just println! for the sake of
//! simplicity.
//!
//! Potentially dangerous data for ANSI Terminals should be formatted
//! with {:?}. Safe human readable strings can be formatted with {}.

use std::env;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};

const DEFAULT_PORT: u16 = 6969;
const DEFAULT_POSTS_ROOT: &str = "./posts";
/// Posts are uploaded into this subdirectory of the posts root and
/// moved out of it once submitted.
const PARTS_DIR: &str = ".parts";
const POST_ID_BYTE_SIZE: usize = 32;

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "pastebeam".to_string());
    let port = match args.next().map(|arg| arg.parse::<u16>()) {
        None => DEFAULT_PORT,
        Some(Ok(port)) => port,
        Some(Err(err)) => {
            eprintln!("Usage: {program} [<port>] [<posts-root>]");
            eprintln!("ERROR: invalid port: {err}");
            return ExitCode::FAILURE;
        }
    };
    let posts_root: Arc<Path> = args
        .next()
        .map_or_else(|| PathBuf::from(DEFAULT_POSTS_ROOT), PathBuf::from)
        .into();

    // Uploads interrupted by a previous shutdown can't be resumed
    let parts_dir = posts_root.join(PARTS_DIR);
    let _ = fs::remove_dir_all(&parts_dir).await;
    if let Err(err) = fs::create_dir_all(&parts_dir).await {
        eprintln!("ERROR: could not create {}: {err}", parts_dir.display());
        return ExitCode::FAILURE;
    }

    let listener = match TcpListener::bind(("0.0.0.0", port)).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("ERROR: could not listen on port {port}: {err}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "INFO: listening on port {port}, storing posts in {}",
        posts_root.display()
    );

    tokio::select! {
        () = accepter(listener, posts_root) => {}
        () = shutdown_signal() => println!("INFO: shutting down"),
    }
    ExitCode::SUCCESS
}

async fn accepter(listener: TcpListener, posts_root: Arc<Path>) {
    loop {
        match listener.accept().await {
            Ok((sock, addr)) => {
                tokio::spawn(handle_connection(sock, addr, Arc::clone(&posts_root)));
            }
            Err(err) => {
                // Most likely out of file descriptors, so give some connections time to close
                println!("ERROR: could not accept a connection: {err}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Resolves on Ctrl+C or SIGTERM. The latter is how containers are
/// stopped, and as PID 1 we would ignore it without a handler.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate()).expect("could not handle SIGTERM");
        tokio::select! {
            _ = terminate.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

async fn handle_connection(mut sock: TcpStream, addr: SocketAddr, posts_root: Arc<Path>) {
    println!("{addr}: connected");
    let (reader, writer) = sock.split();
    let mut session = Session {
        reader: BufReader::new(reader),
        writer,
        addr,
        posts_root: &posts_root,
    };
    match session.run().await {
        Ok(()) => println!("{addr}: exited normally"),
        Err(err) => {
            println!("{addr}: ERROR: exited with reason: {err}");
            let _ = session.writer.write_all(b"500\r\n").await;
        }
    }
}

struct Session<'a> {
    reader: BufReader<ReadHalf<'a>>,
    writer: WriteHalf<'a>,
    addr: SocketAddr,
    posts_root: &'a Path,
}

impl Session<'_> {
    async fn run(&mut self) -> io::Result<()> {
        let addr = self.addr;
        self.writer.write_all(b"HI\r\n").await?;
        let mut command = Vec::new();
        if self.reader.read_until(b'\n', &mut command).await? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        match command.as_slice() {
            b"CRASH\r\n" => Err(io::Error::other("crash")),
            // All the limits are gone, so there are no parameters left to report
            b"PARAMS\r\n" => Ok(()),
            b"POST\r\n" => {
                self.writer.write_all(b"OK\r\n").await?;
                println!("{addr}: wants to make a post");
                self.post().await
            }
            [b'G', b'E', b'T', b' ', id @ ..] => {
                println!("{addr}: wants to get a post");
                self.get(id).await
            }
            _ => {
                println!(
                    "{addr}: ERROR: invalid command: {:?}",
                    String::from_utf8_lossy(&command)
                );
                self.writer.write_all(b"INVALID COMMAND\r\n").await
            }
        }
    }

    async fn post(&mut self) -> io::Result<()> {
        // The post is streamed to disk as it arrives, so its size is not limited by RAM
        let part_path = self.posts_root.join(PARTS_DIR).join(random_post_id()?);
        let result = self.receive_post(&part_path).await;
        // Already gone if the post was submitted
        let _ = fs::remove_file(&part_path).await;
        result
    }

    async fn receive_post(&mut self, part_path: &Path) -> io::Result<()> {
        let addr = self.addr;
        let mut part = BufWriter::new(File::create(part_path).await?);
        let mut size: u64 = 0;
        let mut line = Vec::new();
        loop {
            line.clear();
            if self.reader.read_until(b'\n', &mut line).await? == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            if line == b"SUBMIT\r\n" {
                break;
            }
            if std::str::from_utf8(&line).is_err() {
                println!("{addr}: ERROR: invalid utf8");
                return self.writer.write_all(b"INVALID UTF8\r\n").await;
            }
            if !line.ends_with(b"\r\n") {
                println!("{addr}: ERROR: bad line ending");
                return self.writer.write_all(b"BAD LINE ENDING\r\n").await;
            }
            part.write_all(&line).await?;
            size += line.len() as u64;
            self.writer.write_all(b"OK\r\n").await?;
        }
        part.flush().await?;
        println!("{addr}: submitted the post of size {size} bytes");

        let id = loop {
            let id = random_post_id()?;
            // Very unlikely to be taken, but still
            if !fs::try_exists(self.posts_root.join(&id)).await? {
                break id;
            }
        };
        fs::rename(part_path, self.posts_root.join(&id)).await?;
        println!("{addr}: assigned post id: {id}");
        self.writer
            .write_all(format!("SENT {id}\r\n").as_bytes())
            .await
    }

    async fn get(&mut self, id: &[u8]) -> io::Result<()> {
        let addr = self.addr;
        let trimmed = id.trim_ascii();
        let Some(id) = std::str::from_utf8(trimmed)
            .ok()
            .filter(|id| is_valid_post_id(id))
        else {
            // id is submitted by the user! Always log it with {:?}.
            // Do not let the user know that the id is invalid. It's all "not found" for them.
            println!(
                "{addr}: ERROR: invalid post id: {:?}",
                String::from_utf8_lossy(trimmed)
            );
            return self.writer.write_all(b"404\r\n").await;
        };
        // Safe to log with {} since it's made out of posts_root which we
        // trust and id which is verified with is_valid_post_id().
        let path = self.posts_root.join(id);
        match File::open(&path).await {
            Ok(mut file) => {
                println!("{addr}: sending out post {}", path.display());
                tokio::io::copy(&mut file, &mut self.writer).await?;
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                println!(
                    "{addr}: ERROR: could not read post file {}: doesn't exist",
                    path.display()
                );
                self.writer.write_all(b"404\r\n").await
            }
            Err(err) => Err(err),
        }
    }
}

fn random_post_id() -> io::Result<String> {
    let mut bytes = [0; POST_ID_BYTE_SIZE];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02X}")).collect())
}

fn is_valid_post_id(id: &str) -> bool {
    id.len() == POST_ID_BYTE_SIZE * 2 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'A'..=b'F'))
}

// TODO: Support deleting posts.
// TODO: Maybe post ids should be uuids?
// TODO: Should we support TLS connections?
