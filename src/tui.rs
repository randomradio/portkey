#![allow(clippy::collapsible_match)]

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
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

use crate::models::Server;
use crate::ssh;
use crate::vault::Vault;

fn cleanup_terminal(inside_tmux: bool) -> io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    if !inside_tmux {
        crossterm::execute!(stdout, crossterm::event::DisableMouseCapture)?;
    }
    crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}

// Full TUI application replacing interactive prompts
pub fn run_full_ui(vault: &mut Vault) -> anyhow::Result<()> {
    let inside_tmux = std::env::var("TMUX").is_ok();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    if !inside_tmux {
        crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let matcher = SkimMatcherV2::default();
    let mut input = String::new();
    let mut selected_idx: usize = 0;
    // 200ms tick rate: provides responsive UI updates while being long enough
    // for crossterm to assemble multi-byte escape sequences from tmux.
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    // Persistent list state so scroll offset is preserved across frames
    let mut list_state = ratatui::widgets::ListState::default();

    let clamp_selection = |idx: &mut usize, len: usize| {
        if len == 0 {
            *idx = 0;
        } else if *idx >= len {
            *idx = len - 1;
        }
    };

    // UI modes
    enum Mode {
        Browse,
        Filter,
        Add(AddForm),
        Edit(EditForm),
        ConfirmDelete(Uuid),
        Message(String, Instant),
    }
    #[derive(Default, Clone)]
    struct AddForm {
        name: String,
        host: String,
        port: String,
        username: String,
        password: String,
        identity_file: String,
        forward_agent: bool,
        description: String,
        step: usize,
    }
    #[derive(Clone)]
    struct EditForm {
        id: Uuid,
        name: String,
        host: String,
        port: String,
        username: String,
        password: String,
        identity_file: String,
        forward_agent: bool,
        description: String,
        step: usize,
    }
    let mut mode = Mode::Browse;

    let mut servers: Vec<Server> = vault.list_servers()?.clone();
    let make_filtered = |query: &str, servers_src: &[Server]| -> Vec<(i64, usize)> {
        if query.is_empty() {
            servers_src
                .iter()
                .enumerate()
                .map(|(i, _)| (0, i))
                .collect()
        } else {
            let mut scored: Vec<(i64, usize)> = servers_src
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    let hay = format!(
                        "{} {} {} {} {}",
                        s.name,
                        s.host,
                        s.username,
                        s.port,
                        s.description.as_deref().unwrap_or("")
                    );
                    matcher.fuzzy_match(&hay, query).map(|score| (score, i))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            scored
        }
    };
    let mut filtered: Vec<(i64, usize)> = make_filtered("", &servers);
    clamp_selection(&mut selected_idx, filtered.len());

    loop {
        // Sync selection to persistent list_state before drawing
        list_state.select(if filtered.is_empty() {
            None
        } else {
            Some(selected_idx)
        });

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
            let header = Paragraph::new(
                "Portkey -- / filter | a add | e edit | Enter connect | j/k navigate | q quit",
            )
            .block(Block::default().borders(Borders::NONE));
            f.render_widget(header, chunks[0]);

            // Input area (filter or add)
            let (title, text): (String, String) = match &mode {
                Mode::Filter => (
                    "Filter (type text, Enter to apply)".to_string(),
                    input.clone(),
                ),
                Mode::Add(form) => {
                    let label = match form.step {
                        0 => "Name",
                        1 => "Host",
                        2 => "Port",
                        3 => "Username",
                        4 => "Password",
                        5 => "Identity file",
                        6 => "Forward agent (y/n)",
                        7 => "Description",
                        _ => "",
                    };
                    let current = match form.step {
                        0 => form.name.clone(),
                        1 => form.host.clone(),
                        2 => form.port.clone(),
                        3 => form.username.clone(),
                        4 => "*".repeat(form.password.chars().count()),
                        5 => form.identity_file.clone(),
                        6 => {
                            if form.forward_agent {
                                "yes".to_string()
                            } else {
                                "no".to_string()
                            }
                        }
                        7 => form.description.clone(),
                        _ => form.name.clone(),
                    };
                    (
                        format!("Add server -- {label} (Shift+Tab to go back):"),
                        current,
                    )
                }
                Mode::Edit(form) => {
                    let label = match form.step {
                        0 => "Name",
                        1 => "Host",
                        2 => "Port",
                        3 => "Username",
                        4 => "Password (blank keeps existing)",
                        5 => "Identity file",
                        6 => "Forward agent (y/n)",
                        7 => "Description",
                        _ => "",
                    };
                    let current = match form.step {
                        0 => form.name.clone(),
                        1 => form.host.clone(),
                        2 => form.port.clone(),
                        3 => form.username.clone(),
                        4 => "*".repeat(form.password.chars().count()),
                        5 => form.identity_file.clone(),
                        6 => {
                            if form.forward_agent {
                                "yes".to_string()
                            } else {
                                "no".to_string()
                            }
                        }
                        7 => form.description.clone(),
                        _ => form.name.clone(),
                    };
                    (
                        format!("Edit server -- {label} (Shift+Tab to go back):"),
                        current,
                    )
                }
                Mode::Message(msg, _) => ("Message".to_string(), msg.clone()),
                Mode::ConfirmDelete(_) => (
                    "Confirm Delete".to_string(),
                    "Press 'y' to confirm, 'n' or Esc to cancel".to_string(),
                ),
                _ => ("Filter (press / to edit)".to_string(), input.clone()),
            };
            let input_widget =
                Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(title));
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
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Servers"))
                .highlight_style(
                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
                );
            f.render_stateful_widget(list, chunks[2], &mut list_state);

            // Footer
            let footer_text = match &mode {
                Mode::ConfirmDelete(_) => "y=YES | n=NO (or Esc to cancel)",
                _ => "d delete | PgUp/PgDn scroll | Home/End jump | Ctrl+C force quit",
            };
            let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::NONE));
            f.render_widget(footer, chunks[3]);
        })?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Global Ctrl+C: emergency exit from any mode
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        cleanup_terminal(inside_tmux)?;
                        return Ok(());
                    }

                    match &mut mode {
                        Mode::Browse => match key.code {
                            KeyCode::Char('/') => {
                                mode = Mode::Filter;
                            }
                            KeyCode::Char('a') => {
                                mode = Mode::Add(AddForm::default());
                            }
                            KeyCode::Char('e') => {
                                if let Some((_, idx)) = filtered.get(selected_idx) {
                                    let s = &servers[*idx];
                                    let form = EditForm {
                                        id: s.id,
                                        name: s.name.clone(),
                                        host: s.host.clone(),
                                        port: s.port.to_string(),
                                        username: s.username.clone(),
                                        password: String::new(),
                                        identity_file: s.identity_file.clone().unwrap_or_default(),
                                        forward_agent: s.forward_agent,
                                        description: s.description.clone().unwrap_or_default(),
                                        step: 0,
                                    };
                                    mode = Mode::Edit(form);
                                }
                            }
                            KeyCode::Char('x') | KeyCode::Char('d') => {
                                if let Some((_, idx)) = filtered.get(selected_idx) {
                                    mode = Mode::ConfirmDelete(servers[*idx].id);
                                }
                            }
                            // Arrow key navigation
                            KeyCode::Up | KeyCode::Char('k') => {
                                if !filtered.is_empty() {
                                    selected_idx = selected_idx.saturating_sub(1);
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if !filtered.is_empty() {
                                    selected_idx =
                                        (selected_idx + 1).min(filtered.len().saturating_sub(1));
                                }
                            }
                            // Page navigation
                            KeyCode::PageUp => {
                                if !filtered.is_empty() {
                                    selected_idx = selected_idx.saturating_sub(10);
                                }
                            }
                            KeyCode::PageDown => {
                                if !filtered.is_empty() {
                                    selected_idx =
                                        (selected_idx + 10).min(filtered.len().saturating_sub(1));
                                }
                            }
                            KeyCode::Home => {
                                selected_idx = 0;
                            }
                            KeyCode::End => {
                                if !filtered.is_empty() {
                                    selected_idx = filtered.len().saturating_sub(1);
                                }
                            }
                            KeyCode::Enter => {
                                if let Some((_, idx)) = filtered.get(selected_idx) {
                                    // Clone server data before tearing down terminal
                                    let server = servers[*idx].clone();

                                    // Fully clean up terminal state
                                    cleanup_terminal(inside_tmux)?;
                                    // Drop old terminal to release stdout handle
                                    drop(terminal);

                                    // Run SSH (blocking, inherits stdio)
                                    let connection_result = ssh::connect(&server);

                                    // Rebuild terminal from scratch
                                    enable_raw_mode()?;
                                    let mut stdout = io::stdout();
                                    crossterm::execute!(
                                        stdout,
                                        crossterm::terminal::EnterAlternateScreen
                                    )?;
                                    if !inside_tmux {
                                        crossterm::execute!(
                                            stdout,
                                            crossterm::event::EnableMouseCapture
                                        )?;
                                    }
                                    let backend = CrosstermBackend::new(stdout);
                                    terminal = Terminal::new(backend)?;
                                    terminal.clear()?;

                                    // Reload servers in case vault changed externally
                                    servers = vault.list_servers()?.clone();
                                    filtered = make_filtered(&input, &servers);
                                    clamp_selection(&mut selected_idx, filtered.len());
                                    if let Err(e) = connection_result {
                                        mode = Mode::Message(
                                            format!("Connection failed: {e}"),
                                            Instant::now(),
                                        );
                                    }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                cleanup_terminal(inside_tmux)?;
                                return Ok(());
                            }
                            _ => {}
                        },
                        Mode::Filter => match key.code {
                            KeyCode::Enter => {
                                mode = Mode::Browse;
                            }
                            KeyCode::Esc => {
                                input.clear();
                                filtered = make_filtered("", &servers);
                                clamp_selection(&mut selected_idx, filtered.len());
                                mode = Mode::Browse;
                            }
                            KeyCode::Backspace => {
                                input.pop();
                                filtered = make_filtered(&input, &servers);
                                clamp_selection(&mut selected_idx, filtered.len());
                            }
                            KeyCode::Delete => {
                                input.clear();
                                filtered = make_filtered("", &servers);
                                clamp_selection(&mut selected_idx, filtered.len());
                            }
                            KeyCode::Up => {
                                if !filtered.is_empty() {
                                    selected_idx = selected_idx.saturating_sub(1);
                                }
                            }
                            KeyCode::Down => {
                                if !filtered.is_empty() {
                                    selected_idx =
                                        (selected_idx + 1).min(filtered.len().saturating_sub(1));
                                }
                            }
                            KeyCode::PageUp => {
                                if !filtered.is_empty() {
                                    selected_idx = selected_idx.saturating_sub(10);
                                }
                            }
                            KeyCode::PageDown => {
                                if !filtered.is_empty() {
                                    selected_idx =
                                        (selected_idx + 10).min(filtered.len().saturating_sub(1));
                                }
                            }
                            KeyCode::Home => {
                                selected_idx = 0;
                            }
                            KeyCode::End => {
                                if !filtered.is_empty() {
                                    selected_idx = filtered.len().saturating_sub(1);
                                }
                            }
                            KeyCode::Char(c) => {
                                input.push(c);
                                filtered = make_filtered(&input, &servers);
                                clamp_selection(&mut selected_idx, filtered.len());
                            }
                            _ => {}
                        },
                        Mode::Add(form) => match key.code {
                            KeyCode::Esc => {
                                mode = Mode::Browse;
                            }
                            KeyCode::BackTab => {
                                if form.step > 0 {
                                    form.step -= 1;
                                }
                            }
                            KeyCode::Tab => {
                                form.step = (form.step + 1).min(7);
                            }
                            KeyCode::Enter => {
                                form.step += 1;
                                if form.step > 7 {
                                    // finalize and add
                                    match form.port.parse::<u16>() {
                                        Ok(port) => {
                                            let mut server = Server::new(
                                                form.name.clone(),
                                                form.host.clone(),
                                                port,
                                                form.username.clone(),
                                                form.password.clone(),
                                                if form.description.is_empty() {
                                                    None
                                                } else {
                                                    Some(form.description.clone())
                                                },
                                            );
                                            server.identity_file = if form.identity_file.is_empty()
                                            {
                                                None
                                            } else {
                                                Some(form.identity_file.clone())
                                            };
                                            server.forward_agent = form.forward_agent;
                                            if let Err(e) = vault.add_server(server) {
                                                mode = Mode::Message(
                                                    format!("Add failed: {e}"),
                                                    Instant::now(),
                                                );
                                            } else {
                                                servers = vault.list_servers()?.clone();
                                                filtered = make_filtered(&input, &servers);
                                                clamp_selection(&mut selected_idx, filtered.len());
                                                mode = Mode::Message(
                                                    "Server added".to_string(),
                                                    Instant::now(),
                                                );
                                            }
                                        }
                                        Err(_) => {
                                            mode = Mode::Message(
                                                "Invalid port".to_string(),
                                                Instant::now(),
                                            );
                                        }
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                if form.step == 6 {
                                    form.forward_agent = false;
                                } else {
                                    let target = match form.step {
                                        0 => &mut form.name,
                                        1 => &mut form.host,
                                        2 => &mut form.port,
                                        3 => &mut form.username,
                                        4 => &mut form.password,
                                        5 => &mut form.identity_file,
                                        7 => &mut form.description,
                                        _ => &mut form.name,
                                    };
                                    target.pop();
                                }
                            }
                            KeyCode::Delete => {
                                if form.step == 6 {
                                    form.forward_agent = false;
                                } else {
                                    let target = match form.step {
                                        0 => &mut form.name,
                                        1 => &mut form.host,
                                        2 => &mut form.port,
                                        3 => &mut form.username,
                                        4 => &mut form.password,
                                        5 => &mut form.identity_file,
                                        7 => &mut form.description,
                                        _ => &mut form.name,
                                    };
                                    target.clear();
                                }
                            }
                            KeyCode::Char(c) => {
                                if form.step == 6 {
                                    match c {
                                        ' ' => form.forward_agent = !form.forward_agent,
                                        'y' | 'Y' | 't' | 'T' | '1' => form.forward_agent = true,
                                        'n' | 'N' | 'f' | 'F' | '0' => form.forward_agent = false,
                                        _ => {}
                                    }
                                } else {
                                    let target = match form.step {
                                        0 => &mut form.name,
                                        1 => &mut form.host,
                                        2 => &mut form.port,
                                        3 => &mut form.username,
                                        4 => &mut form.password,
                                        5 => &mut form.identity_file,
                                        7 => &mut form.description,
                                        _ => &mut form.name,
                                    };
                                    target.push(c);
                                }
                            }
                            _ => {}
                        },
                        Mode::Edit(form) => match key.code {
                            KeyCode::Esc => {
                                mode = Mode::Browse;
                            }
                            KeyCode::BackTab => {
                                if form.step > 0 {
                                    form.step -= 1;
                                }
                            }
                            KeyCode::Tab => {
                                form.step = (form.step + 1).min(7);
                            }
                            KeyCode::Enter => {
                                form.step += 1;
                                if form.step > 7 {
                                    // finalize and update
                                    match form.port.parse::<u16>() {
                                        Ok(port) => {
                                            // find original
                                            if let Some(pos) =
                                                servers.iter().position(|s| s.id == form.id)
                                            {
                                                let mut updated = servers[pos].clone();
                                                let password = if form.password.is_empty() {
                                                    updated.password.clone()
                                                } else {
                                                    form.password.clone()
                                                };
                                                updated.update_fields(
                                                    form.name.clone(),
                                                    form.host.clone(),
                                                    port,
                                                    form.username.clone(),
                                                    password,
                                                    if form.description.is_empty() {
                                                        None
                                                    } else {
                                                        Some(form.description.clone())
                                                    },
                                                );
                                                updated.identity_file =
                                                    if form.identity_file.is_empty() {
                                                        None
                                                    } else {
                                                        Some(form.identity_file.clone())
                                                    };
                                                updated.forward_agent = form.forward_agent;
                                                match vault.replace_server(updated) {
                                                    Ok(true) => {
                                                        servers = vault.list_servers()?.clone();
                                                        filtered = make_filtered(&input, &servers);
                                                        clamp_selection(
                                                            &mut selected_idx,
                                                            filtered.len(),
                                                        );
                                                        mode = Mode::Message(
                                                            "Server updated".to_string(),
                                                            Instant::now(),
                                                        );
                                                    }
                                                    Ok(false) => {
                                                        mode = Mode::Message(
                                                            "Server not found".to_string(),
                                                            Instant::now(),
                                                        );
                                                    }
                                                    Err(e) => {
                                                        mode = Mode::Message(
                                                            format!("Update failed: {e}"),
                                                            Instant::now(),
                                                        );
                                                    }
                                                }
                                            } else {
                                                mode = Mode::Message(
                                                    "Server not found".to_string(),
                                                    Instant::now(),
                                                );
                                            }
                                        }
                                        Err(_) => {
                                            mode = Mode::Message(
                                                "Invalid port".to_string(),
                                                Instant::now(),
                                            );
                                        }
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                if form.step == 6 {
                                    form.forward_agent = false;
                                } else {
                                    let target = match form.step {
                                        0 => &mut form.name,
                                        1 => &mut form.host,
                                        2 => &mut form.port,
                                        3 => &mut form.username,
                                        4 => &mut form.password,
                                        5 => &mut form.identity_file,
                                        7 => &mut form.description,
                                        _ => &mut form.name,
                                    };
                                    target.pop();
                                }
                            }
                            KeyCode::Delete => {
                                if form.step == 6 {
                                    form.forward_agent = false;
                                } else {
                                    let target = match form.step {
                                        0 => &mut form.name,
                                        1 => &mut form.host,
                                        2 => &mut form.port,
                                        3 => &mut form.username,
                                        4 => &mut form.password,
                                        5 => &mut form.identity_file,
                                        7 => &mut form.description,
                                        _ => &mut form.name,
                                    };
                                    target.clear();
                                }
                            }
                            KeyCode::Char(c) => {
                                if form.step == 6 {
                                    match c {
                                        ' ' => form.forward_agent = !form.forward_agent,
                                        'y' | 'Y' | 't' | 'T' | '1' => form.forward_agent = true,
                                        'n' | 'N' | 'f' | 'F' | '0' => form.forward_agent = false,
                                        _ => {}
                                    }
                                } else {
                                    let target = match form.step {
                                        0 => &mut form.name,
                                        1 => &mut form.host,
                                        2 => &mut form.port,
                                        3 => &mut form.username,
                                        4 => &mut form.password,
                                        5 => &mut form.identity_file,
                                        7 => &mut form.description,
                                        _ => &mut form.name,
                                    };
                                    target.push(c);
                                }
                            }
                            _ => {}
                        },
                        Mode::ConfirmDelete(id) => match key.code {
                            KeyCode::Char('y') => match vault.remove_server(id) {
                                Ok(_) => {
                                    servers = vault.list_servers()?.clone();
                                    filtered = make_filtered(&input, &servers);
                                    clamp_selection(&mut selected_idx, filtered.len());
                                    mode = Mode::Browse;
                                }
                                Err(e) => {
                                    mode = Mode::Message(
                                        format!("Delete failed: {e}"),
                                        Instant::now(),
                                    );
                                }
                            },
                            KeyCode::Char('n') | KeyCode::Esc => {
                                mode = Mode::Browse;
                            }
                            _ => {}
                        },
                        Mode::Message(_, since) => {
                            // any key returns to browse
                            *since = Instant::now();
                            mode = Mode::Browse;
                        }
                    }
                }
                Event::Mouse(mouse_event) => match mouse_event.kind {
                    MouseEventKind::ScrollUp => {
                        if !filtered.is_empty() {
                            selected_idx = selected_idx.saturating_sub(3);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if !filtered.is_empty() {
                            selected_idx = (selected_idx + 3).min(filtered.len().saturating_sub(1));
                        }
                    }
                    _ => {}
                },
                Event::Resize(_width, _height) => {
                    // Force a full clear so the next draw() picks up the new dimensions
                    // without leftover artifacts from the old size.
                    terminal.clear()?;
                }
                _ => {}
            }
        }

        // auto-clear transient messages
        if let Mode::Message(_, t) = &mode {
            if t.elapsed() > Duration::from_secs(2) {
                mode = Mode::Browse;
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}
