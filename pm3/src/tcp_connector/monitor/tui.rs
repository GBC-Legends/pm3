use crate::models::process::ProcessStatus;
use crate::tcp_connector::{AAD, init_stream, send_secure_command};
use crate::utils::config::Config;
use crate::utils::encryption::{decrypt_wire_line, encrypt_reply_to_token};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, ListState, Paragraph,
    },
};

use std::{
    io::{BufRead, BufReader, ErrorKind, Result, Write, stdout},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
struct ProcessInfo {
    id: u64,
    name: String,
    status: ProcessStatus,
    pid: Option<u32>,
    uptime: Option<u64>,
    cpu: Option<f32>,
    mem: Option<u64>,
    user: Option<String>,
    exit_code: Option<i32>,
}

struct LogStreamHandle {
    stop: Arc<AtomicBool>,
}

impl LogStreamHandle {
    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct App {
    processes: Vec<ProcessInfo>,
    selected: usize,
    list_state: ListState,
    logs: Vec<String>,
    stream: Option<LogStreamHandle>,
    log_rx: Option<Receiver<String>>,
    show_all_logs: bool,
    status_message: String,
    cpu_history: Vec<(f64, f64)>,
    mem_history: Vec<(f64, f64)>,
    tick: f64,
    log_scroll: usize,
    process_scroll: usize,
    log_view_height: usize,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            processes: Vec::new(),
            selected: 0,
            list_state,
            logs: Vec::new(),
            stream: None,
            log_rx: None,
            show_all_logs: false,
            status_message: String::from("Connecting..."),
            cpu_history: Vec::new(),
            mem_history: Vec::new(),
            tick: 0.0,
            log_scroll: 0,
            process_scroll: 0,
            log_view_height: 0,
        }
    }

    fn selected_process(&self) -> Option<&ProcessInfo> {
        self.processes.get(self.selected)
    }

    fn selected_process_id(&self) -> Option<u64> {
        self.selected_process().map(|p| p.id)
    }

    fn sync_list_selection(&mut self) {
        if self.processes.is_empty() {
            self.selected = 0;
            self.process_scroll = 0;
            self.list_state.select(None);
        } else {
            if self.selected >= self.processes.len() {
                self.selected = self.processes.len().saturating_sub(1);
            }

            if self.process_scroll > self.selected {
                self.process_scroll = self.selected;
            }

            self.list_state.select(Some(self.selected));
        }
    }

    fn move_up(&mut self) -> bool {
        if self.processes.is_empty() {
            return false;
        }
        let old = self.selected;
        self.selected = self.selected.saturating_sub(1);
        self.sync_list_selection();
        old != self.selected
    }

    fn move_down(&mut self) -> bool {
        if self.processes.is_empty() {
            return false;
        }
        let old = self.selected;
        self.selected = (self.selected + 1).min(self.processes.len().saturating_sub(1));
        self.sync_list_selection();
        old != self.selected
    }

    fn push_log_line(&mut self, line: String) {
        self.logs.push(line);

        const MAX_LOG_LINES: usize = 500;

        if self.logs.len() > MAX_LOG_LINES {
            let overflow = self.logs.len() - MAX_LOG_LINES;
            self.logs.drain(0..overflow);
        }
    }

    fn clear_logs(&mut self) {
        self.logs.clear();
    }

    fn clear_metrics(&mut self) {
        self.cpu_history.clear();
        self.mem_history.clear();
        self.tick = 0.0;
    }

    fn stop_stream(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.stop();
        }
        self.log_rx = None;
    }

    fn start_stream_for_current_selection(&mut self) {
        self.stop_stream();
        self.log_scroll = 0;

        let target = if self.show_all_logs {
            None
        } else {
            self.selected_process_id()
        };

        let (tx, rx) = mpsc::channel();
        let handle = spawn_logs_stream(target, tx);

        self.stream = Some(handle);
        self.log_rx = Some(rx);
    }

    fn drain_logs(&mut self) {
        let mut pending = Vec::new();

        if let Some(rx) = &self.log_rx {
            while let Ok(line) = rx.try_recv() {
                pending.push(line);
            }
        }

        for line in pending {
            self.push_log_line(line);
        }
    }

    fn refresh_status(&mut self) {
        let prev_selected_id = self.selected_process_id();

        match fetch_status_snapshot() {
            Ok(mut next) => {
                if next.is_empty() {
                    self.processes.clear();
                    self.sync_list_selection();
                    self.status_message = "No processes managed by daemon".to_string();
                    return;
                }

                next.sort_by_key(|p| p.id);
                self.processes = next;

                if let Some(prev_id) = prev_selected_id {
                    if let Some(idx) = self.processes.iter().position(|p| p.id == prev_id) {
                        self.selected = idx;
                    }
                }

                self.sync_list_selection();

                if let Some(p) = self.selected_process() {
                    if matches!(p.status, ProcessStatus::Running) {
                        if self.cpu_history.is_empty() {
                            let (cpu_hist, mem_hist) = fetch_metrics(p.id, current_since());

                            self.cpu_history = cpu_hist;
                            self.mem_history = mem_hist;
                            self.tick = self.cpu_history.len() as f64;
                        } else {
                            let (cpu_hist, mem_hist) = fetch_metrics(p.id, current_since());

                            self.tick += 1.0;

                            if let Some((_, cpu)) = cpu_hist.last() {
                                self.cpu_history.push((self.tick, *cpu));
                            } else {
                                self.cpu_history.push((self.tick, 0.0));
                            }

                            if let Some((_, mem)) = mem_hist.last() {
                                self.mem_history.push((self.tick, *mem));
                            } else {
                                self.mem_history.push((self.tick, 0.0));
                            }

                            if self.cpu_history.len() > 30 {
                                self.cpu_history.remove(0);
                            }

                            if self.mem_history.len() > 30 {
                                self.mem_history.remove(0);
                            }
                        }
                    } else {
                        if self.cpu_history.is_empty() {
                            self.cpu_history = (0..30).map(|i| (i as f64, 0.0)).collect();

                            self.mem_history = (0..30).map(|i| (i as f64, 0.0)).collect();

                            self.tick = 30.0;
                        } else {
                            self.tick += 1.0;

                            self.cpu_history.push((self.tick, 0.0));
                            self.mem_history.push((self.tick, 0.0));

                            if self.cpu_history.len() > 30 {
                                self.cpu_history.remove(0);
                            }

                            if self.mem_history.len() > 30 {
                                self.mem_history.remove(0);
                            }
                        }
                    }
                }

                self.status_message = format!("Connected • {} process(es)", self.processes.len());
            }
            Err(e) => {
                self.status_message = format!("Status refresh failed: {e}");
            }
        }
    }
}

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let app_result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    app_result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    app.refresh_status();
    app.start_stream_for_current_selection();

    let mut last_refresh = Instant::now();
    let mut last_selection_change = Instant::now();
    let mut pending_restart = false;

    loop {
        app.drain_logs();

        terminal.draw(|f| {
            let root = f.area();

            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(8),
                    Constraint::Length(12),
                    Constraint::Length(1),
                ])
                .split(root);

            let top = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
                .split(vertical[0]);

            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
                .split(vertical[1]);

            draw_process_list(f, top[0], &mut app);
            draw_details_panel(f, bottom[0], &app);
            draw_logs_panel(f, bottom[1], &mut app);
            draw_controls_panel(f, vertical[2]);
            let charts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(top[1]);

            draw_cpu_chart(f, charts[0], &app);
            draw_mem_chart(f, charts[1], &app);
        })?;

        if last_refresh.elapsed() >= Duration::from_secs(1) {
            let before_id = app.selected_process_id();
            app.refresh_status();
            let after_id = app.selected_process_id();

            if app.show_all_logs {
            } else if before_id != after_id {
                app.clear_logs();
                app.clear_metrics();
                app.start_stream_for_current_selection();
            }

            last_refresh = Instant::now();
        }

        if pending_restart && last_selection_change.elapsed() >= Duration::from_millis(90) {
            app.clear_logs();
            app.clear_metrics();
            app.start_stream_for_current_selection();
            pending_restart = false;
        }

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.stop_stream();
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.move_up() && !app.show_all_logs {
                            app.clear_metrics();
                            last_selection_change = Instant::now();
                            pending_restart = true;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.move_down() && !app.show_all_logs {
                            app.clear_metrics();
                            last_selection_change = Instant::now();
                            pending_restart = true;
                        }
                    }
                    KeyCode::Char('c') => {
                        app.clear_logs();
                    }
                    KeyCode::Char('s') => {
                        if let Some(id) = app.selected_process_id() {
                            let _ = send_secure_command(&format!("stop {}", id));
                            app.refresh_status();
                        }
                    }
                    KeyCode::Char('r') => {
                        if let Some(id) = app.selected_process_id() {
                            let _ = send_secure_command(&format!("restart {}", id));
                            app.refresh_status();
                        }
                    }
                    KeyCode::PageUp => {
                        let max_scroll = app.logs.len().saturating_sub(app.log_view_height);
                        app.log_scroll = app.log_scroll.saturating_add(1).min(max_scroll);
                    }

                    KeyCode::PageDown => {
                        app.log_scroll = app.log_scroll.saturating_sub(1);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn draw_process_list(f: &mut ratatui::Frame<'_>, area: Rect, app: &mut App) {
    let max_lines = area.height.saturating_sub(2) as usize;

    if app.selected >= app.process_scroll + max_lines {
        app.process_scroll = app.selected.saturating_sub(max_lines.saturating_sub(2));
    }

    if app.selected < app.process_scroll {
        app.process_scroll = app.selected;
    }

    let max_scroll = app.processes.len().saturating_sub(max_lines);
    app.process_scroll = app.process_scroll.min(max_scroll);

    let visible = app
        .processes
        .iter()
        .skip(app.process_scroll)
        .take(max_lines);

    let items: Vec<ListItem> = visible
        .map(|p| {
            let status = status_label(&p.status);
            let status_color = status_color(&p.status);

            ListItem::new(Line::from(vec![
                Span::styled(format!("[{}] ", p.id), Style::default().fg(Color::Blue)),
                Span::raw(format!("{} ", p.name)),
                Span::styled(
                    status,
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
        })
        .collect();

    let mut local_state = ListState::default();
    local_state.select(Some(app.selected.saturating_sub(app.process_scroll)));

    let list = List::new(items)
        .block(Block::default().title(" Processes ").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::Rgb(30, 50, 85)))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut local_state);
}

fn draw_logs_panel(f: &mut ratatui::Frame<'_>, area: Rect, app: &mut App) {
    let max_lines = area.height.saturating_sub(2) as usize;
    app.log_view_height = max_lines;
    let total = app.logs.len();
    let max_scroll = total.saturating_sub(max_lines);
    let scroll = app.log_scroll.min(max_scroll);
    app.log_scroll = scroll;
    let start = max_scroll.saturating_sub(scroll);
    let end = (start + max_lines).min(total);
    let visible = &app.logs[start..end];

    let lines: Vec<Line> = visible
        .iter()
        .map(|line| {
            if let Some(pos) = line.find("] ") {
                let prefix = &line[..=pos];
                let msg = &line[pos + 2..];

                let msg_color = if line.contains("err") {
                    Color::LightRed
                } else {
                    Color::LightGreen
                };

                Line::from(vec![
                    Span::styled(prefix.to_string(), Style::default().fg(Color::Cyan)),
                    Span::styled(msg.to_string(), Style::default().fg(msg_color)),
                ])
            } else {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(Color::LightGreen),
                ))
            }
        })
        .collect();

    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(90, 140, 220))),
    );

    f.render_widget(widget, area);
}

