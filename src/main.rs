//! Motus kawaii — Ratatui
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph},
    Frame, Terminal,
};
use rand::seq::SliceRandom;
use std::{
    collections::HashMap,
    io::stdout,
    process::Command,
    time::{Duration, Instant},
};

const MAX_TRIES: usize = 6;
const LEVELS: [(usize, usize); 5] = [(5, 2), (6, 1), (7, 0), (8, 0), (9, 0)];

struct App {
    menu: bool,
    level: usize,
    secret: Vec<char>,
    len: usize,
    grid: Vec<Vec<char>>,
    states: Vec<Vec<u8>>, // 0=miss 1=present 2=good
    current: Vec<char>,
    locked: Vec<bool>,
    cursor: usize,
    over: bool,
    won: bool,
    message: String,
    def: String,
    dark: bool,
    dict: HashMap<usize, Vec<String>>,
    secrets: HashMap<usize, Vec<String>>,
}

fn detect_dark() -> bool {
    // COLORFGBG="15;0" → fond sombre ; défaut : sombre
    if let Ok(v) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = v.split(';').collect();
        if parts.len() >= 2 {
            if let Ok(bg) = parts[parts.len() - 1].parse::<u8>() {
                return bg == 0 || bg == 8;
            }
        }
    }
    true
}

impl App {
    fn new() -> Self {
        let mut dict: HashMap<usize, Vec<String>> = HashMap::new();
        let mut secrets: HashMap<usize, Vec<String>> = HashMap::new();
        // embarqué dans le binaire (include_str! à la compilation)
        let mots_embed = include_str!("../data/mots.txt");
        let secrets_embed = include_str!("../data/secrets.txt");
        for (s, m) in [(mots_embed, &mut dict), (secrets_embed, &mut secrets)] {
            for w in s.lines() {
                let w = w.trim();
                if (5..=9).contains(&w.len()) {
                    m.entry(w.len()).or_default().push(w.to_string());
                }
            }
        }
        App {
            menu: true,
            level: 0,
            secret: Vec::new(),
            len: 5,
            grid: Vec::new(),
            states: Vec::new(),
            current: Vec::new(),
            locked: Vec::new(),
            cursor: 0,
            over: false,
            won: false,
            message: String::new(),
            def: String::new(),
            dark: detect_dark(),
            dict,
            secrets,
        }
    }

    fn start(&mut self, level: usize) {
        self.level = level;
        self.len = LEVELS[level].0;
        let pool = self.secrets.get(&self.len).cloned().unwrap_or_default();
        let mut rng = rand::thread_rng();
        match pool.choose(&mut rng) {
            Some(w) => self.secret = w.chars().collect(),
            None => {
                self.message = "Dictionnaire introuvable (＞﹏＜".into();
                self.menu = true;
                return;
            }
        }
        self.grid.clear();
        self.states.clear();
        self.current = vec!['\0'; self.len];
        self.current[0] = self.secret[0];
        self.locked = vec![false; self.len];
        self.locked[0] = true;
        self.cursor = self.first_free();
        self.over = false;
        self.won = false;
        self.def.clear();
        self.message = format!("Mot de {} lettres · entre 1 et {} ♪", self.len, MAX_TRIES);
        self.menu = false;
    }

    fn first_free(&self) -> usize {
        (0..self.len).find(|&i| !self.locked[i]).unwrap_or(self.len)
    }
    fn next_free_after(&self, from: usize) -> usize {
        (from + 1..self.len).find(|&i| !self.locked[i]).unwrap_or(self.len)
    }
    fn prev_free_before(&self, from: usize) -> usize {
        (0..=from)
            .rev()
            .find(|&i| !self.locked[i])
            .unwrap_or_else(|| self.first_free())
    }

    fn type_rune(&mut self, r: char) {
        if self.over || self.cursor >= self.len || self.locked[self.cursor] {
            return;
        }
        self.current[self.cursor] = r;
        self.cursor = self.next_free_after(self.cursor);
        self.message = "Entrée pour valider ♪".into();
    }

    fn backspace(&mut self) {
        if self.cursor < self.len && !self.locked[self.cursor] && self.current[self.cursor] != '\0' {
            self.current[self.cursor] = '\0';
            return;
        }
        for c in (0..self.cursor).rev() {
            if !self.locked[c] && self.current[c] != '\0' {
                self.current[c] = '\0';
                self.cursor = c;
                return;
            }
        }
    }

