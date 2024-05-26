use crossterm::{cursor, event, style};

use crate::{config_hex_color, CellStyle, Mode, Rect, RenderBuffer, Renderable};

#[derive(Debug)]
pub(crate) struct Prompt {
    pub(crate) commands: Vec<String>,
    pub(crate) nicks: Vec<String>,
    pub(crate) nick: String,
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
            commands: vec![],
            nicks: vec![],
            curr: vec![],
            history: vec![],
            history_offset: 0,
            mode: Mode::Insert,
            nick: String::default(),
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

    pub(crate) fn cursor_state(&self) -> (u16, cursor::SetCursorStyle) {
        let x = (self.nick_display().len() + self.pos) as u16;
        let style = match self.mode {
            Mode::Insert => cursor::SetCursorStyle::SteadyBar,
            Mode::Normal => cursor::SetCursorStyle::SteadyBlock,
        };

        (x, style)
    }

    fn nick_display(&self) -> String {
        if self.nick.is_empty() {
            String::default()
        } else {
            format!("[{}] ", self.nick) // Padding deliberate
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
            event::KeyCode::Tab => self.attempt_autocomplete(),
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

    fn attempt_autocomplete(&mut self) {
        if let Some(first) = self.curr.first() {
            let searchable = match *first {
                '/' => Some(&self.commands),
                '@' => Some(&self.nicks),
                _ => None,
            };

            if self.curr.len() == 1 {
                return;
            }

            if self.curr.iter().any(|c| c.is_whitespace()) {
                return;
            }

            if let Some(searchable) = searchable {
                let to_search = self.curr.iter().skip(1).collect::<String>();

                for value in searchable {
                    if value.starts_with(&to_search) {
                        self.curr = format!("{}{} ", first, value).chars().collect();
                        self.pos = self.curr.len();

                        break;
                    }
                }
            }
        }
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

        let nick_len = self.nick_display().len();

        for (i, ch) in self.nick_display().chars().enumerate() {
            buf.put_at(
                i as u16 + rect.x,
                rect.y + 1,
                ch,
                style::Color::Reset,
                config_hex_color!(colors.prompt_nick),
                CellStyle::default(),
            );
        }

        for (i, &ch) in self.curr.iter().enumerate() {
            buf.put_at(
                i as u16 + rect.x + nick_len as u16,
                rect.y + 1,
                ch,
                style::Color::Reset,
                style::Color::White,
                CellStyle::default(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flush() {
        let mut prompt = Prompt::new();
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.flush();
        assert_eq!(prompt.history, vec!["abc".to_string()]);
        assert_eq!(prompt.history_offset, 0);
        assert_eq!(prompt.curr, Vec::new());
        assert_eq!(prompt.pos, 0);
    }

    #[test]
    fn test_press_i_from_normal_mode() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 2;
        prompt.handle_key_press(event::KeyCode::Char('i'));
        assert!(matches!(prompt.mode, Mode::Insert));
        assert_eq!(prompt.pos, 2);
    }

    #[test]
    fn test_press_a_from_normal_mode() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 2;
        prompt.handle_key_press(event::KeyCode::Char('a'));
        assert!(matches!(prompt.mode, Mode::Insert));
        assert_eq!(prompt.pos, 3);
    }

    #[test]
    fn test_press_i_from_normal_mode_at_start() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 0;
        prompt.handle_key_press(event::KeyCode::Char('i'));
        assert!(matches!(prompt.mode, Mode::Insert));
        assert_eq!(prompt.pos, 0);
    }

    #[test]
    fn test_press_a_from_normal_mode_at_end() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 3;
        prompt.handle_key_press(event::KeyCode::Char('a'));
        assert!(matches!(prompt.mode, Mode::Insert));
        assert_eq!(prompt.pos, 3);
    }

    #[test]
    fn test_press_i_and_insert_char() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 1;
        prompt.handle_key_press(event::KeyCode::Char('i'));
        assert!(matches!(prompt.mode, Mode::Insert));
        prompt.handle_key_press(event::KeyCode::Char('x'));
        assert_eq!(prompt.curr, vec!['a', 'x', 'b', 'c']);
        assert_eq!(prompt.pos, 2);
    }

    #[test]
    fn test_press_a_and_insert_char() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 1;
        prompt.handle_key_press(event::KeyCode::Char('a'));
        assert!(matches!(prompt.mode, Mode::Insert));
        prompt.handle_key_press(event::KeyCode::Char('x'));
        assert_eq!(prompt.curr, vec!['a', 'b', 'x', 'c']);
        assert_eq!(prompt.pos, 3);
    }

