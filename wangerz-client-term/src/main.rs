#![allow(dead_code)]

use std::{
    io::{self, Write},
    mem,
    time::Duration,
};

use crossterm::{
    cursor, event,
    style::{self, Stylize},
    terminal, QueueableCommand,
};
use logger::Logger;
use once_cell::sync::OnceCell;

use crate::chat_client::ChatClient;

mod chat_client;
mod color;
mod config;
mod logger;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum CellStyle {
    Bold,
    Italic,
    #[default]
    Normal,
}

#[derive(Clone, Debug, PartialEq)]
struct RenderCell {
    ch: char,
    fg: style::Color,
    bg: style::Color,
    cell_style: CellStyle,
}

impl RenderCell {
    fn new() -> Self {
        Self {
            ch: ' ',
            fg: style::Color::White,
            bg: style::Color::Reset,
            cell_style: CellStyle::Normal,
        }
    }

    fn reset(&mut self) {
        self.ch = ' ';
        self.fg = style::Color::White;
        self.bg = style::Color::Reset;
        self.cell_style = CellStyle::Normal;
    }
}

#[derive(Debug)]
struct RenderBuffer {
    cells: Vec<RenderCell>,
    width: u16,
    height: u16,
}

impl RenderBuffer {
    fn new(width: u16, height: u16) -> Self {
        Self {
            cells: (0..(width * height)).map(|_| RenderCell::new()).collect(),
            width,
            height: 0,
        }
    }

    fn diff(&self, other: &RenderBuffer) -> Vec<CellPatch> {
        assert!(self.cells.len() == other.cells.len());

        self.cells
            .iter()
            .zip(other.cells.iter())
            .enumerate()
            .filter(|(_, (a, b))| *a != *b)
            .map(|(i, (_, cell))| {
                CellPatch::new(cell.clone(), i as u16 % self.width, i as u16 / self.width)
            })
            .collect()
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.cells
            .resize((width * height) as usize, RenderCell::new());
        self.cells.fill(RenderCell::new());
        self.width = width;
        self.height = height;
    }

    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|cell| cell.reset());
    }

    fn put_at(
        &mut self,
        x: u16,
        y: u16,
        ch: char,
        fg: style::Color,
        bg: style::Color,
        cell_style: CellStyle,
    ) {
        let i = y * self.width + x;

        if let Some(c) = self.cells.get_mut(i as usize) {
            *c = RenderCell {
                ch,
                fg,
                bg,
                cell_style,
            }
        }
    }

    fn render_to(&self, qc: &mut impl QueueableCommand) -> anyhow::Result<()> {
        qc.queue(cursor::MoveTo(0, 0))?;

        for RenderCell {
            ch,
            fg,
            bg,
            cell_style,
        } in &self.cells
        {
            let attr = match cell_style {
                CellStyle::Bold => style::Attribute::Bold,
                CellStyle::Italic => style::Attribute::Italic,
                CellStyle::Normal => style::Attribute::NormalIntensity,
            };
            qc.queue(style::PrintStyledContent(
                ch.with(*fg).on(*bg).attribute(attr),
            ))?;
        }

        Ok(())
    }
}

#[derive(Debug)]
struct CellPatch {
    x: u16,
    y: u16,
    cell: RenderCell,
}

impl CellPatch {
    fn new(cell: RenderCell, x: u16, y: u16) -> Self {
        Self { x, y, cell }
    }

    fn render_to(&self, qc: &mut impl QueueableCommand) -> anyhow::Result<()> {
        let RenderCell {
            bg,
            ch,
            fg,
            cell_style,
        } = self.cell;
        let attr = match cell_style {
            CellStyle::Bold => style::Attribute::Bold,
            CellStyle::Italic => style::Attribute::Italic,
            CellStyle::Normal => style::Attribute::NormalIntensity,
        };

        qc.queue(cursor::MoveTo(self.x, self.y))?
            .queue(style::PrintStyledContent(
                ch.with(fg).on(bg).attribute(attr),
            ))?;

        Ok(())
    }
}

struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

trait Renderable {
    fn render_into(&self, buf: &mut RenderBuffer, rect: &Rect);
}

#[derive(Debug)]
enum Mode {
    Normal,
    Insert,
}

#[derive(Debug)]
struct Prompt {
    command_buffer: Vec<char>,
    curr: Vec<char>,
    history: Vec<String>,
    history_offset: usize,
    mode: Mode,
    pos: usize,
}

impl Prompt {
    fn new() -> Self {
        Self {
            command_buffer: vec![],
            curr: vec![],
            history: vec![],
            history_offset: 0,
            mode: Mode::Insert,
            pos: 0,
        }
    }