    fn score(&self, guess: &[char]) -> (Vec<u8>, Vec<bool>) {
        let mut fb = vec![0u8; self.len];
        let mut locked = vec![false; self.len];
        let mut rem: Vec<char> = self.secret.clone();
        for i in 0..self.len {
            if guess[i] == self.secret[i] {
                fb[i] = 2;
                locked[i] = true;
                if let Some(p) = rem.iter().position(|&r| r == guess[i]) {
                    rem.remove(p);
                }
            }
        }
        for i in 0..self.len {
            if fb[i] == 2 {
                continue;
            }
            if let Some(p) = rem.iter().position(|&r| r == guess[i]) {
                fb[i] = 1;
                rem.remove(p);
            }
        }
        (fb, locked)
    }

    fn submit(&mut self) {
        if self.over || self.cursor < self.len {
            return;
        }
        let word: String = self.current.iter().collect();
        if !self.dict.get(&self.len).map_or(false, |v| v.contains(&word)) {
            self.message = format!("{} n'est pas dans mon dictionnaire... (＞﹏＜", word);
            return;
        }
        let guess: Vec<char> = self.current.clone();
        let (fb, locked) = self.score(&guess);
        self.grid.push(guess.clone());
        self.states.push(fb);
        if word == self.secret.iter().collect::<String>() {
            self.over = true;
            self.won = true;
            self.message = format!(
                "GAGNÉ ! {} essai(s) d'avance ☆*:.｡.o(≧▽≦)o.｡.:*☆",
                MAX_TRIES - self.grid.len()
            );
            self.fetch_def(&word);
            return;
        }
        if self.grid.len() >= MAX_TRIES {
            self.over = true;
            self.message = format!(
                "Perdu ! Le mot était {} (´；ω；`)",
                self.secret.iter().collect::<String>().to_uppercase()
            );
            self.fetch_def(&word);
            return;
        }
        self.locked = locked;
        self.current = vec!['\0'; self.len];
        for i in 0..self.len {
            if self.locked[i] {
                self.current[i] = self.secret[i];
            }
        }
        self.cursor = self.first_free();
        self.message = "Raté ! Une lettre en plus... ♪".into();
    }

    fn fetch_def(&mut self, word: &str) {
        // curl en subprocess : pas de dépendance TLS
        let url = format!(
            "https://fr.wiktionary.org/w/api.php?action=parse&prop=wikitext&page={}&format=json&formatversion=2&redirects=1",
            word
        );
        if let Ok(out) = Command::new("curl")
            .args(["-s", "--max-time", "8", "-A", "motus-kawaii/1.0", &url])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            // extraire "wikitext":"..." puis parser les lignes # 
            if let Some(p) = s.find("\"wikitext\":\"") {
                let rest = &s[p + 12..];
                let wikitext = rest
                    .strip_suffix("\"}") // fin du json
                    .unwrap_or(rest.split_to_quote().unwrap_or(rest));
                let wikitext = unescape_json(wikitext);
                let mut defs = Vec::new();
                let mut in_fr = false;
                for l in wikitext.lines() {
                    if l.starts_with("== {{langue|fr}} ==") {
                        in_fr = true;
                    } else if in_fr && l.starts_with("== {{") {
                        break;
                    } else if in_fr && l.starts_with("# ") {
                        defs.push(clean_wiki(&l[2..]));
                    }
                }
                self.def = defs.join("\n");
            }
        }
    }

    fn colors(&self) -> (Color, Color, Color, Color) {
        if self.dark {
            (
                Color::Rgb(255, 123, 138),
                Color::Rgb(123, 184, 255),
                Color::Rgb(106, 106, 106),
                Color::Rgb(221, 221, 221),
            )
        } else {
            (
                Color::Rgb(179, 36, 60),
                Color::Rgb(31, 95, 168),
                Color::Rgb(138, 138, 138),
                Color::Rgb(68, 68, 68),
            )
        }
    }
}

trait SplitQuote {
    fn split_to_quote(&self) -> Option<&str>;
}
impl SplitQuote for str {
    fn split_to_quote(&self) -> Option<&str> {
        let mut esc = false;
        for (i, c) in self.char_indices() {
            if esc {
                esc = false;
                continue;
            }
            match c {
                '\\' => esc = true,
                '"' => return Some(&self[..i]),
                _ => {}
            }
        }
        None
    }
}

