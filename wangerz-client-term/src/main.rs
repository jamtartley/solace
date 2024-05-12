#![allow(dead_code)]

use std::io;

use crossterm::{cursor, event, style, terminal, ExecutableCommand, QueueableCommand};

#[derive(Clone, Debug)]
enum CellStyle {
    Bold,
    Italic,
    Normal,
}

#[derive(Clone, Debug)]
struct RenderCell {
    ch: char,
    fg: style::Color,
    bg: style::Color,
    cell_style: CellStyle,
}

impl Default for RenderCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: style::Color::White,
            bg: style::Color::Black,
            cell_style: CellStyle::Normal,
        }
    }
}

#[derive(Debug)]
struct RenderBuffer {
    cells: Vec<RenderCell>,
}

impl RenderBuffer {
    fn new(size: usize) -> Self {
        let cells = vec![RenderCell::default(); size];

        Self { cells }
    }

    fn put_at(&mut self, i: usize, ch: char) {
        self.cells[i].ch = ch;
    }

    fn render(&self, stdout: &mut io::Stdout) -> anyhow::Result<()> {
        let width = 20;

        for (i, cell) in self.cells.iter().enumerate() {
            let (row, col) = ((i / width) as u16, (i % width) as u16);

            stdout
                .queue(cursor::MoveTo(col, row))?
                .queue(style::Print(cell.ch))?;
        }

        Ok(())
    }
}

trait Renderable {
    fn render_into(&self, buf: &mut RenderBuffer, start: usize);
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
    fn render_into(&self, buf: &mut RenderBuffer, start: usize) {
        for (i, &ch) in self.curr.iter().enumerate() {
            buf.put_at(start + i, ch)
        }
    }
}

fn main() -> anyhow::Result<()> {
    let size = terminal::size()?;
    let mut buf = RenderBuffer::new(size.0 as usize * size.1 as usize);
    let mut prompt = Prompt::new();
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    stdout
        .execute(terminal::EnterAlternateScreen)?
        .execute(terminal::DisableLineWrap)?;

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

        prompt.render_into(&mut buf, 0);

        let _ = buf.render(&mut stdout)?;
    }

    stdout.execute(terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    Ok(())
}
