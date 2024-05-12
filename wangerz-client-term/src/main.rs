#![allow(dead_code)]

use std::io::{self, prelude::*};

use crossterm::{
    cursor, event,
    style::{self, Stylize},
    terminal, QueueableCommand,
};

#[derive(Clone, Debug, PartialEq)]
enum CellStyle {
    Bold,
    Italic,
    Normal,
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

#[derive(Clone, Debug)]
struct RenderBuffer {
    cells: Vec<RenderCell>,
}

#[derive(Debug)]
struct CellPatch {
    cell: RenderCell,
}

impl CellPatch {
    fn render(&self, stdout: &mut io::Stdout, width: u16) -> anyhow::Result<()> {
        let (row, col) = ((self.cell.pos / width), (self.cell.pos % width));

        stdout
            .queue(cursor::MoveTo(col, row))?
            .queue(style::PrintStyledContent(
                self.cell.ch.with(self.cell.fg).on(self.cell.bg),
            ))?;

        Ok(())
    }
}

impl RenderBuffer {
    fn new(size: u16) -> Self {
        let mut cells = vec![];

        for i in 0..size {
            cells.push(RenderCell::new(i));
        }

        Self { cells }
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

    fn put_at(&mut self, i: u16, ch: char) {
        self.cells[i as usize].ch = ch;
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
            buf.put_at(next, '━');
            next += 1;
        }

        for (_, &ch) in self.curr.iter().enumerate() {
            buf.put_at(next, ch);
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
    let mut buf_curr = RenderBuffer::new(size.0 * size.1);
    let mut buf_prev = RenderBuffer::new(size.0 * size.1);
    let mut prompt = Prompt::new();
    let mut stdout = io::stdout();
    let _screen = Screen::start(&mut stdout)?;

    loop {
        let event = event::read()?;

        match event {
            event::Event::Key(event::KeyEvent { code, .. }) => match code {
                event::KeyCode::Esc => break,
                event::KeyCode::Char(ch) => prompt.append(ch),
                _ => (),
            },
            _ => (),
        }

        let prompt_start_row = size.1 - 2;
        let prompt_start_i = prompt_start_row * size.0;

        prompt.render_into(&mut buf_curr, prompt_start_i, size.0);

        let diff = buf_prev.diff(&buf_curr);

        for patch in &diff {
            patch.render(&mut stdout, size.0)?;
        }

        // let _ = buf_curr.render(&mut stdout, size.0)?;

        buf_prev = buf_curr.clone();
    }

    Ok(())
}
