//! Syntax highlighting service using syntect.

use std::path::Path;
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::ui::HighlightedLine;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

pub fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub fn highlight_text(path: &Path, content: &str, dark_mode: bool) -> Option<Vec<HighlightedLine>> {
    let ss = get_syntax_set();
    let ts = get_theme_set();
    let ext = path.extension()?.to_str()?;
    let syntax = ss.find_syntax_by_extension(ext)?;
    let theme_name = if dark_mode { "base16-ocean.dark" } else { "InspiredGitHub" };
    let theme = ts.themes.get(theme_name)?;
    let mut h = HighlightLines::new(syntax, theme);

    let mut lines = Vec::new();
    for line in content.lines().take(500) {
        let ranges = h.highlight_line(line, ss).ok()?;
        let spans = ranges.iter().map(|(style, text)| {
            let c = style.foreground;
            let rgba = ((c.r as u32) << 24) | ((c.g as u32) << 16) | ((c.b as u32) << 8) | (c.a as u32);
            (rgba, text.to_string())
        }).collect();
        lines.push(HighlightedLine { spans });
    }
    Some(lines)
}
