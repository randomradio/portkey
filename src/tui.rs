use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use uuid::Uuid;
// use chrono::Utc;

use crate::models::Server;
use crate::vault::Vault;
use crate::ssh;

fn cleanup_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::event::DisableMouseCapture, crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}

// Full TUI application replacing interactive prompts
pub fn run_full_ui(vault: &mut Vault) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let matcher = SkimMatcherV2::default();
    let mut input = String::new();
    let mut selected_idx: usize = 0;
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    // UI modes
    enum Mode { Browse, Filter, Add(AddForm), Edit(EditForm), ConfirmDelete(Uuid), Message(String, Instant) }
    #[derive(Default, Clone)]
    struct AddForm { name: String, host: String, port: String, username: String, password: String, description: String, step: usize }
    #[derive(Clone)]
    struct EditForm { id: Uuid, name: String, host: String, port: String, username: String, password: String, description: String, step: usize }
    let mut mode = Mode::Browse;

    let mut servers: Vec<Server> = vault.list_servers()?.clone();
    let make_filtered = |query: &str, servers_src: &[Server]| -> Vec<(i64, usize)> {
        if query.is_empty() {
            servers_src.iter().enumerate().map(|(i, _)| (0, i)).collect()
        } else {
            let mut scored: Vec<(i64, usize)> = servers_src
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    let hay = format!("{} {} {} {} {}", s.name, s.host, s.username, s.port, s.description.as_deref().unwrap_or(""));
                    matcher.fuzzy_match(&hay, query).map(|score| (score, i))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            scored
        }
    };
    let mut filtered: Vec<(i64, usize)> = make_filtered("", &servers);
    if filtered.is_empty() { selected_idx = 0; }

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // header
                    Constraint::Length(3), // filter/input
                    Constraint::Min(1),    // list
                    Constraint::Length(1), // footer
                ])
                .split(size);

            // Header
            let header = Paragraph::new("Portkey — / filter | a add | Enter connect | x delete | q quit")
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(header, chunks[0]);

            // Input area (filter or add)
            let (title, text): (String, String) = match &mode {
                Mode::Filter => ("Filter (type text, Enter to apply)".to_string(), input.clone()),
                Mode::Add(form) => {
                    let label = match form.step { 0 => "Name", 1 => "Host", 2 => "Port", 3 => "Username", 4 => "Password", 5 => "Description", _ => "" };
                    let current = match form.step { 0 => &form.name, 1 => &form.host, 2 => &form.port, 3 => &form.username, 4 => &form.password, 5 => &form.description, _ => &form.name };
                    (format!("Add server — {}:", label), current.clone())
                }
                Mode::Edit(form) => {
                    let label = match form.step { 0 => "Name", 1 => "Host", 2 => "Port", 3 => "Username", 4 => "Password", 5 => "Description", _ => "" };
                    let current = match form.step { 0 => &form.name, 1 => &form.host, 2 => &form.port, 3 => &form.username, 4 => &form.password, 5 => &form.description, _ => &form.name };
                    (format!("Edit server — {}:", label), current.clone())
                }
                Mode::Message(msg, _) => ("Message".to_string(), msg.clone()),
                _ => ("Filter (press / to edit)".to_string(), input.clone()),
            };
            let input_widget = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(title));
            f.render_widget(input_widget, chunks[1]);

            // List
            let items: Vec<ListItem> = if filtered.is_empty() {
                vec![ListItem::new(Line::from(vec![Span::raw("No matches")]))]
            } else {
                filtered
                    .iter()
                    .map(|(_, idx)| {
                        let s = &servers[*idx];
                        let line = format!("{} | {}@{}:{}", s.name, s.username, s.host, s.port);
                        ListItem::new(Line::from(vec![Span::raw(line)]))
                    })
                    .collect()
            };
            let mut state = ratatui::widgets::ListState::default();
            if !filtered.is_empty() { state.select(Some(selected_idx)); }
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Servers"))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));
            f.render_stateful_widget(list, chunks[2], &mut state);

            // Footer
            let footer = Paragraph::new("d delete | e export ssh-config (CLI) | ? help")
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(footer, chunks[3]);
        })?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match &mut mode {
                        Mode::Browse => match key.code {
                            KeyCode::Char('/') => { mode = Mode::Filter; }
                            KeyCode::Char('a') => { mode = Mode::Add(AddForm::default()); }
                            KeyCode::Char('e') => {
                                if let Some((_, idx)) = filtered.get(selected_idx) {
                                    let s = &servers[*idx];
                                    let form = EditForm {
                                        id: s.id,
                                        name: s.name.clone(),
                                        host: s.host.clone(),
                                        port: s.port.to_string(),
                                        username: s.username.clone(),
                                        password: s.password.clone(),
                                        description: s.description.clone().unwrap_or_default(),
                                        step: 0,
                                    };
                                    mode = Mode::Edit(form);
                                }
                            }
                            KeyCode::Char('x') | KeyCode::Char('d') => {
                                if let Some((_, idx)) = filtered.get(selected_idx) { mode = Mode::ConfirmDelete(servers[*idx].id); }
                            }
                            KeyCode::Up => { if !filtered.is_empty() { selected_idx = selected_idx.saturating_sub(1); } }
                            KeyCode::Down => { if !filtered.is_empty() { selected_idx = (selected_idx + 1).min(filtered.len().saturating_sub(1)); } }
                            KeyCode::Enter => {
                                if let Some((_, idx)) = filtered.get(selected_idx) {
                                    // Suspend TUI, run SSH, restore
                                    cleanup_terminal()?;
                                    let _ = ssh::connect(&servers[*idx]);
                                    // Re-init terminal
                                    enable_raw_mode()?;
                                    let mut stdout = io::stdout();
                                    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
                                    // Reload servers in case of changes
                                    servers = vault.list_servers()?.clone();
                                    filtered = make_filtered(&input, &servers);
                                    if filtered.is_empty() { selected_idx = 0; } else if selected_idx >= filtered.len() { selected_idx = filtered.len() - 1; }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => { cleanup_terminal()?; return Ok(()); }
                            _ => {}
                        },
                        Mode::Filter => match key.code {
                            KeyCode::Enter => { mode = Mode::Browse; }
                            KeyCode::Esc => { input.clear(); filtered = make_filtered("", &servers); if filtered.is_empty() { selected_idx = 0; } mode = Mode::Browse; }
                            KeyCode::Backspace => { input.pop(); filtered = make_filtered(&input, &servers); if filtered.is_empty() { selected_idx = 0; } else if selected_idx >= filtered.len() { selected_idx = filtered.len() - 1; } }
                            KeyCode::Delete => { input.clear(); filtered = make_filtered("", &servers); if filtered.is_empty() { selected_idx = 0; } }
                            KeyCode::Up => { if !filtered.is_empty() { selected_idx = selected_idx.saturating_sub(1); } }
                            KeyCode::Down => { if !filtered.is_empty() { selected_idx = (selected_idx + 1).min(filtered.len().saturating_sub(1)); } }
                            KeyCode::Char(c) => { input.push(c); filtered = make_filtered(&input, &servers); if filtered.is_empty() { selected_idx = 0; } else if selected_idx >= filtered.len() { selected_idx = filtered.len() - 1; } }
                            _ => {}
                        },
                        Mode::Add(form) => match key.code {
                            KeyCode::Esc => { mode = Mode::Browse; }
                            KeyCode::Enter => {
                                form.step += 1;
                                if form.step > 5 {
                                    // finalize and add
                                    let port: u16 = form.port.parse().unwrap_or(22);
                                    let server = Server::new(
                                        form.name.clone(),
                                        form.host.clone(),
                                        port,
                                        form.username.clone(),
                                        form.password.clone(),
                                        if form.description.is_empty() { None } else { Some(form.description.clone()) },
                                    );
                                    if let Err(e) = vault.add_server(server) { mode = Mode::Message(format!("Add failed: {}", e), Instant::now()); } else {
                                        servers = vault.list_servers()?.clone();
                                        filtered = make_filtered(&input, &servers);
                                        if filtered.is_empty() { selected_idx = 0; } else if selected_idx >= filtered.len() { selected_idx = filtered.len() - 1; }
                                        mode = Mode::Message("Server added".to_string(), Instant::now());
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                let target = match form.step { 0 => &mut form.name, 1 => &mut form.host, 2 => &mut form.port, 3 => &mut form.username, 4 => &mut form.password, 5 => &mut form.description, _ => &mut form.name };
                                target.pop();
                            }
                            KeyCode::Delete => {
                                let target = match form.step { 0 => &mut form.name, 1 => &mut form.host, 2 => &mut form.port, 3 => &mut form.username, 4 => &mut form.password, 5 => &mut form.description, _ => &mut form.name };
                                target.clear();
                            }
                            KeyCode::Char(c) => {
                                let target = match form.step { 0 => &mut form.name, 1 => &mut form.host, 2 => &mut form.port, 3 => &mut form.username, 4 => &mut form.password, 5 => &mut form.description, _ => &mut form.name };
                                target.push(c);
                            }
                            _ => {}
                        },
                        Mode::Edit(form) => match key.code {
                            KeyCode::Esc => { mode = Mode::Browse; }
                            KeyCode::Enter => {
                                form.step += 1;
                                if form.step > 5 {
                                    // finalize and update
                                    let port: u16 = form.port.parse().unwrap_or(22);
                                    // find original
                                    if let Some(pos) = servers.iter().position(|s| s.id == form.id) {
                                        let mut updated = servers[pos].clone();
                                        updated.update_fields(
                                            form.name.clone(),
                                            form.host.clone(),
                                            port,
                                            form.username.clone(),
                                            form.password.clone(),
                                            if form.description.is_empty() { None } else { Some(form.description.clone()) },
                                        );
                                        match vault.replace_server(updated) {
                                            Ok(true) => {
                                                servers = vault.list_servers()?.clone();
                                                filtered = make_filtered(&input, &servers);
                                                if filtered.is_empty() { selected_idx = 0; } else if selected_idx >= filtered.len() { selected_idx = filtered.len() - 1; }
                                                mode = Mode::Message("Server updated".to_string(), Instant::now());
                                            }
                                            Ok(false) => { mode = Mode::Message("Server not found".to_string(), Instant::now()); }
                                            Err(e) => { mode = Mode::Message(format!("Update failed: {}", e), Instant::now()); }
                                        }
                                    } else {
                                        mode = Mode::Message("Server not found".to_string(), Instant::now());
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                let target = match form.step { 0 => &mut form.name, 1 => &mut form.host, 2 => &mut form.port, 3 => &mut form.username, 4 => &mut form.password, 5 => &mut form.description, _ => &mut form.name };
                                target.pop();
                            }
                            KeyCode::Delete => {
                                let target = match form.step { 0 => &mut form.name, 1 => &mut form.host, 2 => &mut form.port, 3 => &mut form.username, 4 => &mut form.password, 5 => &mut form.description, _ => &mut form.name };
                                target.clear();
                            }
                            KeyCode::Char(c) => {
                                let target = match form.step { 0 => &mut form.name, 1 => &mut form.host, 2 => &mut form.port, 3 => &mut form.username, 4 => &mut form.password, 5 => &mut form.description, _ => &mut form.name };
                                target.push(c);
                            }
                            _ => {}
                        },
                        Mode::ConfirmDelete(id) => match key.code {
                            KeyCode::Char('y') => {
                                let _ = vault.remove_server(id);
                                servers = vault.list_servers()?.clone();
                                filtered = make_filtered(&input, &servers);
                                if filtered.is_empty() { selected_idx = 0; } else if selected_idx >= filtered.len() { selected_idx = filtered.len() - 1; }
                                mode = Mode::Browse;
                            }
                            KeyCode::Char('n') | KeyCode::Esc => { mode = Mode::Browse; }
                            _ => {}
                        },
                        Mode::Message(_, since) => {
                            // any key returns to browse
                            *since = Instant::now();
                            mode = Mode::Browse;
                        }
                    }
                }
            }
        }

        // auto-clear transient messages
        if let Mode::Message(_, t) = &mode {
            if t.elapsed() > Duration::from_secs(2) { mode = Mode::Browse; }
        }

        if last_tick.elapsed() >= tick_rate { last_tick = Instant::now(); }
    }
}
