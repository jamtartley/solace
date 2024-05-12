#![allow(dead_code)]

use std::{
    io::{self, Write},
    mem,
};

use chat_client::ChatClientBuilder;
use crossterm::{
    cursor,
    event::{self},
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
    fn render_into(&self, buf: &mut RenderBuffer, start: u16, width: u16);
}

#[derive(Debug)]
struct Prompt {
    curr: Vec<char>,
}

impl Prompt {
    fn new() -> Self {
        Self { curr: vec![] }
    }

    fn append(&mut self, ch: char) {
        self.curr.push(ch);
    }
}

impl Renderable for Prompt {
    fn render_into(&self, buf: &mut RenderBuffer, start: u16, width: u16) {
        let mut next = start;

        for _ in 0..width {
            buf.put_at(
                next,
                '━',
                style::Color::White,
                style::Color::Reset,
                CellStyle::default(),
            );
            next += 1;
        }

        for (_, &ch) in self.curr.iter().enumerate() {
            buf.put_at(
                next,
                ch,
                style::Color::White,
                style::Color::Reset,
                CellStyle::default(),
            );
            next += 1;
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
        let event = event::read()?;

        match event {
            event::Event::Key(event::KeyEvent { code, .. }) => match code {
                event::KeyCode::Esc => chat_client.should_quit = true,
                event::KeyCode::Char(ch) => prompt.append(ch),
                _ => (),
            },
            _ => (),
        }

        if let Some(prompt_start_row) = size.1.checked_sub(2) {
            let prompt_start_i = prompt_start_row * size.0;

            prompt.render_into(&mut buf_curr, prompt_start_i, size.0);
        }

        let diff = buf_prev.diff(&buf_curr);

        for patch in &diff {
            patch.render(&mut stdout, size.0)?;
        }

        stdout.flush()?;

        mem::swap(&mut buf_curr, &mut buf_prev);
    }

    Ok(())
}