fn draw_cpu_chart(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let data = if app.cpu_history.is_empty() {
        vec![(0.0, 0.0)]
    } else {
        app.cpu_history.clone()
    };

    let max_x = data.last().map(|(x, _)| *x).unwrap_or(30.0);
    let min_x = if max_x > 30.0 { max_x - 30.0 } else { 0.0 };
    let max_cpu = data.iter().map(|(_, y)| *y).fold(5.0, f64::max);
    let cpu_top = round_axis_top(max_cpu * 1.2);
    let labels = time_labels(min_x, max_x);

    let chart = Chart::new(vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&data),
    ])
    .block(Block::default().title(" CPU ").borders(Borders::ALL))
    .x_axis(Axis::default().bounds([min_x, max_x]).labels(labels))
    .y_axis(Axis::default().bounds([0.0, cpu_top]).labels([
        "0%".into(),
        format!("{:.0}%", cpu_top / 2.0),
        format!("{:.0}%", cpu_top),
    ]));

    f.render_widget(chart, area);
}

fn draw_mem_chart(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let data = if app.mem_history.is_empty() {
        vec![(0.0, 0.0)]
    } else {
        app.mem_history.clone()
    };

    let max_mem = data.iter().map(|(_, y)| *y).fold(50.0, f64::max);
    let max_x = data.last().map(|(x, _)| *x).unwrap_or(30.0);
    let min_x = if max_x > 30.0 { max_x - 30.0 } else { 0.0 };
    let mem_top = round_mem_axis(max_mem);
    let labels = time_labels(min_x, max_x);

    let chart = Chart::new(vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&data),
    ])
    .block(Block::default().title(" Memory ").borders(Borders::ALL))
    .x_axis(Axis::default().bounds([min_x, max_x]).labels(labels))
    .y_axis(Axis::default().bounds([0.0, mem_top]).labels([
        "0".into(),
        format!("{:.0}MB", mem_top / 2.0),
        format!("{:.0}MB", mem_top),
    ]));

    f.render_widget(chart, area);
}

