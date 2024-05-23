use std::io::{self, Write};

use crossterm::{cursor, event, style, QueueableCommand};

use crate::{CellStyle, Mode, Rect, RenderBuffer, Renderable};

#[derive(Debug)]
pub(crate) struct Prompt {
    pub(crate) pos: usize,
    command_buffer: Vec<char>,
    curr: Vec<char>,
    history: Vec<String>,
    history_offset: usize,
    mode: Mode,
}

impl Prompt {
    pub(crate) fn new() -> Self {
        Self {
            command_buffer: vec![],
            curr: vec![],
            history: vec![],
            history_offset: 0,
            mode: Mode::Insert,
            pos: 0,
        }
    }

    pub(crate) fn flush(&mut self) {
        self.history.push(self.curr.iter().collect::<String>());
        self.history_offset = 0;

        self.clear()
    }

    pub(crate) fn handle_key_press(&mut self, key_code: event::KeyCode) {
        match self.mode {
            Mode::Insert => self.handle_insert(key_code),
            Mode::Normal => self.handle_normal(key_code),
        }
    }

    pub(crate) fn current_value(&self) -> String {
        self.curr.iter().collect::<String>()
    }

    pub(crate) fn align_cursor(&self, stdout: &mut io::Stdout, y: u16) -> anyhow::Result<()> {
        let x = self.pos as u16;

        stdout
            .queue(cursor::MoveTo(x, y))?
            .queue(match self.mode {
                Mode::Insert => cursor::SetCursorStyle::SteadyBar,
                Mode::Normal => cursor::SetCursorStyle::SteadyBlock,
            })?
            .flush()?;

        Ok(())
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
        for i in 0..rect.width {
            buf.put_at(
                i,
                rect.y,
                '━',
                style::Color::Reset,
                style::Color::White,
                CellStyle::Normal,
            );
        }

        for (i, &ch) in self.curr.iter().enumerate() {
            buf.put_at(
                i as u16 + rect.x,
                rect.y + 1,
                ch,
                style::Color::Reset,
                style::Color::White,
                CellStyle::default(),
            );
        }
    }
}
