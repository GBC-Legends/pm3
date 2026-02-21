use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System, Uid, UpdateKind, Users};

static USERS: OnceLock<RwLock<Users>> = OnceLock::new();
static LAST_REFRESH_SEC: OnceLock<RwLock<u64>> = OnceLock::new();

fn users_lock() -> &'static RwLock<Users> {
    USERS.get_or_init(|| RwLock::new(Users::new_with_refreshed_list()))
}

fn last_refresh_lock() -> &'static RwLock<u64> {
    LAST_REFRESH_SEC.get_or_init(|| RwLock::new(0))
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn maybe_refresh_users() {
    let now = now_epoch_secs();

    if now % 10 != 0 {
        return;
    }

    {
        let last = last_refresh_lock().read().unwrap();
        if *last == now {
            return;
        }
    }

    {
        let mut last = last_refresh_lock().write().unwrap();
        if *last == now {
            return;
        }
        *last = now;
    }

    let mut guard = users_lock().write().unwrap();
    *guard = Users::new_with_refreshed_list();
}

pub fn username_by_user_id(uid: &Uid) -> Option<String> {
    maybe_refresh_users();

    let guard = users_lock().read().unwrap();
    guard.get_user_by_id(uid).map(|u| u.name().to_string())
}

pub fn username_for_pid(pid: &Pid) -> Option<String> {
    maybe_refresh_users();

    let mut sys = System::new();
    let rk = RefreshKind::new()
        .with_processes(ProcessRefreshKind::new().with_user(UpdateKind::OnlyIfNotSet));

    sys.refresh_specifics(rk);

    let p = sys.process(*pid)?;
    let uid = p.user_id()?;
    username_by_user_id(uid)
}