fn time_labels(_min_x: f64, _max_x: f64) -> Vec<String> {
    vec!["-30s".to_string(), "-15s".to_string(), "now".to_string()]
}

fn round_axis_top(value: f64) -> f64 {
    if value <= 10.0 {
        return 10.0;
    }

    let magnitude = 10f64.powf(value.log10().floor());
    let normalized = value / magnitude;

    let rounded = if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };

    rounded * magnitude
}

fn round_mem_axis(value: f64) -> f64 {
    if value <= 20.0 {
        return (value + 2.0).ceil();
    }

    if value <= 50.0 {
        return ((value / 5.0).ceil()) * 5.0;
    }

    ((value / 10.0).ceil()) * 10.0
}

fn draw_controls_panel(f: &mut ratatui::Frame<'_>, area: Rect) {
    let line = Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
        Span::raw(" select  "),
        Span::styled("PgUp/PgDn", Style::default().fg(Color::Cyan)),
        Span::raw(" scroll  "),
        Span::styled("[s]", Style::default().fg(Color::Red)),
        Span::raw(" stop  "),
        Span::styled("[r]", Style::default().fg(Color::Blue)),
        Span::raw(" restart  "),
        Span::styled("[q]", Style::default().fg(Color::Gray)),
        Span::raw(" quit"),
    ]);

    let widget = Paragraph::new(line);

    f.render_widget(widget, area);
}

