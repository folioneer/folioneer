//! The channel on Windows (AGT-021): a named pipe whose access list admits the owner's user
//! alone and which refuses remote clients. Its name is drawn at each opening and kept in a
//! file of the channel folder, the one thing of the data folder the bridge opens.

use std::io::{self, Read, Seek, Write};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::time::Duration;

use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_DATA, ERROR_PIPE_BUSY, HANDLE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

use super::{channel_dir, entry_path};
use crate::core::BACKEND;

/// What every pipe of the channel is named after; a name file that says anything else is
/// not followed.
const PIPE_PREFIX: &str = r"\\.\pipe\folioneer-agent-";
/// How many times the bridge tries a pipe whose application is busy accepting another
/// bridge — or an opening application one that is — and how long it waits between two tries.
const BUSY_TRIES: u32 = 20;
const BUSY_WAIT: Duration = Duration::from_millis(25);
/// How much of the name file is read: more than a name is long.
const NAME_FILE_LIMIT: u64 = 256;

/// The application's end of a connection.
pub type Stream = NamedPipeServer;

/// The user a process runs as, as the system identifies it.
struct UserSid {
    /// A `TOKEN_USER` followed by the identifier it points to; 8-byte words keep the
    /// structure aligned.
    token_user: Vec<u64>,
}

impl UserSid {
    /// The user of the process `process` is a handle of.
    fn of(process: HANDLE) -> io::Result<Self> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: `process` is a process handle the caller holds open, and `token` is a
        // local the call writes a handle to.
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let read = Self::read(token);
        // SAFETY: `token` was opened just above and is closed once, here.
        unsafe { CloseHandle(token) };
        read
    }

    fn read(token: HANDLE) -> io::Result<Self> {
        let mut size = 0u32;
        // SAFETY: a null buffer of size 0 is how the size is asked for; `size` is a local.
        let sized =
            unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size) };
        if sized == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
                return Err(error);
            }
        }
        let mut token_user = vec![0u64; (size as usize).div_ceil(8).max(1)];
        // SAFETY: the buffer is at least `size` bytes long and aligned for a `TOKEN_USER`.
        let filled = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                token_user.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        };
        if filled == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { token_user })
    }

    /// The user this process runs as.
    fn of_this_process() -> io::Result<Self> {
        // SAFETY: the call takes no argument and returns a handle that needs no closing.
        Self::of(unsafe { GetCurrentProcess() })
    }

    /// The user the process `process_id` runs as.
    fn of_process(process_id: u32) -> io::Result<Self> {
        // SAFETY: the call takes plain values; a process that cannot be opened is null.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let user = Self::of(process);
        // SAFETY: `process` was opened just above and is closed once, here.
        unsafe { CloseHandle(process) };
        user
    }

    fn sid(&self) -> PSID {
        // SAFETY: `token_user` was filled by `GetTokenInformation(TokenUser)`, so it starts
        // with a `TOKEN_USER` whose identifier lives in the same buffer, as long as `self`.
        unsafe { (*self.token_user.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn is(&self, other: &Self) -> bool {
        // SAFETY: both identifiers are valid for as long as the two values are borrowed.
        unsafe { EqualSid(self.sid(), other.sid()) != 0 }
    }

    /// The identifier as the access list writes it (`S-1-5-21-…`).
    fn text(&self) -> io::Result<String> {
        let mut wide: *mut u16 = std::ptr::null_mut();
        // SAFETY: the identifier is valid, and `wide` is a local the call writes to.
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut wide) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut length = 0;
        // SAFETY: the call returned a NUL-terminated string, read up to its terminator.
        while unsafe { *wide.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: `length` units were just read from `wide`.
        let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(wide, length) });
        // SAFETY: the system allocated `wide` for the caller to free, once.
        unsafe { LocalFree(wide.cast()) };
        Ok(text)
    }
}

/// AGT-021 — the access list of the pipe: owned by `user`, open to `user` alone, and to
/// nobody by inheritance.
fn access_list(user: &str) -> String {
    format!("O:{user}D:P(A;;GA;;;{user})")
}

/// One instance of the pipe `name`, for one bridge to connect to.
fn instance(name: &str, first: bool) -> io::Result<NamedPipeServer> {
    let access: Vec<u16> = access_list(&UserSid::of_this_process()?.text()?)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `access` is NUL-terminated and outlives the call; `descriptor` is a local.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            access.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    // SAFETY: `attributes` is a valid `SECURITY_ATTRIBUTES` whose descriptor lives until
    // it is freed below, after the pipe copied it.
    let created = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(name, std::ptr::addr_of_mut!(attributes).cast())
    };
    // SAFETY: the system allocated `descriptor` for the caller to free, once.
    unsafe { LocalFree(descriptor.cast()) };
    created
}

