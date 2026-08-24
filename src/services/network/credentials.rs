//! Connecting to a share that needs credentials.
//!
//! `SecretWide` zeroes the password buffer on drop.

/// UTF-16 buffer that overwrites itself when dropped.
///
/// The share password used to be copied into a plain `Vec<u16>` and left behind
/// in freed memory. `zeroize` is not a direct dependency of this crate, so the
/// scrub is done by hand with volatile writes the optimiser may not elide.
#[cfg(any(windows, test))]
pub(super) struct SecretWide {
    pub(super) buf: Vec<u16>,
}

#[cfg(any(windows, test))]
impl SecretWide {
    pub(super) fn new(value: &str) -> Self {
        Self {
            buf: value.encode_utf16().chain(std::iter::once(0)).collect(),
        }
    }

    #[cfg(windows)]
    fn as_ptr(&self) -> *const u16 {
        self.buf.as_ptr()
    }

    pub(super) fn zeroize(&mut self) {
        for slot in self.buf.iter_mut() {
            // SAFETY: `slot` is a live, aligned `u16` owned by this buffer.
            unsafe { std::ptr::write_volatile(slot, 0) };
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(any(windows, test))]
impl Drop for SecretWide {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// #3: Connect to a password-protected network share using WNetAddConnection2W.
/// Returns Ok(()) on success, or an error message on failure.
#[cfg(windows)]
pub fn connect_authenticated(
    remote_path: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    use windows_sys::Win32::Foundation::NO_ERROR;
    use windows_sys::Win32::NetworkManagement::WNet::{
        CONNECT_TEMPORARY, NETRESOURCEW, RESOURCETYPE_DISK, WNetAddConnection2W,
    };

    // `lpRemoteName` is a `*mut u16`; deriving it from an immutable borrow was a
    // provenance lie even though the API only reads it.
    let mut remote_wide: Vec<u16> = remote_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let user_wide: Vec<u16> = username.encode_utf16().chain(std::iter::once(0)).collect();
    let pass_wide = SecretWide::new(password);

    let net_resource = NETRESOURCEW {
        dwScope: 0,
        dwType: RESOURCETYPE_DISK,
        dwDisplayType: 0,
        dwUsage: 0,
        lpLocalName: std::ptr::null_mut(),
        lpRemoteName: remote_wide.as_mut_ptr(),
        lpComment: std::ptr::null_mut(),
        lpProvider: std::ptr::null_mut(),
    };

    // SAFETY: all three buffers are NUL-terminated UTF-16 vectors that live
    // until the end of this function, i.e. past the call; the API reads them and
    // does not retain the pointers. CONNECT_TEMPORARY keeps the mapping out of
    // the user's persistent connections.
    let ret = unsafe {
        WNetAddConnection2W(
            &net_resource,
            pass_wide.as_ptr(),
            user_wide.as_ptr(),
            CONNECT_TEMPORARY,
        )
    };

    // Scrub the password copy now rather than waiting for the end of scope.
    drop(pass_wide);

    if ret == NO_ERROR {
        tracing::info!("Réseau: connexion authentifiée à {} réussie", remote_path);
        Ok(())
    } else {
        let msg = format!("WNetAddConnection2W échoué: code {ret}");
        tracing::warn!("Réseau: {msg}");
        Err(msg)
    }
}

#[cfg(not(windows))]
pub fn connect_authenticated(
    _remote_path: &str,
    _username: &str,
    _password: &str,
) -> Result<(), String> {
    Err("Connexion authentifiée non supportée sur cette plateforme".to_string())
}
