#![allow(dead_code)]

use crossterm::{event, style, terminal};

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
}

fn main() -> anyhow::Result<()> {
    let size = terminal::size()?;
    let buf = RenderBuffer::new(size.0 as usize * size.1 as usize);
    println!("{buf:?}");

    terminal::enable_raw_mode()?;

    loop {
        let event = event::read()?;

        match event {
            event::Event::Key(event::KeyEvent { code, .. }) => match code {
                event::KeyCode::Esc => break,
                _ => (),
            },
            _ => (),
        }
    }

    terminal::disable_raw_mode()?;

    Ok(())
}