    #[test]
    fn test_handle_key_press_normal_mode() {
        let mut prompt = Prompt::new();
        prompt.mode = Mode::Normal;
        prompt.curr = vec!['a', 'b', 'c'];
        prompt.pos = 1;
        prompt.handle_key_press(event::KeyCode::Char('h'));
        assert_eq!(prompt.pos, 0);
        prompt.handle_key_press(event::KeyCode::Char('l'));
        assert_eq!(prompt.pos, 1);
        prompt.handle_key_press(event::KeyCode::Char('0'));
        assert_eq!(prompt.pos, 0);
        prompt.handle_key_press(event::KeyCode::Char('$'));
        assert_eq!(prompt.pos, 3);
    }

    #[test]
    fn test_insert() {
        let mut prompt = Prompt::new();
        prompt.insert('a');
        assert_eq!(prompt.curr, vec!['a']);
        assert_eq!(prompt.pos, 1);
        prompt.insert('b');
        assert_eq!(prompt.curr, vec!['a', 'b']);
        assert_eq!(prompt.pos, 2);
    }

    #[test]
    fn test_remove() {
        let mut prompt = Prompt::new();
        prompt.insert('a');
        prompt.insert('b');
        prompt.remove();
        assert_eq!(prompt.curr, vec!['a']);
        assert_eq!(prompt.pos, 1);
    }

    #[test]
    fn test_switch_to_mode() {
        let mut prompt = Prompt::new();
        prompt.switch_to_mode(Mode::Normal);
        assert!(matches!(prompt.mode, Mode::Normal));
        assert_eq!(prompt.command_buffer, Vec::new());
    }

    #[test]
    fn test_clear() {
        let mut prompt = Prompt::new();
        prompt.insert('a');
        prompt.insert('b');
        prompt.insert('c');
        prompt.clear();
        assert_eq!(prompt.curr, Vec::new());
        assert_eq!(prompt.pos, 0);
        assert!(matches!(prompt.mode, Mode::Insert));
    }

    #[test]
    fn test_fetch_previous_with_history() {
        let mut prompt = Prompt::new();
        prompt.history = vec!["first".to_string(), "second".to_string()];
        prompt.fetch_previous();
        assert_eq!(prompt.curr, vec!['s', 'e', 'c', 'o', 'n', 'd']);
        assert_eq!(prompt.pos, 6);
    }

    #[test]
    fn test_fetch_previous_without_history() {
        let mut prompt = Prompt::new();
        prompt.history = vec![];
        prompt.fetch_previous();
        assert_eq!(prompt.curr, vec![]);
        assert_eq!(prompt.pos, 0);
    }

    #[test]
    fn test_fetch_next_with_history() {
        let mut prompt = Prompt::new();
        prompt.history = vec!["first".to_string(), "second".to_string()];
        prompt.fetch_previous();
        prompt.fetch_previous();
        prompt.fetch_next();
        assert_eq!(prompt.curr, vec!['s', 'e', 'c', 'o', 'n', 'd']);
        assert_eq!(prompt.pos, 6);
    }

    #[test]
    fn test_fetch_next_without_history() {
        let mut prompt = Prompt::new();
        prompt.history = vec![];
        prompt.fetch_previous();
        prompt.fetch_previous();
        prompt.fetch_next();
        assert_eq!(prompt.curr, vec![]);
        assert_eq!(prompt.pos, 0);
    }

    #[test]
    fn test_delete_until_end_from_start() {
        let mut prompt = Prompt::new();
        prompt.insert('a');
        prompt.insert('b');
        prompt.insert('c');
        prompt.pos = 0;
        prompt.delete_until_end();
        assert_eq!(prompt.curr, vec![]);
        assert_eq!(prompt.pos, 0);
    }

