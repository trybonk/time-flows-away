//! Per-user startup registration. Changed only from an explicit settings action.
use crate::options::Options;
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::*,
};
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "TimeFlowsAway";
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn result(code: u32) -> Result<(), String> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(code as i32).to_string())
    }
}

fn quoted(arg: &OsStr) -> Vec<u16> {
    // CreateProcess/CommandLineToArgvW quoting, including trailing backslashes.
    let mut out = vec![b'"' as u16];
    let mut slashes = 0;
    for c in arg.encode_wide() {
        if c == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            b'\\' as u16,
            if c == b'"' as u16 {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    out.push(b'"' as u16);
    out
}
fn command(exe: &Path, options: &Options) -> Result<Vec<u16>, String> {
    let config = std::path::absolute(&options.config).map_err(|e| e.to_string())?;
    let args = [
        exe.as_os_str(),
        OsStr::new(if options.opengl { "--opengl" } else { "--wgpu" }),
        OsStr::new("--config"),
        config.as_os_str(),
    ];
    let mut out = Vec::new();
    for arg in args {
        if !out.is_empty() {
            out.push(b' ' as u16);
        }
        out.extend(quoted(arg));
    }
    Ok(out)
}
fn read_at(path: &str, name: &str) -> Result<Option<Vec<u16>>, String> {
    unsafe {
        let (path, name) = (wide(path), wide(name));
        let mut bytes = 0;
        let code = RegGetValueW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut bytes,
        );
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        result(code)?;
        if bytes > 65536 {
            return Err("Startup command is too long".into());
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        result(RegGetValueW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        ))?;
        buffer.truncate(bytes as usize / 2);
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        Ok(Some(buffer))
    }
}
fn write_at(path: &str, name: &str, value: Option<&[u16]>) -> Result<(), String> {
    unsafe {
        let (path, name) = (wide(path), wide(name));
        let mut key = std::ptr::null_mut();
        if value.is_some() {
            result(RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            ))?;
        } else {
            let code = RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key);
            if code == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            result(code)?;
        }
        let key = Key(key);
        if let Some(value) = value {
            let value: Vec<u16> = value.iter().copied().chain(Some(0)).collect();
            result(RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            ))
        } else {
            let code = RegDeleteValueW(key.0, name.as_ptr());
            if code == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                result(code)
            }
        }
    }
}
fn registered_executable(command: &[u16]) -> Result<std::path::PathBuf, String> {
    use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};
    if command.is_empty() {
        return Err("Empty startup command".into());
    }
    let line: Vec<u16> = command.iter().copied().chain(Some(0)).collect();
    unsafe {
        let mut count = 0;
        let argv = CommandLineToArgvW(line.as_ptr(), &mut count);
        if argv.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let path = if count > 0 {
            let first = *argv;
            let mut length = 0;
            while *first.add(length) != 0 {
                length += 1;
            }
            use std::os::windows::ffi::OsStringExt;
            Some(std::path::PathBuf::from(std::ffi::OsString::from_wide(
                std::slice::from_raw_parts(first, length),
            )))
        } else {
            None
        };
        LocalFree(argv.cast());
        path.filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| "Startup command has no executable".into())
    }
}
pub struct Status {
    pub enabled: bool,
    pub current: bool,
    pub warning: Option<String>,
}
pub fn status(options: &Options) -> Result<Status, String> {
    let saved = read_at(RUN, NAME)?;
    let expected = command(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        options,
    )?;
    let warning = saved
        .as_ref()
        .and_then(|line| match registered_executable(line) {
            Ok(path) if !path.is_file() => Some(format!(
                "Startup executable was moved or removed: {}",
                path.display()
            )),
            Ok(_) if saved.as_ref() != Some(&expected) => Some(
                "Startup uses another application location or settings. Update it to use this EXE."
                    .into(),
            ),
            Ok(_) => None,
            Err(error) => Some(format!("Invalid startup registration: {error}")),
        });
    Ok(Status {
        warning,
        enabled: saved.is_some(),
        current: saved.as_ref() == Some(&expected),
    })
}
pub fn set(enabled: bool, options: &Options) -> Result<(), String> {
    if enabled {
        let command = command(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            options,
        )?;
        write_at(RUN, NAME, Some(&command))
    } else {
        write_at(RUN, NAME, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registered_path_parsing_keeps_spaces_unicode_and_backslashes() {
        let exe = Path::new(r"C:\Папка с пробелом\app.exe");
        let line = command(
            exe,
            &Options::parse(["--config", r"C:\settings.toml"].map(Into::into)).unwrap(),
        )
        .unwrap();
        assert_eq!(registered_executable(&line).unwrap(), exe);
        assert!(registered_executable(&[]).is_err());
    }
    #[test]
    fn startup_preserves_unicode_paths_and_excludes_internal_worker_flags() {
        use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};
        let options = Options::parse(
            [
                "--settings",
                "--opengl",
                "--wallpaper-owner",
                "1234",
                "--config",
                r"C:\папка с пробелом\config.toml",
            ]
            .map(Into::into),
        )
        .unwrap();
        let mut line = command(
            Path::new(r"C:\Program Files\Time Flows Away\app.exe"),
            &options,
        )
        .unwrap();
        line.push(0);
        unsafe {
            let mut count = 0;
            let argv = CommandLineToArgvW(line.as_ptr(), &mut count);
            assert!(!argv.is_null());
            let args: Vec<_> = std::slice::from_raw_parts(argv, count as usize)
                .iter()
                .map(|p| {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1;
                    }
                    String::from_utf16_lossy(std::slice::from_raw_parts(*p, n))
                })
                .collect();
            LocalFree(argv.cast());
            assert_eq!(
                args,
                [
                    r"C:\Program Files\Time Flows Away\app.exe",
                    "--opengl",
                    "--config",
                    r"C:\папка с пробелом\config.toml"
                ]
            );
        }
        assert_eq!(
            String::from_utf16(&quoted(OsStr::new("trailing\\"))).unwrap(),
            "\"trailing\\\\\""
        );
    }
    #[test]
    fn registration_roundtrips_and_removal_is_idempotent_in_isolated_key() {
        let path = format!(
            r"Software\TimeFlowsAway\Tests\{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                unsafe {
                    RegDeleteTreeW(HKEY_CURRENT_USER, wide(&self.0).as_ptr());
                }
            }
        }
        let _cleanup = Cleanup(path.clone());
        assert_eq!(read_at(&path, NAME).unwrap(), None);
        let value: Vec<u16> = "\"C:\\Папка\\app.exe\" --opengl".encode_utf16().collect();
        write_at(&path, NAME, Some(&value)).unwrap();
        assert_eq!(read_at(&path, NAME).unwrap(), Some(value));
        write_at(&path, NAME, None).unwrap();
        write_at(&path, NAME, None).unwrap();
        assert_eq!(read_at(&path, NAME).unwrap(), None);
    }
}
