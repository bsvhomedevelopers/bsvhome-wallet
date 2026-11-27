use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, Read, Write},
    os::fd::AsRawFd,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use libc::{flock, LOCK_EX, LOCK_NB, LOCK_UN};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

use crate::{backup, participant};

const MAX_MESSAGE_BYTES: usize = 8_000_000;
const REPLY_TIMEOUT: Duration = Duration::from_secs(180);
type NativeResult<T> = Result<T, String>;

struct Input {
    messages: Receiver<Zeroizing<Vec<u8>>>,
    current: Zeroizing<Vec<u8>>,
    position: usize,
}

impl Read for Input {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        let length = available.len().min(buffer.len());
        buffer[..length].copy_from_slice(&available[..length]);
        self.consume(length);
        Ok(length)
    }
}

impl BufRead for Input {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.position == self.current.len() {
            self.current.zeroize();
            self.position = 0;
            self.current = match self.messages.recv() {
                Ok(message) => message,
                Err(_) => return Ok(&[]),
            };
        }
        Ok(&self.current[self.position..])
    }

    fn consume(&mut self, length: usize) {
        self.position = (self.position + length).min(self.current.len());
    }
}

struct Output {
    messages: Sender<Zeroizing<String>>,
    current: Zeroizing<Vec<u8>>,
}

impl Write for Output {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.current.len() + buffer.len() > MAX_MESSAGE_BYTES {
            return Err(io::Error::other("MESSAGE_TOO_LARGE"));
        }
        self.current.extend_from_slice(buffer);
        if self.current.last() == Some(&b'\n') {
            let response = String::from_utf8(self.current.to_vec())
                .map_err(|_| io::Error::other("ENCODING_INVALID"))?;
            self.current.zeroize();
            self.current.clear();
            self.messages
                .send(Zeroizing::new(response))
                .map_err(|_| io::Error::other("CHANNEL_CLOSED"))?;
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct Participant {
    commands: Option<Sender<Zeroizing<Vec<u8>>>>,
    responses: Receiver<Zeroizing<String>>,
    worker: Option<JoinHandle<()>>,
    _exclusive_lock: File,
}

impl Participant {
    pub(crate) fn open(
        directory: PathBuf,
        wallet_id: String,
        key: Zeroizing<[u8; 32]>,
    ) -> NativeResult<Self> {
        if wallet_id.is_empty()
            || wallet_id.len() > 64
            || !wallet_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("WALLET_INVALID".into());
        }
        std::fs::create_dir_all(&directory).map_err(|_| "STORAGE_ERROR")?;
        let exclusive_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("participant.lock"))
            .map_err(|_| "STORAGE_ERROR")?;
        lock_file(&exclusive_lock)?;
        let (commands, incoming) = mpsc::channel();
        let (outgoing, responses) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("wowpay-mpc-a".into())
            .spawn(move || {
                let mut input = Input {
                    messages: incoming,
                    current: Zeroizing::new(Vec::new()),
                    position: 0,
                };
                let mut output = Output {
                    messages: outgoing,
                    current: Zeroizing::new(Vec::new()),
                };
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    participant::run(directory, wallet_id, key, &mut input, &mut output)
                }))
                .map_err(|_| "NATIVE_PANIC".to_string())
                .and_then(|result| result);
                if let Err(code) = outcome {
                    let _ = serde_json::to_writer(&mut output, &serde_json::json!({"error":code}));
                    let _ = output.write_all(b"\n");
                }
            })
            .map_err(|_| "NATIVE_THREAD_UNAVAILABLE")?;
        let participant = Self {
            commands: Some(commands),
            responses,
            worker: Some(worker),
            _exclusive_lock: exclusive_lock,
        };
        let ready: Value = serde_json::from_str(&participant.receive()?)
            .map_err(|_| "NATIVE_HANDSHAKE_INVALID")?;
        if let Some(code) = ready["error"].as_str() {
            return Err(code.into());
        }
        if ready["ready"] != true
            || ready["role"] != 1
            || ready["protocol"] != backup::PROTOCOL
            || ready["core_commit"] != "c9c407eae3447bcbfb539d77bb910ceb2d3a005e"
            || ready["api_version"] != 1
        {
            return Err("NATIVE_HANDSHAKE_INVALID".into());
        }
        Ok(participant)
    }

    pub(crate) fn exchange(&mut self, command: &[u8]) -> NativeResult<Zeroizing<String>> {
        if command.len() > MAX_MESSAGE_BYTES || command.contains(&b'\n') || command.contains(&b'\r')
        {
            return Err("MESSAGE_INVALID".into());
        }
        let _: Value = serde_json::from_slice(command).map_err(|_| "INVALID_MESSAGE")?;
        let mut message = Zeroizing::new(command.to_vec());
        message.push(b'\n');
        self.commands
            .as_ref()
            .ok_or("CHANNEL_CLOSED")?
            .send(message)
            .map_err(|_| "CHANNEL_CLOSED")?;
        match self.receive() {
            Ok(response) => Ok(response),
            Err(error) => {
                self.commands.take();
                Err(error)
            }
        }
    }

    fn receive(&self) -> NativeResult<Zeroizing<String>> {
        self.responses
            .recv_timeout(REPLY_TIMEOUT)
            .map_err(|_| "NATIVE_CHANNEL_UNAVAILABLE".into())
    }
}

impl Drop for Participant {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        unlock_file(&self._exclusive_lock);
    }
}

fn lock_file(file: &File) -> NativeResult<()> {
    if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
        return Ok(());
    }
    match io::Error::last_os_error().kind() {
        io::ErrorKind::WouldBlock => Err("MPC_PARTICIPANT_ALREADY_OPEN".into()),
        _ => Err("MPC_LOCK_UNAVAILABLE".into()),
    }
}

fn unlock_file(file: &File) {
    unsafe {
        flock(file.as_raw_fd(), LOCK_UN);
    }
}
