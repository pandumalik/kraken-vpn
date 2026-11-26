//! Credential protection at rest using Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`),
//! with a clean cross-platform fallback for non-Windows builds.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

const DPAPI_PREFIX: &str = "dpapi:v1:";
const FALLBACK_PREFIX: &str = "b64:v1:";
const ENTROPY: &[u8] = b"KrakenVPN.SecretStore.v2";

/// Protects a plaintext secret for storage on disk.
/// Returns `Ok(None)` for empty secrets so empty passwords are never stored.
pub fn protect_secret(plaintext: &str) -> Result<Option<String>, String> {
    if plaintext.is_empty() {
        return Ok(None);
    }

    #[cfg(windows)]
    {
        let blob = dpapi_protect(plaintext.as_bytes())?;
        Ok(Some(format!("{DPAPI_PREFIX}{}", B64.encode(blob))))
    }

    #[cfg(not(windows))]
    {
        Ok(Some(format!(
            "{FALLBACK_PREFIX}{}",
            B64.encode(plaintext.as_bytes())
        )))
    }
}

/// Unprotects a stored secret blob back into memory.
pub fn unprotect_secret(stored: Option<&str>) -> Result<String, String> {
    let Some(raw) = stored.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(String::new());
    };

    if let Some(encoded) = raw.strip_prefix(DPAPI_PREFIX) {
        #[cfg(windows)]
        {
            let cipher = B64
                .decode(encoded)
                .map_err(|e| format!("Invalid base64 in protected credential: {e}"))?;
            let plain = dpapi_unprotect(&cipher)?;
            return String::from_utf8(plain)
                .map_err(|e| format!("Decrypted credential is not valid UTF-8: {e}"));
        }
        #[cfg(not(windows))]
        {
            let _ = encoded;
            return Err(
                "DPAPI-protected credentials can only be decrypted on Windows.".to_string(),
            );
        }
    }

    if let Some(encoded) = raw.strip_prefix(FALLBACK_PREFIX) {
        let bytes = B64
            .decode(encoded)
            .map_err(|e| format!("Invalid base64 in stored credential: {e}"))?;
        return String::from_utf8(bytes)
            .map_err(|e| format!("Decoded credential is not valid UTF-8: {e}"));
    }

    Err("Unrecognized credential encoding format.".to_string())
}

#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|_| "Secret too large".to_string())?,
        pbData: data.as_ptr() as *mut u8,
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: All pointers refer to valid slices for the duration of the FFI call,
    // and output.pbData is freed via LocalFree when CryptProtectData succeeds.
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            &entropy,
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err("Windows DPAPI CryptProtectData failed".to_string());
    }

    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData as *mut _);
    }
    Ok(bytes)
}

#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|_| "Ciphertext too large".to_string())?,
        pbData: data.as_ptr() as *mut u8,
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: All pointers refer to valid slices; output.pbData is freed via LocalFree on success.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            &entropy,
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err("Windows DPAPI CryptUnprotectData failed".to_string());
    }

    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData as *mut _);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_and_unprotects_secret_round_trip() {
        let protected = protect_secret("s3cr3t-p@ss!").unwrap().unwrap();
        assert!(!protected.contains("s3cr3t-p@ss!"));
        let restored = unprotect_secret(Some(&protected)).unwrap();
        assert_eq!(restored, "s3cr3t-p@ss!");
    }

    #[test]
    fn empty_secret_returns_none_and_restores_empty() {
        assert_eq!(protect_secret("").unwrap(), None);
        assert_eq!(unprotect_secret(None).unwrap(), "");
        assert_eq!(unprotect_secret(Some("  ")).unwrap(), "");
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(unprotect_secret(Some("plaintext-not-allowed")).is_err());
    }
}