    fn insert(&mut self, ch: char) {
        self.curr.insert(self.pos, ch);
        self.pos += 1;
    }

    fn remove(&mut self) {
        if self.pos > 0 {
            self.pos -= 1;
            self.curr.remove(self.pos);
        }
    }

    fn handle_key_press(&mut self, key_code: event::KeyCode) {
        match self.mode {
            Mode::Insert => self.handle_insert(key_code),
            Mode::Normal => self.handle_normal(key_code),
        }
    }

    fn handle_insert(&mut self, key_code: event::KeyCode) {
        match key_code {
            event::KeyCode::Char(ch) => self.insert(ch),
            event::KeyCode::Esc => {
                self.switch_to_mode(Mode::Normal);
                self.pos = self.pos.saturating_sub(1);
            }
            event::KeyCode::Backspace => self.remove(),
            event::KeyCode::Up => self.fetch_previous(),
            event::KeyCode::Down => self.fetch_next(),
            _ => (),
        }
    }

    fn switch_to_mode(&mut self, new_mode: Mode) {
        self.mode = new_mode;
        self.command_buffer.clear();
    }

    fn handle_normal(&mut self, key_code: event::KeyCode) {
        match key_code {
            event::KeyCode::Char(ch) if self.command_buffer.first() == Some(&'F') => {
                if let Some(found_at) = self.find_previous_index(|c| c == ch) {
                    self.pos = found_at;
                }

                self.command_buffer.clear();
            }
            event::KeyCode::Char('i') => self.switch_to_mode(Mode::Insert),
            event::KeyCode::Char('h') => self.pos = self.pos.saturating_sub(1),
            event::KeyCode::Char('l') => {
                self.pos = (self.pos + 1).clamp(0, self.curr.len().saturating_sub(1))
            }
            event::KeyCode::Char('I') => {
                self.switch_to_mode(Mode::Insert);
                self.pos = 0;
            }
            event::KeyCode::Char('a') => {
                self.switch_to_mode(Mode::Insert);
                if self.pos < self.curr.len() {
                    self.pos += 1;
                }
            }
            event::KeyCode::Char('A') => {
                self.switch_to_mode(Mode::Insert);
                self.pos = self.curr.len();
            }
            event::KeyCode::Char('C') => {
                self.delete_until_end();
                self.switch_to_mode(Mode::Insert);
            }
            event::KeyCode::Char('D') => {
                self.delete_until_end();
                self.pos = self.pos.clamp(0, self.curr.len().saturating_sub(1));
            }
            event::KeyCode::Char('F') => self.command_buffer.push('F'),
            event::KeyCode::Char('d') => {
                if let Some(ch) = self.command_buffer.first() {
                    if ch == &'d' {
                        self.clear();
                        self.command_buffer.clear();
                    }
                } else {
                    self.command_buffer.push('d');
                }
            }
            event::KeyCode::Char('x') => {
                if !self.curr.is_empty() {
                    self.curr.remove(self.pos);
                    self.pos = self.pos.clamp(0, self.curr.len().saturating_sub(1));
                }
            }
            event::KeyCode::Char('X') => self.clear(),
            event::KeyCode::Char('0') => self.pos = 0,
            event::KeyCode::Char('$') => self.pos = self.curr.len(),
            event::KeyCode::Up => self.fetch_previous(),
            event::KeyCode::Down => self.fetch_next(),
            event::KeyCode::Esc => self.command_buffer.clear(),
            _ => (),
        }
    }

    fn clear(&mut self) {
        self.curr.clear();
        self.pos = 0;
        self.switch_to_mode(Mode::Insert);
    }

    fn flush(&mut self) {
        self.history.push(self.curr.iter().collect::<String>());
        self.history_offset = 0;

        self.clear()
    }

    fn fetch_previous(&mut self) {
        // @CLEANUP: fetch_previous()/fetch_next()
        if self.history_offset + 1 > self.history.len() {
            return;
        }

        self.history_offset += 1;

        if let Some(entry) = self.history.get(self.history.len() - self.history_offset) {
            self.curr = entry.chars().collect();
            self.pos = self.curr.len();
        }
    }

    fn fetch_next(&mut self) {
        if self.history_offset == 0 {
            return;
        }

        self.history_offset -= 1;

        if let Some(entry) = self.history.get(self.history.len() - self.history_offset) {
            self.curr = entry.chars().collect();
            self.pos = self.curr.len();
        }
    }

    fn delete_until_end(&mut self) {
        self.curr = self
            .curr
            .iter()
            .enumerate()
            .filter(|(i, _)| i < &self.pos)
            .map(|(_, val)| *val)
            .collect::<Vec<char>>();
    }

