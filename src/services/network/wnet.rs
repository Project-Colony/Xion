//! SMB discovery through the Win32 WNet API.
//!
//! All the `unsafe` in this service lives here, each block with its own
//! SAFETY note.

// ── WinAPI discovery (WNetEnumResource) ─────────────────────────────────────

/// Entries the first `WNetEnumResourceW` buffer can hold. The API packs the
/// structs at the front and their strings at the back of the same block, so this
/// is a starting point, not a hard limit — see [`enum_resources_step`].
#[cfg(windows)]
pub(super) const INITIAL_ENUM_ENTRIES: usize = 256;

/// Refuse to grow the enumeration buffer past this, so a misbehaving provider
/// cannot drive an unbounded allocation.
#[cfg(windows)]
pub(super) const MAX_ENUM_ENTRIES: usize = 8192;

/// Owns an enumeration handle so every exit path closes it exactly once.
#[cfg(windows)]
pub(super) struct EnumHandle(*mut std::ffi::c_void);

#[cfg(windows)]
impl Drop for EnumHandle {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::NO_ERROR;
        use windows_sys::Win32::NetworkManagement::WNet::WNetCloseEnum;

        // SAFETY: `self.0` came from a successful `WNetOpenEnumW`, is not null,
        // and this is the only place that closes it — the struct is not Copy and
        // Drop runs once.
        let ret = unsafe { WNetCloseEnum(self.0) };
        if ret != NO_ERROR {
            tracing::debug!("Réseau: WNetCloseEnum échoué: code {ret}");
        }
    }
}

#[cfg(windows)]
pub(super) fn zeroed_netresource() -> windows_sys::Win32::NetworkManagement::WNet::NETRESOURCEW {
    windows_sys::Win32::NetworkManagement::WNet::NETRESOURCEW {
        dwScope: 0,
        dwType: 0,
        dwDisplayType: 0,
        dwUsage: 0,
        lpLocalName: std::ptr::null_mut(),
        lpRemoteName: std::ptr::null_mut(),
        lpComment: std::ptr::null_mut(),
        lpProvider: std::ptr::null_mut(),
    }
}

#[cfg(windows)]
pub(super) enum EnumStep {
    /// Number of entries written at the front of the buffer.
    Entries(usize),
    Done,
    Failed(String),
}

/// One `WNetEnumResourceW` round, growing the buffer when the API says it is too
/// small for a single entry.
///
/// The buffer is a `Vec<NETRESOURCEW>` rather than a `Vec<u8>`: reinterpreting a
/// byte vector as an array of structs holding four pointers violated the
/// alignment contract of `slice::from_raw_parts`. As an element vector it is
/// aligned by construction and the filled prefix is a plain safe slice.
/// `ERROR_MORE_DATA` also used to abort the whole enumeration instead of
/// retrying, silently hiding everything past the first 16 KiB.
#[cfg(windows)]
pub(super) fn enum_resources_step(
    handle: &EnumHandle,
    buffer: &mut Vec<windows_sys::Win32::NetworkManagement::WNet::NETRESOURCEW>,
) -> EnumStep {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::WNet::{NETRESOURCEW, WNetEnumResourceW};

    let entry_size = std::mem::size_of::<NETRESOURCEW>();

    loop {
        let mut count: u32 = u32::MAX; // -1 = as many as fit
        let mut byte_size = (buffer.len() * entry_size) as u32;

        // SAFETY: `handle.0` is a live enumeration handle from `WNetOpenEnumW`.
        // `buffer` is a live allocation of exactly `byte_size` bytes whose
        // alignment matches NETRESOURCEW because that is its element type. The
        // API writes only inside that range and does not retain the pointer;
        // `count` and `byte_size` are valid in/out `u32` locals.
        let ret = unsafe {
            WNetEnumResourceW(
                handle.0,
                &mut count,
                buffer.as_mut_ptr().cast(),
                &mut byte_size,
            )
        };

        match ret {
            NO_ERROR => return EnumStep::Entries((count as usize).min(buffer.len())),
            ERROR_NO_MORE_ITEMS => return EnumStep::Done,
            ERROR_MORE_DATA => {
                // `byte_size` now holds what a single entry requires.
                let needed = (byte_size as usize)
                    .div_ceil(entry_size)
                    .max(buffer.len().saturating_mul(2));
                if needed <= buffer.len() || needed > MAX_ENUM_ENTRIES {
                    return EnumStep::Failed(format!(
                        "WNetEnumResourceW demande un tampon de {byte_size} octets, au-delà de la limite"
                    ));
                }
                buffer.resize(needed, zeroed_netresource());
            }
            other => {
                return EnumStep::Failed(format!("WNetEnumResourceW échoué: code {other}"));
            }
        }
    }
}

