use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};

use crate::display::{
    format_cwd, format_relative_time, get_status_color, get_status_icon, get_status_label,
    simplify_notification_message, truncate_text,
};
use crate::session::{enrich_sessions_with_index, filter_active_sessions, load_sessions, Session};

fn load_and_filter_sessions() -> Result<Vec<Session>> {
    let all_sessions = load_sessions()?;
    let mut sessions = filter_active_sessions(all_sessions)?;
    enrich_sessions_with_index(&mut sessions)?;
    Ok(sessions)
}

pub struct App {
    sessions: Vec<Session>,
    state: ListState,
    should_quit: bool,
    last_update: Instant,
}

impl App {
    pub fn new(sessions: Vec<Session>) -> Self {
        let mut state = ListState::default();
        if !sessions.is_empty() {
            state.select(Some(0));
        }

        Self {
            sessions,
            state,
            should_quit: false,
            last_update: Instant::now(),
        }
    }

    pub fn update_sessions(&mut self, sessions: Vec<Session>) {
        let selected = self.state.selected();
        self.sessions = sessions;

        // 選択位置を維持
        if !self.sessions.is_empty() {
            if let Some(idx) = selected {
                if idx >= self.sessions.len() {
                    self.state.select(Some(self.sessions.len() - 1));
                } else {
                    self.state.select(Some(idx));
                }
            } else {
                self.state.select(Some(0));
            }
        } else {
            self.state.select(None);
        }

        self.last_update = Instant::now();
    }

    pub fn next(&mut self) {
        if self.sessions.is_empty() {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.sessions.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.sessions.is_empty() {
            return;
        }

        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.sessions.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.state.selected().and_then(|i| self.sessions.get(i))
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}

fn format_dir_name(cwd: &str) -> &str {
    cwd.rsplit('/').next().unwrap_or(cwd)
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3),  // ヘッダー
            Constraint::Min(0),     // ボディ
            Constraint::Length(1),  // フッター
        ])
        .split(f.area());

    // ボディを左右に分割
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),  // 左ペイン
            Constraint::Percentage(70),  // 右ペイン
        ])
        .split(chunks[1]);

    // ヘッダー
    let header = Paragraph::new("📋 Claude Code セッション監視")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // 左ペイン: セッション一覧（コンパクト）
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|session| {
            let icon = get_status_icon(&session.status);
            let status_label = get_status_label(&session.status);
            let dir_name = format_dir_name(&session.cwd);
            let color = get_status_color(&session.status);

            // 1行: "{icon} {status_label} {dir_name}"
            let line = Line::from(vec![
                Span::raw(format!("{} ", icon)),
                Span::styled(
                    format!("{:<8}", status_label),
                    Style::default().fg(color),
                ),
                Span::raw(format!(" {}", dir_name)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Sessions ({})", app.sessions.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, body[0], &mut app.state);

    // 右ペイン: 選択セッションの詳細
    render_detail(f, body[1], app.selected_session());

    // フッター
    let footer_text = if app.sessions.is_empty() {
        "アクティブなセッションがありません | q: 終了"
    } else {
        "↑↓: 選択 | Enter: ジャンプ | q: 終了"
    };

    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::Gray));
    f.render_widget(footer, chunks[2]);
}

fn render_detail(f: &mut Frame, area: ratatui::layout::Rect, session: Option<&Session>) {
    let Some(session) = session else {
        let text = Paragraph::new("セッションを選択してください")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Detail"));
        f.render_widget(text, area);
        return;
    };

    let icon = get_status_icon(&session.status);
    let status_label = get_status_label(&session.status);
    let color = get_status_color(&session.status);
    let cwd = format_cwd(&session.cwd);

    let mut lines = vec![];

    // ステータス行
    lines.push(Line::from(vec![
        Span::raw(format!("{} ", icon)),
        Span::styled(
            status_label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // パス行
    lines.push(Line::from(vec![
        Span::raw("📁 "),
        Span::styled(cwd, Style::default().fg(Color::White)),
    ]));

    // メタ行
    let mut meta_parts = vec![];
    if let Some(ref branch) = session.git_branch {
        meta_parts.push(format!("🔀 {}", branch));
    }
    if let Some(count) = session.message_count {
        meta_parts.push(format!("📨 {}msg", count));
    }
    if let Some(mem_kb) = session.memory_usage_kb {
        let mem_mb = mem_kb / 1024;
        if mem_mb >= 1024 {
            meta_parts.push(format!("💾 {:.1}GB", mem_mb as f64 / 1024.0));
        } else {
            meta_parts.push(format!("💾 {}MB", mem_mb));
        }
    }
    if let Some(ref modified) = session.modified {
        meta_parts.push(format!("🕐 {}", format_relative_time(modified)));
    }

    if !meta_parts.is_empty() {
        lines.push(Line::from(Span::styled(
            meta_parts.join(" · "),
            Style::default().fg(Color::DarkGray),
        )));
    }

    // 通知行
    if let Some(ref msg) = session.notification_message {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("⚠ "),
            Span::styled(
                simplify_notification_message(msg),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Task
    if let Some(ref first_prompt) = session.first_prompt {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "── Task ──────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            truncate_text(first_prompt, 100),
            Style::default().fg(Color::Cyan),
        )));
    }

    // Summary
    if let Some(ref summary) = session.summary {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "── Summary ───────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            truncate_text(summary, 150),
            Style::default().fg(Color::White),
        )));
    }

    let detail = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Detail"));
    f.render_widget(detail, area);
}

pub fn run_tui(sessions: Vec<Session>) -> Result<Option<String>> {
    // ターミナルのセットアップ
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(sessions);
    let mut selected_session_id: Option<String> = None;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // イベント処理（タイムアウト付き）
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') => {
                        app.quit();
                        break;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.next();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.previous();
                    }
                    KeyCode::Enter => {
                        if let Some(session) = app.selected_session() {
                            selected_session_id = Some(session.session_id.clone());
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }

        // 1秒ごとに自動更新
        if app.last_update.elapsed() >= Duration::from_secs(1) {
            match load_and_filter_sessions() {
                Ok(new_sessions) => {
                    app.update_sessions(new_sessions);
                }
                Err(_) => {
                    // エラー時は更新をスキップ（次回リトライ）
                    app.last_update = Instant::now();
                }
            }
        }
    }

    // ターミナルのクリーンアップ
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(selected_session_id)
}
