#![allow(dead_code)]

use crossterm::{event, terminal};

fn main() -> anyhow::Result<()> {
    let _size = terminal::size;

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