fn unescape_json(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('u') => {
                    let hex: String = it.by_ref().take(4).collect();
                    if let Ok(n) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(n) {
                            out.push(ch);
                        }
                    }
                }
                other => {
                    if let Some(o) = other {
                        out.push(o);
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn clean_wiki(s: &str) -> String {
    let mut s = s.replace("'''", "").replace("''", "");
    while let Some(a) = s.find("{{") {
        if let Some(b) = s[a..].find("}}") {
            s = format!("{}{}", &s[..a], &s[a + b + 2..]);
        } else {
            break;
        }
    }
    while let Some(a) = s.find("[[") {
        if let Some(b) = s[a..].find("]]") {
            let inner = &s[a + 2..a + b];
            let inner = inner.split('|').last().unwrap_or(inner);
            s = format!("{}{}{}", &s[..a], inner, &s[a + b + 2..]);
        } else {
            break;
        }
    }
    s
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let (good, pres, miss, given) = app.colors();
    let accent = if app.dark { Color::Rgb(255, 158, 196) } else { Color::Rgb(214, 72, 140) };
    let title_s = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let msg_s = Style::default().fg(if app.dark { Color::Rgb(187, 187, 187) } else { Color::Rgb(85, 85, 85) });
    let help_s = Style::default().fg(Color::DarkGray);

    if app.menu {
        let mut lines = vec![
            Line::from(Span::styled(" ♡  M O T U S  ♡ ", title_s)),
            Line::from(""),
            Line::from(Span::styled("Choisis ta longueur :", msg_s)),
            Line::from(""),
        ];
        for (i, (n, _hints)) in LEVELS.iter().enumerate() {
            let style = if i == app.level {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                msg_s
            };
            lines.push(Line::from(Span::styled(
                format!("  [{}] {} lettres · {} essais", i + 1, n, MAX_TRIES),
                style,
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "↑↓ choisir · entrée jouer · 1-5 direct · q quitter",
            help_s,
        )));
        let p = Paragraph::new(lines).alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    // grille dessinée : colonnes de 6 (border chars autour de cellules de 5)
    let cw = 6usize; // 1 bordure + 5
    let gw = app.len * cw + 1;
    let gh = MAX_TRIES * 2 + 1;
    let col = (area.width as usize).saturating_sub(gw) / 2;
    let row = (area.height as usize).saturating_sub(gh + 6) / 2 + 2;

    // titre + HUD
    let hud = if app.over {
        vec![
            Line::from(Span::styled("♡  M O T U S  ♡", title_s)).alignment(Alignment::Center),
            Line::from(""),
            Line::from(Span::styled(app.message.clone(), msg_s)).alignment(Alignment::Center),
        ]
    } else {
        vec![
            Line::from(Span::styled("♡  M O T U S  ♡", title_s)).alignment(Alignment::Center),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "mot de {} lettres · essai {}/{}",
                    app.len,
                    app.grid.len() + 1,
                    MAX_TRIES
                ),
                msg_s,
            ))
            .alignment(Alignment::Center),
        ]
    };
    let hud_area = Rect { x: 0, y: 1, width: area.width, height: 3 };
    f.render_widget(Paragraph::new(hud), hud_area);

    // bordures de la grille
    let border_s = Style::default().fg(Color::Gray);
    let seg = "─".repeat(cw - 1);
    for r in 0..gh {
        let y = (row + r) as u16;
        if y >= area.height {
            break;
        }
        let mut line = String::new();
        if col > 0 {
            line.push_str(&" ".repeat(col));
        }
        if r % 2 == 0 {
            // ligne horizontale
            let left = if r == 0 { '┌' } else if r == gh - 1 { '└' } else { '├' };
            let mid = if r == 0 { '┬' } else if r == gh - 1 { '┴' } else { '┼' };
            let right = if r == 0 { '┐' } else if r == gh - 1 { '┘' } else { '┤' };
            line.push(left);
            for c in 0..app.len {
                if c > 0 {
                    line.push(mid);
                }
                line.push_str(&seg);
            }
            line.push(right);
        } else {
            // ligne de cellules : bords verticaux seulement
            line.push('│');
            for c in 0..app.len {
                line.push_str(&" ".repeat(cw - 1));
                line.push('│');
            }
        }
        f.render_widget(
            Line::from(Span::styled(line, border_s)),
            Rect { x: 0, y, width: area.width, height: 1 },
        );
    }

    // cellules (lignes impaires)
    let vline_y = row; // top border row index
    for gr in 0..MAX_TRIES {
        let r = 2 * gr + 1;
        let y = (row + r) as u16;
        if y >= area.height {
            break;
        }
        let mut spans: Vec<Span> = Vec::new();
        if col > 0 {
            spans.push(Span::raw(" ".repeat(col)));
        }
        spans.push(Span::styled("│", border_s));
        for c in 0..app.len {
            let mut ch = '\0';
            let mut st = Style::default().fg(miss);
            if gr < app.grid.len() {
                ch = app.grid[gr][c];
                st = match app.states[gr][c] {
                    2 => Style::default().fg(good).add_modifier(Modifier::BOLD),
                    1 => Style::default().fg(pres).add_modifier(Modifier::BOLD),
                    _ => Style::default().fg(miss),
                };
            } else if gr == app.grid.len() {
                ch = app.current[c];
                if ch == '\0' && c == app.cursor && !app.over {
                    ch = '_';
                }
                st = if app.locked[c] {
                    Style::default().fg(given).add_modifier(Modifier::BOLD)
                } else if c == app.cursor && !app.over {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(miss)
                };
            }
            let txt = if ch == '\0' {
                "  ·  ".to_string()
            } else { format!("  {}  ", ch) };
            spans.push(Span::styled(txt, st));
            spans.push(Span::styled("│", border_s));
        }
        f.render_widget(Line::from(spans), Rect { x: 0, y, width: area.width, height: 1 });
        let _ = vline_y;
    }

    // messages bas
    let bottom = (row + gh) as u16 + 1;
    let mut lines = vec![
        Line::from(Span::styled(app.message.clone(), msg_s)).alignment(Alignment::Center),
        Line::from(Span::styled(
            if app.over {
                "[n] rejouer · esc menu · q quitter".to_string()
            } else {
                "←→ bouger · backspace · entrée · esc menu".to_string()
            },
            help_s,
        ))
        .alignment(Alignment::Center),
    ];
    if !app.def.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("📖 Définition", msg_s)).alignment(Alignment::Center));
        for d in app.def.lines().take(4) {
            lines.push(Line::from(Span::styled(d.to_string(), msg_s)).alignment(Alignment::Center));
        }
    }
    let bh = (area.height.saturating_sub(bottom)).min(6);
    if bh > 0 {
        f.render_widget(
            Paragraph::new(lines),
            Rect { x: 0, y: bottom, width: area.width, height: bh },
        );
    }
    let _ = gw;
}

