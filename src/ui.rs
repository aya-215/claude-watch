use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::{Duration, Instant};

use crate::session::Session;

pub struct App {
    sessions: Vec<Session>,
    state: ListState,
    should_quit: bool,
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
}

fn get_status_icon(status: &str) -> &str {
    match status {
        "active" => "🟢",
        "waiting" => "🟡",
        "stopped" => "⚪",
        _ => "❓",
    }
}

fn get_status_label(status: &str) -> &str {
    match status {
        "active" => "実行中",
        "waiting" => "承認待ち",
        "stopped" => "完了",
        _ => "不明",
    }
}

fn get_status_color(status: &str) -> Color {
    match status {
        "active" => Color::Green,
        "waiting" => Color::Yellow,
        "stopped" => Color::Gray,
        _ => Color::White,
    }
}

fn format_cwd(cwd: &str) -> String {
    if let Some(home) = std::env::var("HOME").ok() {
        cwd.replace(&home, "~")
    } else {
        cwd.to_string()
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

fn simplify_notification_message(msg: &str) -> String {
    // "Claude needs your permission to use Bash" -> "Bash許可待ち"
    // "Claude Code needs your approval for the plan" -> "プラン承認待ち"
    if msg.contains("permission to use") {
        if let Some(tool_name) = msg.split("use ").nth(1) {
            return format!("{}許可待ち", tool_name);
        }
    } else if msg.contains("approval for the plan") {
        return "プラン承認待ち".to_string();
    }

    // デフォルトは元のメッセージをそのまま返す
    truncate_text(msg, 40)
}

fn format_relative_time(timestamp_str: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // ISO 8601形式の時刻をパース（簡易版）
    // 例: "2026-01-15T07:08:52.172Z"
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let modified_ts = parsed.timestamp();
        let diff = now - modified_ts;

        if diff < 60 {
            return "たった今".to_string();
        } else if diff < 3600 {
            return format!("{}分前", diff / 60);
        } else if diff < 86400 {
            return format!("{}時間前", diff / 3600);
        } else {
            return format!("{}日前", diff / 86400);
        }
    }

    "不明".to_string()
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // ヘッダー
    let header = Paragraph::new("📋 Claude Code セッション監視")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // セッション一覧
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|session| {
            let icon = get_status_icon(&session.status);
            let status_label = get_status_label(&session.status);
            let cwd = format_cwd(&session.cwd);
            let color = get_status_color(&session.status);

            let mut lines = vec![
                Line::from(vec![
                    Span::raw(format!("{} ", icon)),
                    Span::styled(
                        format!("{:<10}", status_label),
                        Style::default().fg(color),
                    ),
                    Span::raw(format!(" {} ", cwd)),
                    Span::styled(
                        format!("(pane:{})", session.pane_id),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            ];

            // notification_messageがあれば表示
            if let Some(ref msg) = session.notification_message {
                // "Claude needs your permission to use Bash" -> "Bash許可待ち"
                let simplified_msg = simplify_notification_message(msg);
                lines.push(Line::from(vec![
                    Span::raw("   └─ "),
                    Span::styled(
                        simplified_msg,
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
            }

            // summaryまたはfirst_promptがあれば表示
            if let Some(ref summary) = session.summary {
                lines.push(Line::from(vec![
                    Span::raw("   └─ "),
                    Span::styled(
                        format!("\"{}\"", truncate_text(summary, 50)),
                        Style::default().fg(Color::Cyan),
                    ),
                ]));
            } else if let Some(ref first_prompt) = session.first_prompt {
                lines.push(Line::from(vec![
                    Span::raw("   └─ "),
                    Span::styled(
                        format!("\"{}\"", truncate_text(first_prompt, 50)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }

            // メッセージ数、Gitブランチ、最終更新時刻を表示
            let mut meta_parts = vec![];

            if let Some(count) = session.message_count {
                meta_parts.push(format!("{}msg", count));
            }

            if let Some(ref branch) = session.git_branch {
                meta_parts.push(format!("@{}", branch));
            }

            if let Some(ref modified) = session.modified {
                meta_parts.push(format_relative_time(modified));
            }

            if !meta_parts.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("   └─ "),
                    Span::styled(
                        meta_parts.join(" · "),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("セッション一覧 ({})", app.sessions.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[1], &mut app.state);

    // フッター
    let footer_text = if app.sessions.is_empty() {
        "アクティブなセッションがありません | q: 終了"
    } else {
        "↑↓: 選択 | Enter: ジャンプ | q: 終了"
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
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

        // 1秒ごとに自動更新（TODO: Phase 2で実装）
        // if app.last_update.elapsed() >= Duration::from_secs(1) {
        //     let new_sessions = load_and_filter_sessions()?;
        //     app.update_sessions(new_sessions);
        // }
    }

    // ターミナルのクリーンアップ
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(selected_session_id)
}
