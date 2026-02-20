pub mod pm3_process;
pub mod runner;

mod idx {

    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    pub fn alloc_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }
}
