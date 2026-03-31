pub mod tui;

use std::io::Result;

pub fn open_monitor() -> Result<()> {
    tui::run()
}
