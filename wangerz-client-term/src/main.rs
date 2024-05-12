#![allow(dead_code)]

use std::{
    io::{self, Write},
    mem, thread,
};

use chat_client::ChatClientBuilder;
use crossterm::{
    cursor, event,
    style::{self, Stylize},
    terminal, QueueableCommand,
};

mod chat_client;

#[derive(Clone, Copy, Debug, PartialEq)]
enum CellStyle {
    Bold,
    Italic,
    Normal,
}

impl Default for CellStyle {
    fn default() -> Self {
        CellStyle::Normal
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RenderCell {
    pos: u16,
    ch: char,
    fg: style::Color,
    bg: style::Color,
    cell_style: CellStyle,
}

impl RenderCell {
    fn new(pos: u16) -> Self {
        Self {
            pos,
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
    stdout: io::Stdout,
}

impl RenderBuffer {
    fn new(size: u16) -> Self {
        let mut cells = vec![];

        for i in 0..size {
            cells.push(RenderCell::new(i));
        }

        Self {
            cells,
            stdout: io::stdout(),
        }
    }

    fn diff(&self, other: &RenderBuffer) -> Vec<CellPatch> {
        assert!(self.cells.len() == other.cells.len());

        self.cells
            .iter()
            .zip(other.cells.iter())
            .filter(|(us, them)| us != them)
            .map(|(_, them)| CellPatch { cell: them.clone() })
            .collect()
    }

    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|cell| cell.reset());
    }

    fn put_at(
        &mut self,
        i: u16,
        ch: char,
        fg: style::Color,
        bg: style::Color,
        cell_style: CellStyle,
    ) {
        if let Some(c) = self.cells.get_mut(i as usize) {
            *c = RenderCell {
                pos: i,
                ch,
                fg,
                bg,
                cell_style,
            }
        }
    }
}

#[derive(Debug)]
struct CellPatch {
    cell: RenderCell,
}

impl CellPatch {
    fn render(&self, stdout: &mut io::Stdout, width: u16) -> anyhow::Result<()> {
        let RenderCell {
            bg,
            ch,
            fg,
            pos,
            cell_style,
        } = self.cell;
        let (row, col) = ((pos / width), (pos % width));
        let attr = match cell_style {
            CellStyle::Bold => style::Attribute::Bold,
            CellStyle::Italic => style::Attribute::Italic,
            CellStyle::Normal => style::Attribute::NormalIntensity,
        };

        stdout
            .queue(cursor::MoveTo(col, row))?
            .queue(style::PrintStyledContent(
                ch.with(fg).on(bg).attribute(attr),
            ))?;

        Ok(())
    }
}

trait Renderable {
    fn render_into(&self, buf: &mut RenderBuffer, start: u16);
}

#[derive(Debug)]
enum Mode {
    Normal,
    Insert,
}

#[derive(Debug)]
struct Prompt {
    curr: Vec<char>,
    pos: usize,
    mode: Mode,
}

impl Prompt {
    fn new() -> Self {
        Self {
            curr: vec![],
            pos: 0,
            mode: Mode::Insert,
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
            event::KeyCode::Esc => self.mode = Mode::Normal,
            event::KeyCode::Backspace => self.remove(),
            _ => (),
        }
    }

    fn handle_normal(&mut self, key_code: event::KeyCode) {
        match key_code {
            event::KeyCode::Char('i') => self.mode = Mode::Insert,
            _ => (),
        };
    }

    fn clear(&mut self) {
        self.curr.clear();
        self.pos = 0;
        self.mode = Mode::Insert;
    }
}

impl Renderable for Prompt {
    fn render_into(&self, buf: &mut RenderBuffer, start: u16) {
        for (i, &ch) in self.curr.iter().enumerate() {
            buf.put_at(
                i as u16 + start,
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
        let _ = terminal::disable_raw_mode().unwrap();
        let _ = crossterm::execute!(io::stdout(), terminal::LeaveAlternateScreen).unwrap();
    }
}

fn main() -> anyhow::Result<()> {
    let size = terminal::size()?;
    let mut chat_client = ChatClientBuilder::new()
        .with_ip("0.0.0.0")
        .with_port(7878)
        .connect()?;
    let mut stdout = io::stdout();
    let mut buf_curr = RenderBuffer::new(size.0 * size.1);
    let mut buf_prev = RenderBuffer::new(size.0 * size.1);
    let mut prompt = Prompt::new();
    let _screen = Screen::start(&mut stdout)?;

    while !chat_client.should_quit {
        buf_curr.clear();

        match event::read()? {
            event::Event::Key(event::KeyEvent {
                code, modifiers, ..
            }) => match code {
                event::KeyCode::Char('c') if modifiers.contains(event::KeyModifiers::CONTROL) => {
                    chat_client.should_quit = true
                }
                event::KeyCode::Enter => {
                    let to_send = prompt.curr.iter().collect::<String>();

                    // @CLEANUP: Maybe render an error message in the log if not?
                    if chat_client.write(to_send).is_ok() {
                        prompt.clear();
                    }
                }
                _ => prompt.handle_key_press(code),
            },
            _ => (),
        }

        if let Some(prompt_start_row) = size.1.checked_sub(2) {
            for i in 0..size.0 {
                buf_curr.put_at(
                    prompt_start_row * size.0 + i,
                    '━',
                    style::Color::White,
                    style::Color::Reset,
                    CellStyle::Normal,
                );
            }

            let prompt_start_i = (prompt_start_row + 1) * size.0;

            prompt.render_into(&mut buf_curr, prompt_start_i);
        }

        let diff = buf_prev.diff(&buf_curr);

        for patch in &diff {
            patch.render(&mut stdout, size.0)?;
        }

        stdout
            .queue(cursor::MoveTo(prompt.curr.len() as u16, size.1 - 1))?
            .queue(match prompt.mode {
                Mode::Insert => cursor::SetCursorStyle::SteadyBar,
                Mode::Normal => cursor::SetCursorStyle::SteadyBlock,
            })?
            .flush()?;

        mem::swap(&mut buf_curr, &mut buf_prev);

        thread::sleep(std::time::Duration::from_millis(16));
    }

    Ok(())
}
