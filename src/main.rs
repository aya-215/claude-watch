mod display;
mod session;
mod ui;
mod wezterm;

use anyhow::{anyhow, Result};
use display::display_sessions;
use session::{enrich_sessions_with_index, filter_active_sessions, find_session_by_id, load_sessions};
use ui::run_tui;
use wezterm::jump_to_pane;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let all_sessions = load_sessions()?;
    let mut sessions = filter_active_sessions(all_sessions)?;

    // sessions-index.jsonからsummaryとfirst_promptを取得
    enrich_sessions_with_index(&mut sessions)?;

    if sessions.is_empty() {
        println!("⚠️  アクティブなClaude Codeセッションが見つかりません");
        return Ok(());
    }

    // サブコマンドの処理
    if args.len() >= 2 {
        match args[1].as_str() {
            "jump" => {
                if args.len() < 3 {
                    return Err(anyhow!("使い方: claude-watch jump <session_id>"));
                }
                let session_id = &args[2];
                if let Some(session) = find_session_by_id(&sessions, session_id) {
                    jump_to_pane(&session.pane_id)?;
                } else {
                    return Err(anyhow!("セッションID {} が見つかりません", session_id));
                }
            }
            "list" => {
                // シンプルなリスト表示
                display_sessions(&sessions);
            }
            "tui" | "watch" => {
                // TUIモード
                if let Some(session_id) = run_tui(sessions)? {
                    // Enterが押されたセッションにジャンプ
                    let all_sessions = load_sessions()?;
                    let mut sessions = filter_active_sessions(all_sessions)?;
                    enrich_sessions_with_index(&mut sessions)?;
                    if let Some(session) = find_session_by_id(&sessions, &session_id) {
                        jump_to_pane(&session.pane_id)?;
                    }
                }
            }
            _ => {
                println!("不明なコマンド: {}", args[1]);
                println!("\n使い方:");
                println!("  claude-watch           TUIモードで起動（デフォルト）");
                println!("  claude-watch list      セッション一覧を表示");
                println!("  claude-watch tui       TUIモードで起動");
                println!("  claude-watch jump <id> 指定セッションにジャンプ");
            }
        }
    } else {
        // デフォルト: TUIモード
        if let Some(session_id) = run_tui(sessions)? {
            let all_sessions = load_sessions()?;
            let mut sessions = filter_active_sessions(all_sessions)?;
            enrich_sessions_with_index(&mut sessions)?;
            if let Some(session) = find_session_by_id(&sessions, &session_id) {
                jump_to_pane(&session.pane_id)?;
            }
        }
    }

    Ok(())
}
