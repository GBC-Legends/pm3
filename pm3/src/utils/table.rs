use crate::models::process::{PmProcessStatusInfo, ProcessStatus};
use tabled::Table;
use tabled::settings::object::Columns;
use tabled::settings::{Alignment, Color, Modify, Style, object::Cell, object::Rows};

use tabled::Tabled;

#[derive(Tabled)]
struct ProcessRow {
    #[tabled(rename = "ID")]
    id: u64,

    #[tabled(rename = "NAME")]
    name: String,

    #[tabled(rename = "PID")]
    pid: String,

    #[tabled(rename = "STATUS")]
    status: String,

    #[tabled(rename = "CPU%")]
    cpu: String,

    #[tabled(rename = "MEM")]
    mem: String,

    #[tabled(rename = "USER")]
    user: String,

    #[tabled(rename = "UPTIME")]
    uptime: String,
}

fn fmt_uptime(sec: Option<u64>) -> String {
    let mut s = match sec {
        Some(v) => v,
        None => return "-".into(),
    };

    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let years = s / YEAR;
    s %= YEAR;

    let months = s / MONTH;
    s %= MONTH;

    let days = s / DAY;
    s %= DAY;

    let hours = s / HOUR;
    s %= HOUR;

    let minutes = s / MIN;
    let seconds = s % MIN;

    if years > 0 {
        return format!("{} year{}", years, if years != 1 { "s" } else { "" });
    }

    if months > 0 {
        return format!("{} month{}", months, if months != 1 { "s" } else { "" });
    }

    if days > 0 {
        return format!("{} day{}", days, if days != 1 { "s" } else { "" });
    }

    if hours > 0 {
        return format!("{}h {}m", hours, minutes);
    }

    if minutes > 0 {
        return format!("{}m {}s", minutes, seconds);
    }

    format!("{}s", seconds)
}

fn fmt_mem(mem: Option<u64>) -> String {
    let bytes = match mem {
        Some(b) => b,
        None => return "-".into(),
    };

    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;
    const TB: f64 = 1024.0 * GB;

    let b = bytes as f64;

    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_cpu(cpu: Option<f32>) -> String {
    cpu.map(|c| format!("{:.2}", c))
        .unwrap_or_else(|| "-".into())
}

fn fmt_pid(pid: Option<u32>) -> String {
    pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into())
}

pub fn print_process_table(processes: &[PmProcessStatusInfo]) {
    let rows: Vec<ProcessRow> = processes
        .iter()
        .map(|p| ProcessRow {
            id: p.id,
            name: p.name.clone(),
            pid: fmt_pid(p.pid),
            status: match p.status {
                ProcessStatus::Running => "RUNNING".into(),
                ProcessStatus::Stopped => "STOPPED".into(),
                ProcessStatus::Exited => match p.exit_code {
                    Some(code) => format!("EXITED ({})", code),
                    None => "EXITED".into(),
                },
            },
            cpu: fmt_cpu(p.cpu),
            mem: fmt_mem(p.mem),
            user: p.user.clone().unwrap_or_else(|| "-".into()),
            uptime: fmt_uptime(p.uptime),
        })
        .collect();

    let mut table = Table::new(rows);

    table
        .with(Style::modern_rounded())
        .with(Modify::new(Columns::new(0..=0)).with(Alignment::right()))
        .with(Modify::new(Columns::new(2..=2)).with(Alignment::right()))
        .with(Modify::new(Columns::new(4..=4)).with(Alignment::right()))
        .with(Modify::new(Columns::new(5..=5)).with(Alignment::right()));

    for (i, proc) in processes.iter().enumerate() {
        let row = i + 1;
        let col = 3;

        let color = match proc.status {
            ProcessStatus::Running => Color::FG_GREEN,
            ProcessStatus::Stopped => Color::FG_YELLOW,
            ProcessStatus::Exited => Color::FG_RED,
        };

        table.with(
            Modify::new(Cell::new(row, col))
                .with(color)
                .with(Alignment::center()),
        );
    }

    table.with(
        Modify::new(Rows::first())
            .with(Color::FG_CYAN)
            .with(Alignment::center()),
    );

    println!("Process status:");
    println!("{table}");
}