    fn find_previous_index<F>(&self, predicate: F) -> Option<usize>
    where
        F: Fn(char) -> bool,
    {
        if self.pos == 0 {
            return None;
        }

        let mut pos = self.pos - 1;
        while pos > 0 {
            if let Some(ch) = self.curr.get(pos) {
                if predicate(*ch) {
                    return Some(pos);
                }
            }

            pos -= 1;
        }

        None
    }
}

impl Renderable for Prompt {
    fn render_into(&self, buf: &mut RenderBuffer, rect: &Rect) {
        for (i, &ch) in self.curr.iter().enumerate() {
            buf.put_at(
                i as u16 + rect.x,
                rect.y,
                ch,
                style::Color::White,
                style::Color::Reset,
                CellStyle::default(),
            );
        }
    }
}

struct Screen;

impl Screen {
    fn start(stdout: &mut io::Stdout) -> anyhow::Result<Self> {
        crossterm::execute!(stdout, terminal::EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        Ok(Self)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        terminal::disable_raw_mode().unwrap();
        crossterm::execute!(io::stdout(), terminal::LeaveAlternateScreen).unwrap();
    }
}

static LOGGER: OnceCell<Option<Logger>> = OnceCell::new();

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        {
            let log_message = format!($($arg)*);
            if let Some(logger) = $crate::LOGGER.get_or_init(|| Some($crate::Logger::new("/tmp/wangerz.log"))) {
                logger.log(&log_message);
            }
        }
    };
}

fn main() -> anyhow::Result<()> {
    let config = config::Config::new();
    let mut size = terminal::size()?;
    let mut chat_client = ChatClient::new();
    let mut stdout = io::stdout();
    let mut buf_curr = RenderBuffer::new(size.0, size.1);
    let mut buf_prev = RenderBuffer::new(size.0, size.1);
    let mut prompt = Prompt::new();
    let _screen = Screen::start(&mut stdout)?;
    const FRAME_TIME: Duration = std::time::Duration::from_millis(16);

    while !chat_client.should_quit {
        if event::poll(FRAME_TIME)? {
            match event::read()? {
                event::Event::Resize(width, height) => {
                    size = (width, height);
                    buf_curr.resize(width, height);
                    buf_prev.resize(width, height);
                    buf_prev.render_to(&mut stdout)?;
                    stdout.flush()?;
                }
                event::Event::Key(key) => {
                    let event::KeyEvent {
                        code, modifiers, ..
                    } = key;

                    match code {
                        event::KeyCode::Char('c')
                            if modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            // @TODO: Revisit quitting method
                            chat_client.should_quit = true
                        }
                        event::KeyCode::Enter => {
                            let to_send = prompt.curr.iter().collect::<String>();

                            chat_client.write(to_send)?;

                            // @CLEANUP: maybe render an error message in the log if not?
                            prompt.flush();
                        }
                        _ => prompt.handle_key_press(code),
                    }
                }
                _ => (),
            }
        }

        buf_curr.clear();

        chat_client.read()?;

        for i in 0..size.0 {
            buf_curr.put_at(
                i,
                0,
                if let Some(ch) = chat_client.topic.chars().nth(i.into()) {
                    ch
                } else {
                    ' '
                },
                style::Color::Black,
                style::Color::Yellow,
                CellStyle::Bold,
            );
        }

        chat_client.history.render_into(
            &mut buf_curr,
            &Rect {
                x: 0,
                y: 1,
                width: size.0,
                height: size.1.saturating_sub(3),
            },
        );

        if let Some(prompt_start_row) = size.1.checked_sub(2) {
            for i in 0..size.0 {
                buf_curr.put_at(
                    i,
                    prompt_start_row,
                    '━',
                    style::Color::White,
                    style::Color::Reset,
                    CellStyle::Normal,
                );
            }

            prompt.render_into(
                &mut buf_curr,
                &Rect {
                    x: 0,
                    y: prompt_start_row + 1,
                    width: size.0,
                    height: size.1,
                },
            );
        }

        let diff = buf_prev.diff(&buf_curr);

        for patch in &diff {
            patch.render_to(&mut stdout)?;
        }

        stdout
            .queue(cursor::MoveTo(prompt.pos as u16, size.1 - 1))?
            .queue(match prompt.mode {
                Mode::Insert => cursor::SetCursorStyle::SteadyBar,
                Mode::Normal => cursor::SetCursorStyle::SteadyBlock,
            })?
            .flush()?;

        mem::swap(&mut buf_curr, &mut buf_prev);
    }

    Ok(())
}