fn draw_details_panel(f: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();

    if let Some(p) = app.selected_process() {
        lines.push(Line::from(format!("ID: {}", p.id)));
        lines.push(Line::from(format!("Name: {}", p.name)));
        lines.push(Line::from(format!("PID: {}", fmt_pid(p.pid))));
        lines.push(Line::from(format!("Status: {}", status_label(&p.status))));
        lines.push(Line::from(format!("CPU: {}", fmt_cpu(p.cpu))));
        lines.push(Line::from(format!("Memory: {}", fmt_mem(p.mem))));
        lines.push(Line::from(format!("Uptime: {}", fmt_uptime(p.uptime))));
        lines.push(Line::from(format!(
            "User: {}",
            p.user.clone().unwrap_or_else(|| "-".to_string())
        )));
        lines.push(Line::from(format!(
            "Exit: {}",
            p.exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string())
        )));

        lines.push(Line::from(format!("Daemon: {}", app.status_message)));
    }

    let widget = Paragraph::new(lines).block(
        Block::default()
            .title(" Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(widget, area);
}

fn fetch_status_snapshot() -> Result<Vec<ProcessInfo>> {
    let reply = send_secure_command("list")?;
    let mut lines = reply.lines();

    let first = lines.next().unwrap_or("");
    let mut count_hint = 0usize;

    if let Some(rest) = first.strip_prefix("OK ") {
        count_hint = rest.trim().parse::<usize>().unwrap_or(0);
    }

    let mut processes = Vec::with_capacity(count_hint);

    for line in lines {
        if let Some(p) = parse_process(line.as_bytes()) {
            processes.push(p);
        }
    }

    Ok(processes)
}

fn fetch_metrics(process_id: u64, since: u64) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let url = format!(
        "http://localhost:8096/api/v1/get_metrics/{}?since={}",
        process_id, since
    );

    let response = match reqwest::blocking::get(&url) {
        Ok(r) => r,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let text = match response.text() {
        Ok(t) => t,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    parse_metrics(&text)
}

fn parse_metrics(text: &str) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let mut cpu = Vec::new();
    let mut mem = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        if let Some((_, values)) = line.split_once(':') {
            if let Some((c, m)) = values.split_once(',') {
                cpu.push((idx as f64, c.trim().parse().unwrap_or(0.0)));
                mem.push((idx as f64, m.trim().parse::<f64>().unwrap_or(0.0) / 1024.0));
            }
        }
    }

    (cpu, mem)
}