/// The pipe `text` names, when it is one of the channel's: the prefix, then letters, digits
/// and dashes.
fn pipe_named(text: &str) -> Option<String> {
    let name = text.trim();
    let drawn = name.strip_prefix(PIPE_PREFIX)?;
    let plain = !drawn.is_empty()
        && drawn
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    plain.then(|| name.to_string())
}

/// What `file` says, up to the longest text a name can be.
fn read_name(file: &mut std::fs::File) -> Option<String> {
    let mut text = String::new();
    file.take(NAME_FILE_LIMIT).read_to_string(&mut text).ok()?;
    pipe_named(&text)
}

/// The application's end of the channel: the instance the next bridge connects to.
#[derive(Debug)]
pub struct Listener {
    name: String,
    waiting: NamedPipeServer,
}

impl Listener {
    /// The next bridge that connects.
    pub async fn accept(&mut self) -> io::Result<Stream> {
        loop {
            match self.waiting.connect().await {
                Ok(()) => break,
                // A bridge that left before it was accepted: the instance waits again.
                Err(error) if error.raw_os_error() == Some(ERROR_NO_DATA as i32) => {
                    self.waiting.disconnect()?;
                }
                Err(error) => return Err(error),
            }
        }
        let next = instance(&self.name, false)?;
        Ok(std::mem::replace(&mut self.waiting, next))
    }
}

/// AGT-021 — opens the channel in `data_dir`: a pipe under a name drawn for this opening,
/// written to a file of the channel folder. A name left by an earlier run is replaced; one
/// a running application still answers on is not.
pub fn listen(data_dir: &Path) -> io::Result<Listener> {
    let dir = channel_dir(data_dir);
    match std::fs::symlink_metadata(&dir) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the channel folder is not a folder of the data folder",
            ));
        }
        Ok(_) => {}
        Err(_) => std::fs::create_dir(&dir)?,
    }
    // Held until the name is written: two applications opening at once are told apart, and
    // the second finds the first one's pipe.
    let mut entry = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(entry_path(data_dir))?;
    entry.lock()?;
    if read_name(&mut entry).is_some_and(|left| answered_by_this_user(&left)) {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "another running application holds the channel",
        ));
    }
    let name = format!("{PIPE_PREFIX}{}", uuid::Uuid::new_v4());
    let waiting = instance(&name, true)?;
    entry.set_len(0)?;
    entry.seek(io::SeekFrom::Start(0))?;
    entry.write_all(name.as_bytes())?;
    Ok(Listener { name, waiting })
}

/// AGT-024 — whether an application of this user still answers on the pipe `name`: it
/// keeps its channel. A pipe nobody answers on, or one another user's program opened under
/// a name left by an earlier run, is not a channel to keep.
fn answered_by_this_user(name: &str) -> bool {
    for _ in 0..BUSY_TRIES {
        match ClientOptions::new().open(name) {
            Ok(pipe) => return served_by_this_user(&pipe),
            // The application is accepting a bridge: it is asked again.
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                std::thread::sleep(BUSY_WAIT);
            }
            Err(_) => return false,
        }
    }
    false
}

/// Whether the program that answers on `pipe` runs as the user this process runs as.
fn served_by_this_user(pipe: &NamedPipeClient) -> bool {
    let mut process_id = 0u32;
    // SAFETY: the handle is the open pipe `pipe` owns; `process_id` is a local.
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle().cast(), &mut process_id) } == 0 {
        return false;
    }
    runs_as_this_user(process_id).unwrap_or(false)
}

/// AGT-021 — whether the program at the other end of `stream` runs as the user this
/// application runs as. A user that cannot be read is written to the log and turned away.
pub fn same_user(stream: &Stream, _data_dir: &Path) -> bool {
    let mut process_id = 0u32;
    // SAFETY: the handle is the open pipe `stream` owns; `process_id` is a local.
    if unsafe { GetNamedPipeClientProcessId(stream.as_raw_handle().cast(), &mut process_id) } == 0 {
        let error = io::Error::last_os_error();
        tracing::warn!(target: BACKEND, err = ?error, "agent channel: the connecting program is unknown");
        return false;
    }
    runs_as_this_user(process_id).unwrap_or_else(|error| {
        tracing::warn!(target: BACKEND, err = ?error, "agent channel: a program's user could not be read");
        false
    })
}

