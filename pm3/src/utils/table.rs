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

    #[tabled(rename = "EXIT")]
    exit: String,
}

fn fmt_uptime(sec: Option<u64>) -> String {
    match sec {
        Some(s) => {
            let h = s / 3600;
            let m = (s % 3600) / 60;
            let s = s % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        }
        None => "-".into(),
    }
}

fn fmt_mem(mem: Option<u64>) -> String {
    match mem {
        Some(bytes) => {
            let mb = bytes as f64 / 1024.0 / 1024.0;
            format!("{:.1} MB", mb)
        }
        None => "-".into(),
    }
}

fn fmt_cpu(cpu: Option<f32>) -> String {
    cpu.map(|c| format!("{:.2}", c))
        .unwrap_or_else(|| "-".into())
}

fn fmt_pid(pid: Option<u32>) -> String {
    pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into())
}

fn fmt_exit(exit: Option<i32>) -> String {
    exit.map(|e| e.to_string()).unwrap_or_else(|| "-".into())
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
                ProcessStatus::Exited => "EXITED".into(),
            },
            cpu: fmt_cpu(p.cpu),
            mem: fmt_mem(p.mem),
            user: p.user.clone().unwrap_or_else(|| "-".into()),
            uptime: fmt_uptime(p.uptime),
            exit: fmt_exit(p.exit_code),
        })
        .collect();

    let mut table = Table::new(rows);

    table
        .with(Style::modern_rounded())
        .with(Modify::new(Columns::new(0..=0)).with(Alignment::right()))
        .with(Modify::new(Columns::new(2..=2)).with(Alignment::right()))
        .with(Modify::new(Columns::new(4..=4)).with(Alignment::right()))
        .with(Modify::new(Columns::new(5..=5)).with(Alignment::right()))
        .with(Modify::new(Columns::new(8..=8)).with(Alignment::right()));

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

    println!("{table}");
}
