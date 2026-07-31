use std::{os::unix::net::{UnixListener, UnixStream}, path::{Path, PathBuf}};

use adi2::ipc::{IPCError, IPCStream, Request, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("failed to remove stale socket at {socket_path:?}: {cause:?}")]
    RemovingStaleSocketFailed {
        socket_path: PathBuf,
        cause: std::io::Error,
    },
    #[error("ipc error: {0}")]
    IPCError(#[from] IPCError),
    #[error("io error: {0}")]
    OtherIOError(#[from] std::io::Error),
    #[error("server is already running at {socket_path:?}")]
    ServerAlreadyRunning {
        socket_path: PathBuf,
    },
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