/// Whether the process `process_id` runs as the user this process runs as.
fn runs_as_this_user(process_id: u32) -> io::Result<bool> {
    Ok(UserSid::of_this_process()?.is(&UserSid::of_process(process_id)?))
}

/// The bridge's end: connects to the pipe the file of `data_dir` names; `None` when there
/// is none, when no application answers on it, or when the program that answers runs as
/// another user.
pub async fn connect(data_dir: &Path) -> Option<NamedPipeClient> {
    let name = read_name(&mut std::fs::File::open(entry_path(data_dir)).ok()?)?;
    let mut tries = 0;
    let pipe = loop {
        match ClientOptions::new().open(&name) {
            Ok(pipe) => break pipe,
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) && tries < BUSY_TRIES =>
            {
                tries += 1;
                tokio::time::sleep(BUSY_WAIT).await;
            }
            Err(_) => return None,
        }
    };
    served_by_this_user(&pipe).then_some(pipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    // AGT-021 — the pipe's access list names the owner's user and nobody else, and a
    // program of the same user is recognised at both ends.
    #[tokio::test]
    async fn agt_021_the_channel_is_reachable_by_the_owners_user_only() {
        let user = UserSid::of_this_process()
            .expect("user")
            .text()
            .expect("text");
        assert!(user.starts_with("S-1-"), "{user}");
        assert_eq!(
            access_list("S-1-5-21-7"),
            "O:S-1-5-21-7D:P(A;;GA;;;S-1-5-21-7)"
        );
        let data_dir = tempfile::tempdir().expect("tempdir");

        let mut listener = listen(data_dir.path()).expect("listening");

        let bridge = connect(data_dir.path()).await.expect("connected");
        let application_end = listener.accept().await.expect("accepted");
        assert!(same_user(&application_end, data_dir.path()));
        drop(bridge);
    }

    // AGT-021 — another program of the owner's user is recognised, not only this one.
    #[test]
    fn agt_021_another_program_of_the_same_user_is_recognised() {
        let mut other = std::process::Command::new("cmd")
            .args(["/C", "pause"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("another program");

        let recognised = runs_as_this_user(other.id());

        other.kill().expect("ended");
        other.wait().expect("waited");
        assert!(recognised.expect("its user is read"));
    }

    // AGT-021 — the bridge follows the name file only to a pipe of the channel.
    #[tokio::test]
    async fn agt_021_a_name_that_is_not_the_channels_is_not_followed() {
        let data_dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(channel_dir(data_dir.path())).expect("folder");
        for name in [
            r"\\.\pipe\another",
            r"\\elsewhere\pipe\folioneer-agent-1",
            r"\\.\pipe\folioneer-agent-",
            r"\\.\pipe\folioneer-agent-a\..\b",
            r"C:\Users\someone\file",
        ] {
            std::fs::write(entry_path(data_dir.path()), name).expect("written");
            assert_eq!(pipe_named(name), None, "{name}");
            assert!(connect(data_dir.path()).await.is_none(), "{name}");
        }
    }

    // AGT-022 / AGT-024 — a name left by an earlier run is replaced; a running application
    // keeps its channel; closing leaves nothing to connect to.
    #[tokio::test]
    async fn agt_022_closing_the_channel_leaves_nothing_to_reach() {
        let data_dir = tempfile::tempdir().expect("tempdir");
        drop(listen(data_dir.path()).expect("first run"));
        let left = std::fs::read_to_string(entry_path(data_dir.path())).expect("a name");
        assert!(connect(data_dir.path()).await.is_none(), "nobody listens");
        let _listener = listen(data_dir.path()).expect("second run");
        let drawn = std::fs::read_to_string(entry_path(data_dir.path())).expect("a name");
        assert_eq!(pipe_named(&drawn), Some(drawn.clone()));
        assert_ne!(left, drawn, "a name is drawn at each opening");
        let taken = listen(data_dir.path()).expect_err("a running application keeps its channel");
        assert_eq!(taken.kind(), io::ErrorKind::AddrInUse);

        super::super::close(data_dir.path());
        assert!(!entry_path(data_dir.path()).exists());
        assert!(!channel_dir(data_dir.path()).exists());
        assert!(connect(data_dir.path()).await.is_none());
    }
}