#[cfg(windows)]
pub(super) fn discover_via_wnet() -> Result<Vec<NetworkResource>, String> {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::NO_ERROR;
    use windows_sys::Win32::NetworkManagement::WNet::{
        RESOURCE_GLOBALNET, RESOURCETYPE_DISK, RESOURCEUSAGE_CONTAINER, WNetOpenEnumW,
    };

    let mut results = Vec::new();
    let mut handle: *mut c_void = std::ptr::null_mut();

    // SAFETY: a null `lpNetResource` is the documented way to enumerate the root
    // of the global network; `handle` is a live local the API only writes to.
    let ret = unsafe {
        WNetOpenEnumW(
            RESOURCE_GLOBALNET,
            RESOURCETYPE_DISK,
            0,
            std::ptr::null(),
            &mut handle,
        )
    };
    if ret != NO_ERROR {
        return Err(format!("WNetOpenEnumW échoué: code {ret}"));
    }
    let handle = EnumHandle(handle);

    let mut buffer = vec![zeroed_netresource(); INITIAL_ENUM_ENTRIES];

    loop {
        let count = match enum_resources_step(&handle, &mut buffer) {
            EnumStep::Entries(count) => count,
            EnumStep::Done => break,
            EnumStep::Failed(error) => return Err(error),
        };

        // Safe slice: the elements were initialised before the call and the API
        // overwrote the first `count` of them with valid POD values.
        for res in &buffer[..count] {
            let remote_name = read_remote_name(res);
            if remote_name.is_empty() {
                continue;
            }

            // It could be a server or a share — check dwType
            let display_name = remote_name.trim_start_matches('\\').to_string();
            let is_container = res.dwUsage & RESOURCEUSAGE_CONTAINER != 0;

            if !is_container {
                results.push(NetworkResource {
                    name: display_name,
                    path: format!("smb:{}", remote_name),
                    status: NetworkStatus::Online,
                });
            } else {
                // Container (server/domain) - enumerate its shares
                match enumerate_shares_wnet(res) {
                    Ok(shares) if !shares.is_empty() => {
                        results.extend(shares);
                    }
                    _ => {
                        results.push(NetworkResource {
                            name: display_name,
                            path: format!("smb:{}", remote_name),
                            status: NetworkStatus::Online,
                        });
                    }
                }
            }
        }
    }

    Ok(results)
}

#[cfg(windows)]
pub(super) fn enumerate_shares_wnet(
    resource: &windows_sys::Win32::NetworkManagement::WNet::NETRESOURCEW,
) -> Result<Vec<NetworkResource>, String> {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::NO_ERROR;
    use windows_sys::Win32::NetworkManagement::WNet::{
        RESOURCE_GLOBALNET, RESOURCETYPE_DISK, WNetOpenEnumW,
    };

    let mut results = Vec::new();
    let mut handle: *mut c_void = std::ptr::null_mut();

    // SAFETY: `resource` points at an entry of the caller's live enumeration
    // buffer, together with the strings it references; both outlive this call.
    // The API reads it and writes only `handle`.
    let ret = unsafe {
        WNetOpenEnumW(
            RESOURCE_GLOBALNET,
            RESOURCETYPE_DISK,
            0,
            resource as *const _,
            &mut handle,
        )
    };
    if ret != NO_ERROR {
        return Err(format!("WNetOpenEnumW (shares) échoué: code {ret}"));
    }
    let handle = EnumHandle(handle);

    let mut buffer = vec![zeroed_netresource(); INITIAL_ENUM_ENTRIES];

    loop {
        let count = match enum_resources_step(&handle, &mut buffer) {
            EnumStep::Entries(count) => count,
            EnumStep::Done => break,
            EnumStep::Failed(error) => {
                tracing::debug!("Réseau: énumération des partages interrompue: {error}");
                break;
            }
        };

        for res in &buffer[..count] {
            let remote_name = read_remote_name(res);
            if remote_name.is_empty() {
                continue;
            }

            // Skip admin shares
            let share_name = remote_name.rsplit('\\').next().unwrap_or("");
            if share_name.ends_with('$') || share_name.eq_ignore_ascii_case("IPC$") {
                continue;
            }

            let display_name = remote_name.trim_start_matches('\\').to_string();
            results.push(NetworkResource {
                name: display_name,
                path: format!("smb:{}", remote_name),
                status: NetworkStatus::Online,
            });
        }
    }

    Ok(results)
}

/// Copies the `lpRemoteName` of an enumeration entry into an owned `String`.
#[cfg(windows)]
pub(super) fn read_remote_name(
    resource: &windows_sys::Win32::NetworkManagement::WNet::NETRESOURCEW,
) -> String {
    if resource.lpRemoteName.is_null() {
        return String::new();
    }
    // SAFETY: the pointer was filled by WNetEnumResourceW and points into the
    // still-live enumeration buffer that `resource` belongs to, NUL-terminated
    // as documented. `wstr_to_string` only reads, and caps its own scan.
    unsafe { wstr_to_string(resource.lpRemoteName) }
}

/// Reads a NUL-terminated UTF-16 string.
///
/// # Safety
/// `ptr` must be null or point at a NUL-terminated UTF-16 buffer that stays
/// valid for the duration of the call.
#[cfg(windows)]
pub(super) unsafe fn wstr_to_string(ptr: *const u16) -> String {
    /// Nothing the WNet API returns comes close; the cap only bounds the scan if
    /// a provider ever hands back an unterminated buffer.
    const MAX_WIDE_LEN: usize = 32768;

    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: the caller guarantees a NUL-terminated buffer; we stop at the
    // terminator or at MAX_WIDE_LEN, whichever comes first.
    unsafe {
        while len < MAX_WIDE_LEN && *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

// ── net view fallback ──────────────────────────────────────────────────────
