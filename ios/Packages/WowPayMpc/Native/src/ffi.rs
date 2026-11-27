use std::{
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    ptr, slice, str,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use zeroize::Zeroizing;

use crate::transport::Participant;

type NativeResult<T> = Result<T, String>;
type Participants = BTreeMap<u64, Arc<Mutex<Option<Participant>>>>;
type Buffers = BTreeMap<u64, Zeroizing<Vec<u8>>>;

static PARTICIPANTS: OnceLock<Mutex<Participants>> = OnceLock::new();
static BUFFERS: OnceLock<Mutex<Buffers>> = OnceLock::new();
static NEXT_IDENTIFIER: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
struct MpcResult {
    status: i32,
    handle: u64,
    buffer_id: u64,
    data: *const u8,
    length: usize,
}

impl MpcResult {
    fn success(handle: u64) -> Self {
        Self {
            status: 0,
            handle,
            buffer_id: 0,
            data: ptr::null(),
            length: 0,
        }
    }

    fn bytes(bytes: Zeroizing<Vec<u8>>, status: i32) -> Self {
        let result = (|| -> NativeResult<Self> {
            let buffer_id = next_identifier()?;
            let data = bytes.as_ptr();
            let length = bytes.len();
            buffers()
                .lock()
                .map_err(|_| "NATIVE_STATE_UNAVAILABLE")?
                .insert(buffer_id, bytes);
            Ok(Self {
                status,
                handle: 0,
                buffer_id,
                data,
                length,
            })
        })();
        match result {
            Ok(result) => result,
            Err(_) => Self {
                status: 2,
                handle: 0,
                buffer_id: 0,
                data: ptr::null(),
                length: 0,
            },
        }
    }

    fn failure(code: String) -> Self {
        Self::bytes(Zeroizing::new(code.into_bytes()), 1)
    }
}

fn participants() -> &'static Mutex<Participants> {
    PARTICIPANTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn buffers() -> &'static Mutex<Buffers> {
    BUFFERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_identifier() -> NativeResult<u64> {
    NEXT_IDENTIFIER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| "NATIVE_CAPACITY_EXHAUSTED".into())
}

fn boundary(action: impl FnOnce() -> NativeResult<MpcResult>) -> MpcResult {
    match catch_unwind(AssertUnwindSafe(action)) {
        Ok(Ok(result)) => result,
        Ok(Err(code)) => MpcResult::failure(code),
        Err(_) => MpcResult::failure("NATIVE_PANIC".into()),
    }
}

unsafe fn input<'a>(data: *const u8, length: usize, maximum: usize) -> NativeResult<&'a [u8]> {
    if data.is_null() || length == 0 || length > maximum {
        return Err("NATIVE_INPUT_INVALID".into());
    }
    Ok(unsafe { slice::from_raw_parts(data, length) })
}

#[unsafe(no_mangle)]
extern "C" fn wowpay_mpc_api_version() -> u32 {
    1
}

#[unsafe(no_mangle)]
unsafe extern "C" fn wowpay_mpc_open(
    directory: *const u8,
    directory_length: usize,
    wallet_id: *const u8,
    wallet_id_length: usize,
    seal_key: *const u8,
    seal_key_length: usize,
) -> MpcResult {
    boundary(|| {
        let directory = str::from_utf8(unsafe { input(directory, directory_length, 4096)? })
            .map_err(|_| "STORAGE_PATH_INVALID")?;
        if directory.contains('\0') {
            return Err("STORAGE_PATH_INVALID".into());
        }
        let wallet_id = str::from_utf8(unsafe { input(wallet_id, wallet_id_length, 64)? })
            .map_err(|_| "WALLET_INVALID")?;
        let bytes = unsafe { input(seal_key, seal_key_length, 32)? };
        let key: [u8; 32] = bytes.try_into().map_err(|_| "MPC_SEAL_KEY_INVALID")?;
        let participant = Participant::open(
            PathBuf::from(directory),
            wallet_id.to_string(),
            Zeroizing::new(key),
        )?;
        let handle = next_identifier()?;
        participants()
            .lock()
            .map_err(|_| "NATIVE_STATE_UNAVAILABLE")?
            .insert(handle, Arc::new(Mutex::new(Some(participant))));
        Ok(MpcResult::success(handle))
    })
}