fn main() -> std::io::Result<()> {
    let mut app = App::new();
    let mut term = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))?;
    crossterm::terminal::enable_raw_mode()?;
    let mut out = stdout();
    crossterm::execute!(out, crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    term.clear()?;

    let res = run_app(&mut term, &mut app);

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        &mut term.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    term.show_cursor()?;
    res
}

fn run_app(
    term: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> std::io::Result<()> {
    loop {
        term.draw(|f| ui(f, app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                    return Ok(());
                }
                match k.code {
                    KeyCode::Char('q') if app.menu => return Ok(()),
                    KeyCode::Esc if !app.menu => app.menu = true,
                    _ => {}
                }
                if app.menu {
                    match k.code {
                        KeyCode::Char(c @ '1'..='5') => {
                            app.start(c as usize - '1' as usize)
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Right | KeyCode::Char('l') => {
                            app.level = (app.level + 1) % LEVELS.len()
                        }
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Left | KeyCode::Char('h') => {
                            app.level = (app.level + LEVELS.len() - 1) % LEVELS.len()
                        }
                        KeyCode::Enter => app.start(app.level),
                        _ => {}
                    }
                } else {
                    match k.code {
                        KeyCode::Char('q') if app.over => return Ok(()),
                        KeyCode::Char('n') if app.over => app.start(app.level),
                        KeyCode::Char('n') => app.type_rune('n'),
                        KeyCode::Left => app.cursor = app.prev_free_before(app.cursor),
                        KeyCode::Right => app.cursor = app.next_free_after(app.cursor),
                        KeyCode::Backspace => app.backspace(),
                        KeyCode::Enter => app.submit(),
                        KeyCode::Char(c) if c.is_ascii_lowercase() => app.type_rune(c),
                        _ => {}
                    }
                }
            }
        }
    }
}

// silencieux les imports non utilisés éventuels
#[allow(dead_code)]
fn _now() -> Instant { Instant::now() }
#[allow(dead_code)]
fn _layout_check(c: Constraint) -> Constraint { c }
