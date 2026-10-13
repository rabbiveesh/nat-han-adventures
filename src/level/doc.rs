//! A level file as the level editor holds it: the header's lines as written (comments, order
//! and all), the tile grid and the tweak layer as char grids. Edits change only what they touch,
//! so a file loaded and saved unchanged is byte for byte the same, and an edit's diff is just
//! the edit. [`LevelDoc::text`] is the file; [`Level::parse`] (or Side B's parser) reads it.
//!
//! Cell-anchored header lines (`gate:`, `say@`, `hint@`) move with their cells when columns or
//! rows are inserted or deleted.

use super::Level;

/// A level file, for editing.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelDoc {
    /// Header lines, verbatim (no newline).
    pub header: Vec<String>,
    /// The tile grid, rectangular (padded with `.`), row 0 at the top.
    rows: Vec<Vec<char>>,
    /// The tweak layer, the same shape (`.` where nothing's painted).
    layer: Vec<Vec<char>>,
    /// How long each grid row was written (a row is saved at least this long, so untouched
    /// rows keep their trailing dots), and the same for the layer.
    row_len: Vec<usize>,
    layer_len: Vec<usize>,
    /// The file had a tweak layer section (kept, even empty).
    had_layer: bool,
}

/// Is this char empty in a grid?
pub fn is_empty(c: char) -> bool {
    c == '.' || c == ' '
}

/// The key of a header line (`None`: blank or a comment).
pub fn key_of(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.is_empty() || t.starts_with("//") {
        return None;
    }
    t.split_once(':').map(|(k, _)| k.trim())
}

/// The value of a header line (after its first `:`).
pub fn value_of(line: &str) -> &str {
    line.split_once(':').map_or("", |(_, v)| v.trim())
}

impl LevelDoc {
    pub fn parse(src: &str) -> Result<LevelDoc, String> {
        let src = src.replace('\r', "");
        let (header, rest) = src.split_once("\n---\n").ok_or("missing `---` line between header and grid")?;
        let (grid, layer, had_layer) = match rest.split_once("\n---\n") {
            Some((g, l)) => (g, l, true),
            None => match rest.strip_suffix("\n---") {
                Some(g) => (g, "", true),
                None => (rest, "", false),
            },
        };
        let lines = |s: &str| -> Vec<Vec<char>> {
            let mut v: Vec<Vec<char>> = s.lines().map(|l| l.chars().collect()).collect();
            while v.last().is_some_and(|r| r.iter().all(|&c| is_empty(c))) {
                v.pop();
            }
            v
        };
        let rows = lines(grid);
        let layer = lines(layer);
        let mut doc = LevelDoc {
            header: header.lines().map(str::to_string).collect(),
            row_len: rows.iter().map(Vec::len).collect(),
            layer_len: layer.iter().map(Vec::len).collect(),
            rows,
            layer,
            had_layer,
        };
        let (w, h) = (doc.width().max(1), doc.rows.len().max(1));
        doc.resize(w, h);
        Ok(doc)
    }

    /// A new, empty level `w` x `h`: ground along the bottom, the start and the goal on it.
    pub fn blank(name: &str, w: usize, h: usize, side_b: bool) -> LevelDoc {
        let (w, h) = (w.max(8), h.max(4));
        let mut header = vec![format!("name: {name}"), "world: 1".into(), "intro: Here we go!".into()];
        if side_b {
            header.push("setlist: giant=1 gap=1".into());
        } else {
            header.push("deaths: 0".into());
        }
        let mut doc = LevelDoc { header, rows: Vec::new(), layer: Vec::new(), row_len: Vec::new(), layer_len: Vec::new(), had_layer: false };
        doc.resize(w, h);
        doc.row_len = vec![w; h];
        for c in 0..w {
            doc.set(c, h - 1, '#');
            doc.set(c, h - 2, '#');
        }
        doc.set(2, h - 3, 'P');
        doc.set(w - 3, h - 3, 'G');
        doc
    }