fn current_since() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_sub(30)
}

fn spawn_logs_stream(selected_id: Option<u64>, tx: Sender<String>) -> LogStreamHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);

    thread::spawn(move || {
        let config = Config::load();
        let key = config.key();

        let mut stream = match init_stream() {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(format!("monitor: failed to connect for logs: {e}"));
                return;
            }
        };

        let cmd = if let Some(id) = selected_id {
            format!("logs --lines=40 {}", id)
        } else {
            "logs --lines=40".to_string()
        };

        let token = encrypt_reply_to_token(&key, cmd.as_bytes(), AAD);
        let out_line = format!("ENC {}\n", token);

        if let Err(e) = stream.write_all(out_line.as_bytes()) {
            let _ = tx.send(format!("monitor: failed to send logs command: {e}"));
            return;
        }

        if let Err(e) = stream.flush() {
            let _ = tx.send(format!("monitor: failed to flush logs command: {e}"));
            return;
        }

        let mut reader = BufReader::new(stream);
        let _ = tx.send("logs: stream connected".to_string());

        while !stop_for_thread.load(Ordering::Relaxed) {
            let mut wire_line = String::new();

            match reader.read_line(&mut wire_line) {
                Ok(0) => {
                    let _ = tx.send("logs: stream closed by daemon".to_string());
                    break;
                }
                Ok(_) => {
                    let decrypted = match decrypt_wire_line(&key, &wire_line, AAD) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            let _ = tx.send("logs: decryption error".to_string());
                            continue;
                        }
                    };

                    let mut msg = String::from_utf8_lossy(&decrypted).trim().to_string();

                    if let Some(rest) = msg.strip_prefix("OK ") {
                        msg = rest.to_string();
                    }

                    if msg == "LOGS" || msg == "PING" {
                        continue;
                    }

                    if msg == "EOF" {
                        let _ = tx.send("logs: EOF".to_string());
                        break;
                    }

                    if let Some(rest) = msg.strip_prefix("LOG ") {
                        let mut parts = rest.splitn(3, ' ');
                        let id = parts.next().unwrap_or("?");
                        let stream_name = parts.next().unwrap_or("?");
                        let text = parts.next().unwrap_or("").trim();

                        if !text.is_empty() {
                            let prefix = if stream_name.eq_ignore_ascii_case("stderr") {
                                format!("[{}:{}] ", id, "err")
                            } else {
                                format!("[{}:{}] ", id, "out")
                            };

                            let _ = tx.send(format!("{prefix}{text}"));
                        }

                        continue;
                    }

                    if let Some(err) = msg.strip_prefix("ERR ") {
                        let _ = tx.send(format!("logs error: {err}"));
                        continue;
                    }

                    let _ = tx.send(msg);
                }

                Err(e) => match e.kind() {
                    ErrorKind::TimedOut | ErrorKind::WouldBlock => {}
                    _ => {
                        let _ = tx.send(format!("logs stream read error: {e}"));
                        break;
                    }
                },
            }

            thread::sleep(Duration::from_millis(10));
        }
    });

    LogStreamHandle { stop }
}

