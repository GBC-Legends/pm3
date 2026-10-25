pub(crate) fn pm3_home_dir_safe() -> std::path::PathBuf {
    pm3_runtime::state_dir().expect("cannot resolve PM3 instance directory")
}