    #[test]
    fn test_delete_until_end_from_middle() {
        let mut prompt = Prompt::new();
        prompt.insert('a');
        prompt.insert('b');
        prompt.insert('c');
        prompt.pos = 1;
        prompt.delete_until_end();
        assert_eq!(prompt.curr, vec!['a']);
        assert_eq!(prompt.pos, 1);
    }

    #[test]
    fn test_delete_until_end_from_end() {
        let mut prompt = Prompt::new();
        prompt.insert('a');
        prompt.insert('b');
        prompt.insert('c');
        prompt.pos = 3;
        prompt.delete_until_end();
        assert_eq!(prompt.curr, vec!['a', 'b', 'c']);
        assert_eq!(prompt.pos, 3);
    }

    #[test]
    fn test_find_previous_index() {
        let mut prompt = Prompt::new();
        prompt.curr = vec!['a', 'b', 'c', 'd'];
        prompt.pos = 3;
        let index = prompt.find_previous_index(|c| c == 'b');
        assert_eq!(index, Some(1));
    }

    #[test]
    fn test_nick_display_no_nick() {
        let mut prompt = Prompt::new();
        prompt.nick = String::default();
        assert_eq!(prompt.nick_display(), "");
    }

    #[test]
    fn test_nick_display_with_nick() {
        let mut prompt = Prompt::new();
        prompt.nick = "user".to_string();
        assert_eq!(prompt.nick_display(), "[user] ");
    }

    #[test]
    fn test_attempt_autocomplete_does_nothing_without_commands() {
        let mut prompt = Prompt::new();
        prompt.curr = vec!['/'];
        prompt.attempt_autocomplete();
        assert_eq!(prompt.curr, vec!['/']);
    }

    #[test]
    fn test_attempt_autocomplete_does_nothing_if_not_only_slash() {
        let mut prompt = Prompt::new();
        prompt.commands = vec!["topic".to_string()];
        prompt.curr = vec!['c', 'o'];
        prompt.attempt_autocomplete();
        assert_eq!(prompt.curr, vec!['c', 'o']);
    }

    #[test]
    fn test_attempt_autocomplete_does_nothing_if_only_slash() {
        let mut prompt = Prompt::new();
        prompt.commands = vec!["help".to_string()];
        prompt.curr = vec!['/'];
        prompt.attempt_autocomplete();
        assert_eq!(prompt.curr, vec!['/']);
    }

    #[test]
    fn test_attempt_autocomplete_does_nothing_with_whitespace() {
        let mut prompt = Prompt::new();
        prompt.commands = vec!["help".to_string()];
        prompt.curr = vec!['/', 'h', 'e', ' '];
        prompt.attempt_autocomplete();
        assert_eq!(prompt.curr, vec!['/', 'h', 'e', ' ']);
    }

    #[test]
    fn test_attempt_autocomplete_successful() {
        let mut prompt = Prompt::new();
        prompt.commands = vec!["help".to_string()];
        prompt.curr = vec!['/', 'h', 'e'];
        prompt.attempt_autocomplete();
        assert_eq!(prompt.curr, vec!['/', 'h', 'e', 'l', 'p', ' ']);
    }

    #[test]
    fn test_attempt_autocomplete_no_match() {
        let mut prompt = Prompt::new();
        prompt.commands = vec!["help".to_string()];
        prompt.curr = vec!['/', 'x', 'y', 'z'];
        prompt.attempt_autocomplete();
        assert_eq!(prompt.curr, vec!['/', 'x', 'y', 'z']);
    }

    #[test]
    fn test_attempt_autocomplete_multiple_matches_picks_first() {
        let mut prompt = Prompt::new();
        prompt.commands = vec!["help".to_string(), "hello".to_string()];
        prompt.curr = vec!['/', 'h', 'e'];
        prompt.attempt_autocomplete();
        assert!(prompt.curr == vec!['/', 'h', 'e', 'l', 'p', ' ']);
    }
}
