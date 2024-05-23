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

use crate::chat_client::ChatClient;

mod chat_client;
mod color;
mod config;
mod logger;
mod prompt;

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

struct Screen;

impl Screen {
    fn start(stdout: &mut io::Stdout) -> anyhow::Result<Self> {
        crossterm::execute!(stdout, terminal::EnterAlternateScreen,)?;
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
    let mut prompt = prompt::Prompt::new();
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
                            chat_client.write(prompt.current_value())?;
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

        // @CLEANUP: assumption of prompt y pos
        prompt.align_cursor(&mut stdout, size.1)?;

        mem::swap(&mut buf_curr, &mut buf_prev);
    }

    Ok(())
}
