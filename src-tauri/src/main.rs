mod game;

use game::{Game, UiState};
use std::sync::Mutex;
use tauri::State;

struct AppState(Mutex<Game>);

#[tauri::command]
fn new_game(
    state: State<AppState>,
    seed: Option<u64>,
    enemies: u8,
    difficulty: String,
    size: u8,
) -> UiState {
    let mut g = Game::new(seed, enemies, &difficulty, size);
    g.start_turn(0);
    let s = g.snapshot("Expand! Click your hex, buy a peasant (X).".to_string(), String::new());
    *state.0.lock().unwrap() = g;
    s
}

#[tauri::command]
fn click_hex(state: State<AppState>, x: i32, y: i32) -> UiState {
    let mut g = state.0.lock().unwrap();
    if !g.in_bounds(x, y) {
        return g.snapshot(String::new(), String::new());
    }
    g.focus = Some((x, y));
    let (owner, unit, acted) = {
        let h = &g.grid[Game::idx(g.cols, x, y)];
        (h.owner, h.unit, h.acted)
    };
    let (msg, sfx) = match g.sel {
        None => {
            if owner == 0 && unit != 0 && !acted {
                g.sel = Some((x, y));
                (format!("{} selected — click a glowing hex.", game::rank_name(unit)), "select".to_string())
            } else if owner == 0 && unit != 0 {
                ("Spent — it already acted this turn.".to_string(), String::new())
            } else {
                (String::new(), String::new())
            }
        }
        Some(s) => {
            if s == (x, y) {
                g.sel = None;
                (String::new(), String::new())
            } else {
                let (sx, sy) = s;
                let own_target = g.terr_at(sx, sy)
                    .map(|ti| g.terrs[ti].hexes.contains(&(x, y)))
                    .unwrap_or(false);
                if owner == 0 && own_target {
                    match g.do_move(sx, sy, x, y) {
                        Ok(m) => {
                            let fx = if m.starts_with("Combined") {
                                "combine"
                            } else if m.starts_with("Chopped") {
                                "chop"
                            } else {
                                "move"
                            };
                            let now_acted = g.grid[Game::idx(g.cols, x, y)].acted;
                            g.sel = if now_acted { None } else { Some((x, y)) };
                            (m, fx.to_string())
                        }
                        Err(e) => (e, "error".to_string()),
                    }
                } else {
                    match g.do_attack(sx, sy, x, y, 0) {
                        Ok(m) => {
                            g.sel = None;
                            (m, "attack".to_string())
                        }
                        Err(e) => (e, "error".to_string()),
                    }
                }
            }
        }
    };
    g.snapshot(msg, sfx)
}

#[tauri::command]
fn buy(state: State<AppState>, kind: String) -> UiState {
    let mut g = state.0.lock().unwrap();
    let msg = match g.focus {
        Some((x, y)) => g.buy(x, y, &kind, 0).unwrap_or_else(|e| e),
        None => "Click one of your open hexes first.".to_string(),
    };
    let sfx = if msg == "Peasant ready." {
        "buy".to_string()
    } else if msg == "Castle built." {
        "castle".to_string()
    } else if msg.starts_with("Click") {
        String::new()
    } else {
        "error".to_string()
    };
    g.snapshot(msg, sfx)
}

#[tauri::command]
fn cancel_sel(state: State<AppState>) -> UiState {
    let mut g = state.0.lock().unwrap();
    g.sel = None;
    g.snapshot(String::new(), String::new())
}

#[tauri::command]
fn end_turn(state: State<AppState>) -> UiState {
    let mut g = state.0.lock().unwrap();
    g.sel = None;
    let mut over = false;
    let order: Vec<i8> = g.alive().into_iter().filter(|&o| o != 0).collect();
    for ai in order {
        g.current = ai;
        g.ai_take_turn(ai);
        if g.winner().is_some() {
            over = true;
            break;
        }
    }
    let (msg, sfx) = if over {
        (String::new(), String::new())
    } else {
        g.round += 1;
        g.start_turn(0);
        (format!("Round {}. Treasuries paid out.", g.round), "turn".to_string())
    };
    g.current = 0;
    g.snapshot(msg, sfx)
}

#[tauri::command]
fn rematch(state: State<AppState>, same: bool) -> UiState {
    let (seed, enemies, difficulty, size) = {
        let g = state.0.lock().unwrap();
        let s = if same { Some(g.seed) } else { None };
        (s, g.enemies, g.difficulty.clone(), g.size)
    };
    new_game(state, seed, enemies, difficulty, size)
}

/// Best-effort Omarchy theme detection: finds the active theme name under
/// ~/.config/omarchy and pulls bg/fg/accent out of its terminal theme file.
/// Returns None when Omarchy isn't present or nothing parses — the UI then
/// hides the "Omarchy" theme option.
#[tauri::command]
fn omarchy_theme() -> Option<serde_json::Value> {
    let home = std::env::var("HOME").ok()?;
    let base = format!("{home}/.config/omarchy");
    if !std::path::Path::new(&base).exists() {
        return None;
    }
    let name = ["current/theme", "current/theme-name", "current/theme.txt", "theme"]
        .iter()
        .filter_map(|p| std::fs::read_to_string(format!("{base}/{p}")).ok())
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())?;
    let dir = format!("{base}/themes/{name}");
    let cands = [
        format!("{dir}/alacritty.toml"),
        format!("{dir}/theme.toml"),
        format!("{dir}/ghostty.conf"),
        format!("{dir}/ghostty"),
    ];
    let text = cands.iter().filter_map(|p| std::fs::read_to_string(p).ok()).next()?;
    let mut bg = None;
    let mut fg = None;
    let mut accent = None;
    for line in text.lines() {
        let line = line.trim().trim_start_matches(['*', '-', ' ']);
        let (k, v) = line.split_once(['=', ':'])?;
        let k = k.trim().to_lowercase();
        let v = v.trim().trim_matches(['"', '\'', ' ', '\t']).to_string();
        if !(v.starts_with('#') && (v.len() == 7 || v.len() == 4)) {
            continue;
        }
        if k.contains("background") && bg.is_none() {
            bg = Some(v.clone());
        } else if k.contains("foreground") && fg.is_none() {
            fg = Some(v.clone());
        } else if accent.is_none() && (k == "cyan" || k == "blue" || k.contains("accent") || k == "color14" || k == "color12") {
            accent = Some(v.clone());
        }
    }
    let bg = bg?;
    let fg = fg?;
    Some(serde_json::json!({
        "name": name,
        "bg": bg,
        "fg": fg.clone(),
        "accent": accent.unwrap_or(fg),
    }))
}

fn main() {
    tauri::Builder::default()
        .manage(AppState(Mutex::new(Game::new(None, 2, "normal", 0))))
        .invoke_handler(tauri::generate_handler![
            new_game,
            click_hex,
            buy,
            cancel_sel,
            end_turn,
            rematch,
            omarchy_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running slay");
}
