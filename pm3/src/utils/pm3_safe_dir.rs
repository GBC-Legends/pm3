use std::env;
use std::path::PathBuf;

fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("USERPROFILE").map(PathBuf::from)
    }

    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

fn pm3_base_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("APPDATA").map(PathBuf::from)
    }

    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        home_dir()
    }
}

pub(crate) fn pm3_home_dir_safe() -> PathBuf {
    let base = pm3_base_dir()
        .or_else(home_dir)
        .unwrap_or_else(|| env::current_dir().expect("cannot determine any usable directory"));

    base.join(".pm3")
}