    pub fn width(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// The grid char at (col, row) (`.` outside).
    pub fn get(&self, col: usize, row: usize) -> char {
        self.rows.get(row).and_then(|r| r.get(col)).copied().unwrap_or('.')
    }

    /// Set a grid char; the grid grows to hold it.
    pub fn set(&mut self, col: usize, row: usize, ch: char) {
        if col >= self.width() || row >= self.height() {
            self.resize(self.width().max(col + 1), self.height().max(row + 1));
        }
        self.rows[row][col] = ch;
    }

    /// The tweak letter at (col, row) (`.` for none).
    pub fn tweak(&self, col: usize, row: usize) -> char {
        self.layer.get(row).and_then(|r| r.get(col)).copied().unwrap_or('.')
    }

    pub fn set_tweak(&mut self, col: usize, row: usize, ch: char) {
        if col >= self.width() || row >= self.height() {
            self.resize(self.width().max(col + 1), self.height().max(row + 1));
        }
        self.layer[row][col] = ch;
    }

    /// Any cell painted with a tweak?
    pub fn has_layer(&self) -> bool {
        self.layer.iter().flatten().any(|&c| !is_empty(c))
    }

    /// Make the grid `w` x `h` (cut or padded with `.`, right and bottom).
    pub fn resize(&mut self, w: usize, h: usize) {
        for grid in [&mut self.rows, &mut self.layer] {
            grid.resize(h, Vec::new());
            for r in grid.iter_mut() {
                r.resize(w, '.');
            }
        }
        self.row_len.resize(h, w);
        self.layer_len.resize(h, 0);
    }

    /// Insert an empty column before `at`: everything from `at` right moves one right.
    pub fn insert_col(&mut self, at: usize) {
        for grid in [&mut self.rows, &mut self.layer] {
            for r in grid.iter_mut() {
                r.insert(at.min(r.len()), '.');
            }
        }
        for len in self.row_len.iter_mut().chain(self.layer_len.iter_mut()) {
            if *len > at {
                *len += 1;
            }
        }
        self.shift_cells(|c, r| (if c >= at as i32 { c + 1 } else { c }, r));
    }

    /// Delete column `at`: everything right of it moves one left.
    pub fn delete_col(&mut self, at: usize) {
        if self.width() <= 1 || at >= self.width() {
            return;
        }
        for grid in [&mut self.rows, &mut self.layer] {
            for r in grid.iter_mut() {
                r.remove(at);
            }
        }
        for len in self.row_len.iter_mut().chain(self.layer_len.iter_mut()) {
            if *len > at {
                *len -= 1;
            }
        }
        self.shift_cells(|c, r| (if c > at as i32 { c - 1 } else { c }, r));
    }

    /// Insert an empty row above `at`: everything from `at` down moves one down.
    pub fn insert_row(&mut self, at: usize) {
        let w = self.width();
        let at = at.min(self.height());
        self.rows.insert(at, vec!['.'; w]);
        self.layer.insert(at, vec!['.'; w]);
        self.row_len.insert(at, w);
        self.layer_len.insert(at, 0);
        self.shift_cells(|c, r| (c, if r >= at as i32 { r + 1 } else { r }));
    }

    /// Delete row `at`: everything below it moves one up.
    pub fn delete_row(&mut self, at: usize) {
        if self.height() <= 1 || at >= self.height() {
            return;
        }
        self.rows.remove(at);
        self.layer.remove(at);
        self.row_len.remove(at);
        self.layer_len.remove(at);
        self.shift_cells(|c, r| (c, if r > at as i32 { r - 1 } else { r }));
    }

    /// Move the cells named in the header (`gate:`, `say@`, `hint@`) by `f`.
    fn shift_cells(&mut self, f: impl Fn(i32, i32) -> (i32, i32)) {
        let cell = |s: &str| -> Option<String> {
            let (c, r) = s.split_once(',')?;
            let (c, r) = f(c.trim().parse().ok()?, r.trim().parse().ok()?);
            Some(format!("{c},{r}"))
        };
        for line in &mut self.header {
            let Some(key) = key_of(line).map(str::to_string) else { continue };
            if key == "gate" {
                let v = value_of(line);
                let w: Vec<&str> = v.split_whitespace().collect();
                if let [topic, a, b] = w[..]
                    && let (Some(a), Some(b)) = (cell(a), cell(b))
                {
                    *line = format!("gate: {topic} {a} {b}");
                }
            } else if let Some((what, rest)) = key.split_once('@') {
                let mut words = rest.split_whitespace();
                let Some(at) = words.next().and_then(cell) else { continue };
                let topics: Vec<&str> = words.collect();
                let new_key = std::iter::once(format!("{what}@{at}")).chain(topics.iter().map(|t| t.to_string())).collect::<Vec<_>>().join(" ");
                let v = value_of(line).to_string();
                *line = format!("{new_key}: {v}");
            }
        }
    }

    /// The first header line with key `key`, if any.
    pub fn line_of(&self, key: &str) -> Option<usize> {
        self.header.iter().position(|l| key_of(l) == Some(key))
    }

    /// The value of the first `key:` line.
    pub fn get_field(&self, key: &str) -> Option<&str> {
        self.line_of(key).map(|i| value_of(&self.header[i]))
    }

    /// Set the first `key:` line's value (or add the line: after the last line of the same
    /// kind, else at the end). `None` removes it.
    pub fn set_field(&mut self, key: &str, value: Option<&str>) {
        match (self.line_of(key), value) {
            (Some(i), Some(v)) => self.header[i] = format!("{key}: {v}"),
            (Some(i), None) => {
                self.header.remove(i);
            }
            (None, Some(v)) => self.push_line(format!("{key}: {v}")),
            (None, None) => {}
        }
    }

    /// Indices of the header lines whose key passes `f`.
    pub fn lines_where(&self, f: impl Fn(&str) -> bool) -> Vec<usize> {
        (0..self.header.len()).filter(|&i| key_of(&self.header[i]).is_some_and(&f)).collect()
    }

    /// Add a header line after the last one of its kind (same key, or same `say@`/`hint@`/
    /// `tweak`/`gate` family), else at the end.
    pub fn push_line(&mut self, line: String) {
        let fam = |k: &str| -> String {
            if let Some((w, _)) = k.split_once('@') {
                w.to_string()
            } else if let Some(t) = k.strip_prefix("tweak ") {
                let _ = t;
                "tweak".into()
            } else if k.len() == 1 && k.as_bytes()[0].is_ascii_digit() {
                "platform".into()
            } else {
                k.to_string()
            }
        };
        let mine = key_of(&line).map(fam);
        let at = self.header.iter().rposition(|l| key_of(l).map(fam) == mine && mine.is_some());
        match at {
            Some(i) => self.header.insert(i + 1, line),
            None => {
                // Before trailing blank lines.
                let end = self.header.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |i| i + 1);
                self.header.insert(end, line);
            }
        }
    }

