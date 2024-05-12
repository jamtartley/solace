#![allow(dead_code)]

use std::io;

use crossterm::{
    cursor, event, style,
    terminal::{self, enable_raw_mode},
    ExecutableCommand, QueueableCommand,
};

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
    fn new(size: u16) -> Self {
        let cells = vec![RenderCell::default(); size as usize];

        Self { cells }
    }

    fn put_at(&mut self, i: u16, ch: char) {
        self.cells[i as usize].ch = ch;
    }

    fn render(&self, stdout: &mut io::Stdout, width: u16) -> anyhow::Result<()> {
        for (i, cell) in self.cells.iter().enumerate() {
            let (row, col) = ((i as u16 / width), (i as u16 % width));

            stdout
                .queue(cursor::MoveTo(col, row))?
                .queue(style::Print(cell.ch))?;
        }

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
    let mut buf = RenderBuffer::new(size.0 * size.1);
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

        prompt.render_into(&mut buf, prompt_start_i, size.0);

        let _ = buf.render(&mut stdout, size.0)?;
    }

    Ok(())
}