fn parse_process(line: &[u8]) -> Option<ProcessInfo> {
    let mut id = 0;
    let mut name: Option<String> = None;
    let mut status = ProcessStatus::Stopped;
    let mut pid = None;
    let mut uptime = None;
    let mut cpu = None;
    let mut mem = None;
    let mut user = None;
    let mut exit_code = None;

    for field in line.split(|b| *b == b'&') {
        let eq = match memchr::memchr(b'=', field) {
            Some(p) => p,
            None => continue,
        };

        let (key, value) = (&field[..eq], &field[eq + 1..]);

        match key {
            b"id" => id = atoi(value),
            b"name" => name = Some(String::from_utf8_lossy(value).into_owned()),
            b"status" => {
                status = match value {
                    b"running" => ProcessStatus::Running,
                    b"exited" => ProcessStatus::Exited,
                    _ => ProcessStatus::Stopped,
                };
            }
            b"pid" => pid = Some(atoi(value) as u32),
            b"uptime" => uptime = Some(atoi(value)),
            b"cpu" => cpu = fast_atof(value),
            b"mem" => mem = Some(atoi(value)),
            b"user" => user = Some(String::from_utf8_lossy(value).into_owned()),
            b"exit_code" => exit_code = std::str::from_utf8(value).ok()?.parse::<i32>().ok(),
            _ => {}
        }
    }

    Some(ProcessInfo {
        id,
        name: name?,
        status,
        pid,
        uptime,
        cpu,
        mem,
        user,
        exit_code,
    })
}

#[inline]
fn atoi(bytes: &[u8]) -> u64 {
    let mut n = 0u64;
    for &b in bytes {
        if !b.is_ascii_digit() {
            break;
        }
        n = n * 10 + (b - b'0') as u64;
    }
    n
}

#[inline]
fn fast_atof(bytes: &[u8]) -> Option<f32> {
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn status_label(s: &ProcessStatus) -> &'static str {
    match s {
        ProcessStatus::Running => "RUNNING",
        ProcessStatus::Stopped => "STOPPED",
        ProcessStatus::Exited => "EXITED",
    }
}

fn status_color(s: &ProcessStatus) -> Color {
    match s {
        ProcessStatus::Running => Color::Green,
        ProcessStatus::Stopped => Color::Yellow,
        ProcessStatus::Exited => Color::Red,
    }
}

fn fmt_uptime(sec: Option<u64>) -> String {
    let mut s = match sec {
        Some(v) => v,
        None => return "-".into(),
    };

    const MIN: u64 = 60;
    const HOUR: u64 = 60 * MIN;
    const DAY: u64 = 24 * HOUR;

    let days = s / DAY;
    s %= DAY;
    let hours = s / HOUR;
    s %= HOUR;
    let minutes = s / MIN;
    let seconds = s % MIN;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

fn fmt_mem(mem: Option<u64>) -> String {
    let bytes = match mem {
        Some(v) => v as f64,
        None => return "-".into(),
    };

    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn fmt_cpu(cpu: Option<f32>) -> String {
    cpu.map(|v| format!("{v:.2}%"))
        .unwrap_or_else(|| "-".to_string())
}

fn fmt_pid(pid: Option<u32>) -> String {
    pid.map(|p| p.to_string())
        .unwrap_or_else(|| "-".to_string())
}
