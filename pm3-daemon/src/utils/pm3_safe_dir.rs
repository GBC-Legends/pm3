use std::{env, fs, path::PathBuf};

pub(crate) fn pm3_home_dir_safe() -> PathBuf {
    let dir = pm3_runtime::state_dir().expect("cannot resolve PM3 instance directory");
    fs::create_dir_all(&dir).expect("cannot create PM3 instance directory; install the system service or set PM3_HOME for development");
    dir
}

pub(crate) fn current_dir_safe() -> PathBuf {
    env::current_dir().expect("cannot determine working directory")
}
