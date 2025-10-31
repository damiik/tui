/// Immutable text buffer with cursor position
/// Pure functional data structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    content: String,
    cursor: usize,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Pure transformation: Buffer → char → Buffer
    pub fn insert_char(mut self, c: char) -> Self {
        self.content.insert(self.cursor, c);
        self.cursor += 1;
        self
    }

    /// Pure transformation: Buffer → Buffer
    pub fn delete_char(mut self) -> Self {
        if self.cursor > 0 && !self.content.is_empty() {
            self.content.remove(self.cursor - 1);
            self.cursor -= 1;
        }
        self
    }

    pub fn move_left(mut self) -> Self {
        self.cursor = self.cursor.saturating_sub(1);
        self
    }

    pub fn move_right(mut self) -> Self {
        if self.cursor < self.content.len() {
            self.cursor += 1;
        }
        self
    }

    pub fn move_start(mut self) -> Self {
        self.cursor = 0;
        self
    }

    pub fn move_end(mut self) -> Self {
        self.cursor = self.content.len();
        self
    }

    pub fn clear(mut self) -> Self {
        self.content.clear();
        self.cursor = 0;
        self
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
// Output log with functional append
// ═══════════════════════════════════════════════════════════════

const MAX_LOG_LINES: usize = 1000;

/// Immutable log with bounded size
#[derive(Debug, Clone)]
pub struct OutputLog {
    lines: Vec<String>,
}

impl OutputLog {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Pure transformation: OutputLog → String → OutputLog
    /// Maintains bounded size via functional composition
    pub fn with_message(mut self, msg: String) -> Self {
        self.lines.push(msg);
        
        // Maintain bound by removing oldest entries
        if self.lines.len() > MAX_LOG_LINES {
            self.lines.drain(0..(self.lines.len() - MAX_LOG_LINES));
        }
        
        self
    }

    pub fn clear(mut self) -> Self {
        self.lines.clear();
        self
    }
}

impl Default for OutputLog {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_insert() {
        let buf = Buffer::new().insert_char('a').insert_char('b');
        assert_eq!(buf.content(), "ab");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn test_buffer_delete() {
        let buf = Buffer::new()
            .insert_char('a')
            .insert_char('b')
            .delete_char();
        assert_eq!(buf.content(), "a");
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn test_buffer_movement() {
        let buf = Buffer::new()
            .insert_char('a')
            .insert_char('b')
            .move_left()
            .insert_char('c');
        assert_eq!(buf.content(), "acb");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn test_output_log_append() {
        let log = OutputLog::new()
            .with_message("line1".into())
            .with_message("line2".into());
        assert_eq!(log.lines().len(), 2);
        assert_eq!(log.lines()[0], "line1");
        assert_eq!(log.lines()[1], "line2");
    }

    #[test]
    fn test_output_log_bounds() {
        let mut log = OutputLog::new();
        for i in 0..1500 {
            log = log.with_message(format!("line{}", i));
        }
        assert!(log.lines().len() <= MAX_LOG_LINES);
    }
}
