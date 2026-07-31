use std::{os::unix::net::{UnixListener, UnixStream}, path::{Path, PathBuf}};

use adi2::ipc::{IPCError, IPCStream, Request, Response};

#[derive(Debug)]
pub enum ServerError {
    RemovingStaleSocketFailed {
        socket_path: PathBuf,
        cause: std::io::Error,
    },
    IPCError(IPCError),
    OtherIOError(std::io::Error),
    ServerAlreadyRunning {
        socket_path: PathBuf,
    },
}

impl From<IPCError> for ServerError {
    fn from(e: IPCError) -> Self {
        ServerError::IPCError(e)
    }
}

impl From<std::io::Error> for ServerError {
    fn from(e: std::io::Error) -> Self {
        ServerError::OtherIOError(e)
    }
}

pub struct Server {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl Server {
    pub fn bind<P: Into<PathBuf>>(socket_path: P) -> Result<Self, ServerError> {
        let socket_path = socket_path.into();
        remove_stale_socket(&socket_path)?;
        let listener = UnixListener::bind(&socket_path)?;
        Ok(Server {
            listener,
            socket_path,
        })
    }

    pub fn start<F>(&mut self, mut f: F) -> Result<(), ServerError>
        where F: FnMut(Request) -> Response
    {
        for stream in self.listener.incoming() {
            let mut ipc_stream = IPCStream::new(stream?);
            let request = ipc_stream.read_message()?;
            println!("REQ: {:?}", request);
            let response = f(request);
            ipc_stream.send_message(response)?;
        }
        Ok(())
    }
}

fn remove_stale_socket(socket_path: &Path) -> Result<(), ServerError> {
    if !socket_path.try_exists()? {
        return Ok(());
    }

    match UnixStream::connect(socket_path) {
        Ok(_) => Err(ServerError::ServerAlreadyRunning { socket_path: socket_path.into() }),
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            Ok(std::fs::remove_file(socket_path).map_err(|e| {
                ServerError::RemovingStaleSocketFailed {
                    socket_path: socket_path.into(),
                    cause: e,
                }
            })?)
        }
        Err(e) => Err(e.into()),
    }
}