    /// The file.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for l in &self.header {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str("---\n");
        let write = |out: &mut String, grid: &[Vec<char>], lens: &[usize]| {
            let mut lines: Vec<String> = grid
                .iter()
                .zip(lens)
                .map(|(r, &len)| {
                    let content = r.iter().rposition(|&c| !is_empty(c)).map_or(0, |i| i + 1);
                    r[..content.max(len).min(r.len())].iter().collect()
                })
                .collect();
            while lines.last().is_some_and(|l| l.chars().all(is_empty)) {
                lines.pop();
            }
            for l in lines {
                out.push_str(&l);
                out.push('\n');
            }
        };
        write(&mut out, &self.rows, &self.row_len);
        if self.had_layer || self.has_layer() {
            out.push_str("---\n");
            write(&mut out, &self.layer, &self.layer_len);
        }
        out
    }

    /// The level (Side A's parser: a `setlist:` line is Side B's and is skipped here).
    pub fn level(&self) -> Result<Level, String> {
        let text: String = self.text().lines().filter(|l| key_of(l) != Some("setlist")).map(|l| format!("{l}\n")).collect();
        Level::parse(&text)
    }

    /// A Side B level (it has a `setlist:` line).
    pub fn is_side_b(&self) -> bool {
        self.line_of("setlist").is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "name: T\n// a comment\ngate: giant 4,0 5,1\nhint@2,1 toot: Toot!\ntweak a: bounce=1\n---\n......\n.P..G\n######\n---\n\n.aa\n";

    #[test]
    fn round_trips_and_edits() {
        let mut d = LevelDoc::parse(SRC).unwrap();
        assert_eq!(d.text(), SRC);
        assert_eq!((d.width(), d.height()), (6, 3));
        assert_eq!(d.tweak(1, 1), 'a');
        d.insert_col(3);
        assert_eq!(d.get(5, 1), 'G');
        assert_eq!(d.get_field("gate"), Some("giant 5,0 6,1"));
        assert!(d.header.contains(&"hint@2,1 toot: Toot!".to_string()), "left of the insert: stays");
        d.insert_row(0);
        assert_eq!(d.get(1, 2), 'P');
        assert!(d.header.contains(&"hint@2,2 toot: Toot!".to_string()));
        d.delete_row(0);
        d.delete_col(3);
        assert_eq!(d.text(), LevelDoc::parse(SRC).unwrap().text());
        assert!(d.level().is_ok());
        d.set_field("world", Some("3"));
        d.set_field("gate", None);
        assert_eq!(d.get_field("world"), Some("3"));
        assert_eq!(d.level().unwrap().world, 3);
        assert!(d.level().unwrap().gates.is_empty());
    }

    #[test]
    fn blank_levels_parse() {
        let d = LevelDoc::blank("New", 40, 14, false);
        let l = d.level().unwrap();
        assert_eq!((l.width, l.height), (40, 14));
        let b = LevelDoc::blank("New", 40, 14, true);
        assert!(b.is_side_b() && b.level().is_ok());
    }
}
