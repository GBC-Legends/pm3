//! Shared instance discovery. Linux never implicitly falls back to a user's home.
use std::{env, io, path::PathBuf, sync::OnceLock};

static INSTANCE: OnceLock<String> = OnceLock::new();

pub fn validate_instance(name: &str) -> Result<String, String> {
    if name.is_empty()
        || name.len() > 32
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        || !name.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err("instance must be 1–32 lowercase letters, digits, '-' or '_', starting with a letter or digit".into());
    }
    Ok(name.to_owned())
}

pub fn select_instance(name: &str) -> Result<(), String> {
    INSTANCE
        .set(validate_instance(name)?)
        .map_err(|_| "instance already selected".into())
}

pub fn instance() -> io::Result<String> {
    let name = INSTANCE
        .get()
        .cloned()
        .unwrap_or_else(|| env::var("PM3_INSTANCE").unwrap_or_else(|_| "default".into()));
    validate_instance(&name).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}

pub fn state_dir() -> io::Result<PathBuf> {
    let name = instance()?;
    if let Some(path) = env::var_os("PM3_HOME") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PM3_HOME must be an absolute path",
            ));
        }
        return Ok(path);
    }
    #[cfg(target_os = "linux")]
    return Ok(linux_state_dir(&name));

    #[cfg(not(target_os = "linux"))]
    {
        #[cfg(target_os = "windows")]
        let base = env::var_os("APPDATA")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = env::var_os("HOME").map(PathBuf::from).map(|h| {
            if cfg!(target_os = "macos") {
                h.join("Library/Application Support")
            } else {
                h
            }
        });
        let base = base
            .or_else(|| env::current_dir().ok())
            .ok_or_else(|| io::Error::other("cannot determine state directory"))?
            .join(".pm3");
        Ok(if name == "default" {
            base
        } else {
            base.join("instances").join(name)
        })
    }
}

#[cfg(any(target_os = "linux", test))]
fn linux_state_dir(name: &str) -> PathBuf {
    PathBuf::from("/var/lib/pm3").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_cannot_escape_the_instance_directory_or_systemd_unit() {
        for name in [
            "",
            "..",
            "../root",
            "a/b",
            "a@b",
            "a b",
            "-a",
            "UPPER",
            "a\nb",
            &"a".repeat(33),
        ] {
            assert!(validate_instance(name).is_err(), "{name:?}");
        }
        for name in ["default", "api-2", "team_a", "5"] {
            assert_eq!(validate_instance(name).unwrap(), name);
        }
    }
    #[test]
    fn linux_instances_have_system_wide_distinct_paths() {
        assert_eq!(
            linux_state_dir("default"),
            PathBuf::from("/var/lib/pm3/default")
        );
        assert_ne!(linux_state_dir("default"), linux_state_dir("api"));
    }
}
