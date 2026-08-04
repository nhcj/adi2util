use std::{os::unix::net::{UnixListener, UnixStream}, path::{Path, PathBuf}};

use adi2::ipc::{IPCError, IPCStream, Request, Response};
use log::{error, info};
use thiserror::Error;

use crate::midi_client::MidiClientError;

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

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("state does not have active output")]
    ActiveOutputUnset,
    #[error("error with midi client: {0}")]
    MidiClientError(#[from] MidiClientError),
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

    pub fn start<F>(&mut self, mut f: F)
        where F: FnMut(Request) -> Result<Response, HandlerError>
    {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => { 
                    if let Err(e) = Self::handle(IPCStream::new(stream), &mut f) {
                        error!("Error with stream: {:?}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to accept connection: {:?}", e);
                }
            }
        }
    }

    fn handle<F>(mut stream: IPCStream, mut f: F) -> Result<(), ServerError>
        where F: FnMut(Request) -> Result<Response, HandlerError>
    {
        let request = stream.read_message()?;
        info!("Received request: {:?}", request);
        let response = f(request).unwrap_or_else(|e| {
            error!("Error handling request: {:?}", e);
            Response::Err(e.to_string())
        });
        Ok(stream.send_message(response)?)
    }
}

fn remove_stale_socket(socket_path: &Path) -> Result<(), ServerError> {
    if !socket_path.try_exists()? {
        return Ok(());
    }

    match UnixStream::connect(socket_path) {
        Ok(_) => Err(ServerError::ServerAlreadyRunning { socket_path: socket_path.into() }),
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            info!("Removing stale socket at {}", socket_path.display());
            Ok(std::fs::remove_file(socket_path).map_err(|e| {
                error!("Failed to remove stale socket at {}: {:?}", socket_path.display(), e);
                ServerError::RemovingStaleSocketFailed {
                    socket_path: socket_path.into(),
                    cause: e,
                }
            })?)
        }
        Err(e) => Err(e.into()),
    }
}