#[unsafe(no_mangle)]
unsafe extern "C" fn wowpay_mpc_exchange(
    handle: u64,
    command: *const u8,
    command_length: usize,
) -> MpcResult {
    boundary(|| {
        let command = unsafe { input(command, command_length, 8_000_000)? };
        let participant = participants()
            .lock()
            .map_err(|_| "NATIVE_STATE_UNAVAILABLE")?
            .get(&handle)
            .cloned()
            .ok_or("NATIVE_HANDLE_INVALID")?;
        let mut participant = participant.lock().map_err(|_| "NATIVE_STATE_UNAVAILABLE")?;
        let response = participant
            .as_mut()
            .ok_or("NATIVE_HANDLE_INVALID")?
            .exchange(command)?;
        Ok(MpcResult::bytes(
            Zeroizing::new(response.as_bytes().to_vec()),
            0,
        ))
    })
}

#[unsafe(no_mangle)]
extern "C" fn wowpay_mpc_close(handle: u64) -> MpcResult {
    boundary(|| {
        let participant = participants()
            .lock()
            .map_err(|_| "NATIVE_STATE_UNAVAILABLE")?
            .remove(&handle);
        if let Some(participant) = participant {
            participant
                .lock()
                .map_err(|_| "NATIVE_STATE_UNAVAILABLE")?
                .take();
        }
        Ok(MpcResult::success(0))
    })
}

#[unsafe(no_mangle)]
extern "C" fn wowpay_mpc_buffer_free(buffer_id: u64) -> i32 {
    match catch_unwind(|| {
        buffers()
            .lock()
            .map_err(|_| ())?
            .remove(&buffer_id)
            .ok_or(())?;
        Ok::<(), ()>(())
    }) {
        Ok(Ok(())) => 0,
        Ok(Err(())) | Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::random;

    #[test]
    fn test_ffi_rejects_invalid_inputs_and_owns_buffers_until_free() {
        let result = unsafe { wowpay_mpc_open(ptr::null(), 0, ptr::null(), 0, ptr::null(), 0) };
        assert_eq!(result.status, 1);
        assert_eq!(
            unsafe { str::from_utf8(slice::from_raw_parts(result.data, result.length)).unwrap() },
            "NATIVE_INPUT_INVALID"
        );
        assert_eq!(wowpay_mpc_buffer_free(result.buffer_id), 0);
        assert_eq!(wowpay_mpc_buffer_free(result.buffer_id), 1);
    }

    #[test]
    fn test_ffi_close_invalidates_handle_and_releases_wallet_lock() {
        let directory = tempfile::tempdir().unwrap();
        let directory = directory.path().to_str().unwrap().as_bytes();
        let wallet = b"test-wallet";
        let key = random::<32>().unwrap();
        let open = || unsafe {
            wowpay_mpc_open(
                directory.as_ptr(),
                directory.len(),
                wallet.as_ptr(),
                wallet.len(),
                key.as_ptr(),
                key.len(),
            )
        };
        let result = open();
        assert_eq!(result.status, 0);
        let other = open();
        assert_eq!(other.status, 1);
        assert_eq!(
            unsafe { str::from_utf8(slice::from_raw_parts(other.data, other.length)).unwrap() },
            "MPC_PARTICIPANT_ALREADY_OPEN"
        );
        assert_eq!(wowpay_mpc_buffer_free(other.buffer_id), 0);
        assert_eq!(wowpay_mpc_close(result.handle).status, 0);
        assert_eq!(wowpay_mpc_close(result.handle).status, 0);
        let response = unsafe { wowpay_mpc_exchange(result.handle, b"{}".as_ptr(), 2) };
        assert_eq!(response.status, 1);
        assert_eq!(
            unsafe {
                str::from_utf8(slice::from_raw_parts(response.data, response.length)).unwrap()
            },
            "NATIVE_HANDLE_INVALID"
        );
        assert_eq!(wowpay_mpc_buffer_free(response.buffer_id), 0);
        let reopened = open();
        assert_eq!(reopened.status, 0);
        assert_eq!(wowpay_mpc_close(reopened.handle).status, 0);
    }
}
