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

use crate::{chat_client::ChatClient, command::parse_command};

mod chat_client;
mod command;

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
        if let event::KeyCode::Char('i') = key_code {
            self.mode = Mode::Insert
        }
    }

    fn clear(&mut self) {
        self.curr.clear();
        self.pos = 0;
        self.mode = Mode::Insert;
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

fn main() -> anyhow::Result<()> {
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
                            chat_client.should_quit = true
                        }
                        event::KeyCode::Enter => {
                            let to_send = prompt.curr.iter().collect::<String>();
                            chat_client.history.message(&to_send);

                            match parse_command(&to_send) {
                                Ok(Some((command, args))) => {
                                    (command.execute)(&mut chat_client, &args)?;
                                }
                                // @CLEANUP: Improve 'command not found' error
                                Ok(None) => chat_client
                                    .history
                                    .error(format!("ERROR: Command not found.")),
                                Err(_) => {
                                    if chat_client.write(to_send).is_ok() {
                                        prompt.clear();
                                    }
                                }
                            }

                            // @CLEANUP: Maybe render an error message in the log if not?
                            prompt.clear();
                        }
                        _ => prompt.handle_key_press(code),
                    }
                }
                _ => (),
            }
        }

        buf_curr.clear();

        chat_client.read()?;
        chat_client.history.render_into(
            &mut buf_curr,
            &Rect {
                x: 0,
                y: 0,
                width: size.0,
                height: size.1.saturating_sub(2),
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
            .queue(cursor::MoveTo(prompt.curr.len() as u16, size.1 - 1))?
            .queue(match prompt.mode {
                Mode::Insert => cursor::SetCursorStyle::SteadyBar,
                Mode::Normal => cursor::SetCursorStyle::SteadyBlock,
            })?
            .flush()?;

        mem::swap(&mut buf_curr, &mut buf_prev);
    }

    Ok(())
}
