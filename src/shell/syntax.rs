//! Syntax highlighting using the Paper Light / Paper Dark palette.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

static SYNTAX_SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
static PAPER_LIGHT_THEME: OnceLock<syntect::highlighting::Theme> = OnceLock::new();
static PAPER_DARK_THEME: OnceLock<syntect::highlighting::Theme> = OnceLock::new();

pub fn is_code_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "rs" | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "py"
            | "c"
            | "cpp"
            | "cc"
            | "h"
            | "hpp"
            | "java"
            | "go"
            | "toml"
            | "json"
            | "yaml"
            | "yml"
            | "css"
            | "scss"
            | "html"
            | "sh"
            | "bash"
            | "sql"
            | "kt"
            | "swift"
            | "rb"
            | "php"
            | "cs"
            | "xml"
            | "svg"
            | "ini"
    )
}

fn syntect_rgb(hex: u32) -> syntect::highlighting::Color {
    syntect::highlighting::Color {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
        a: 0xff,
    }
}

fn paper_scope(selector: &str, hex: u32) -> syntect::highlighting::ThemeItem {
    use std::str::FromStr;
    syntect::highlighting::ThemeItem {
        scope: syntect::highlighting::ScopeSelectors::from_str(selector)
            .expect("valid syntect scope selector"),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(syntect_rgb(hex)),
            background: None,
            font_style: None,
        },
    }
}

/// Build the Paper Light or Paper Dark syntax theme matching the Zed "Paper"
/// theme palette.
pub fn build_paper_syntax_theme(is_dark: bool) -> syntect::highlighting::Theme {
    let (
        fg,
        comment,
        comment_doc,
        keyword,
        operator,
        function,
        type_color,
        string_color,
        string_escape,
        number,
        boolean,
        constant,
        property,
        variable,
        param,
        var_special,
        punctuation,
        bracket,
        delimiter,
        selector,
        title,
        diff_plus,
        diff_minus,
    ) = if is_dark {
        (
            0xd2d2ce, // primary / variable
            0x777772, // comment
            0x858580, // comment.doc
            0x7897ca, // keyword / attribute / tag / label / constructor
            0x92928c, // operator
            0xa487bd, // function / preproc
            0x89a7d8, // type / enum
            0xc5a35a, // string / string.regex / string.special
            0xad9b78, // string.escape
            0xc09a52, // number
            0xb99958, // boolean
            0xc1a15a, // constant
            0xa08da9, // property
            0xd2d2ce, // variable
            0xb8b8b2, // variable.parameter
            0xc09a52, // variable.special
            0x9a9a94, // punctuation
            0xaaa9a3, // punctuation.bracket
            0x858580, // punctuation.delimiter
            0x819b76, // selector / text.literal
            0xa08da9, // title
            0x819b76, // diff.plus
            0xc17b75, // diff.minus
        )
    } else {
        (
            0x30302f, // primary / variable
            0xa1a19e, // comment
            0x969693, // comment.doc
            0x5477b8, // keyword / attribute / tag / label / constructor
            0x6d6d6a, // operator
            0x845cad, // function / preproc
            0x5f83bd, // type / enum
            0xb98922, // string / string.regex / string.special
            0x9a8760, // string.escape
            0xa77b2c, // number
            0xb28a43, // boolean
            0xb98a32, // constant
            0x7d6b89, // property
            0x30302f, // variable
            0x4d4d4a, // variable.parameter
            0xa77b2c, // variable.special
            0x5f5f5c, // punctuation
            0x777774, // punctuation.bracket
            0x858582, // punctuation.delimiter
            0x718d67, // selector / text.literal
            0x7d6b89, // title
            0x718d67, // diff.plus
            0xb96f69, // diff.minus
        )
    };

    syntect::highlighting::Theme {
        name: Some(if is_dark { "Paper Dark" } else { "Paper Light" }.to_string()),
        author: Some("Declan".to_string()),
        settings: syntect::highlighting::ThemeSettings {
            foreground: Some(syntect_rgb(fg)),
            ..Default::default()
        },
        scopes: vec![
            paper_scope("comment", comment),
            paper_scope(
                "comment.block.documentation, comment.line.documentation",
                comment_doc,
            ),
            paper_scope("keyword, storage.type, storage.modifier", keyword),
            paper_scope("keyword.operator", operator),
            paper_scope(
                "entity.name.function, support.function, meta.function-call, variable.function",
                function,
            ),
            paper_scope(
                "meta.preprocessor, support.macro, entity.name.function.macro",
                function,
            ),
            paper_scope(
                "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, entity.name.interface, support.type, support.class",
                type_color,
            ),
            paper_scope(
                "entity.name.tag, entity.other.attribute-name, meta.annotation, entity.name.label",
                keyword,
            ),
            paper_scope("string", string_color),
            paper_scope("constant.character.escape", string_escape),
            paper_scope("constant.numeric", number),
            paper_scope("constant.language.boolean", boolean),
            paper_scope(
                "constant.language, constant.other, variable.other.constant",
                constant,
            ),
            paper_scope(
                "variable.other.member, variable.other.property, meta.property-name, support.type.property-name",
                property,
            ),
            paper_scope("variable", variable),
            paper_scope("variable.parameter", param),
            paper_scope("variable.language", var_special),
            paper_scope("punctuation", punctuation),
            paper_scope(
                "punctuation.section, punctuation.definition.block, punctuation.definition.parameters, punctuation.definition.group, meta.brace",
                bracket,
            ),
            paper_scope(
                "punctuation.separator, punctuation.terminator, punctuation.accessor",
                delimiter,
            ),
            paper_scope(
                "entity.other.attribute-name.class.css, entity.other.attribute-name.id.css, markup.inline.raw",
                selector,
            ),
            paper_scope("markup.heading, entity.name.section", title),
            paper_scope("markup.inserted", diff_plus),
            paper_scope("markup.deleted", diff_minus),
        ],
    }
}

pub fn highlight_line_spans(
    text: &str,
    ext: &str,
    is_dark: bool,
    default_color: u32,
) -> Vec<(u32, usize, usize)> {
    const MAX_ENTRIES: usize = 8192;
    static CACHE: OnceLock<Mutex<HashMap<u64, Vec<(u32, usize, usize)>>>> = OnceLock::new();
    let effective_ext = if ext.eq_ignore_ascii_case("svg") {
        "xml"
    } else {
        ext
    };
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    effective_ext.hash(&mut hasher);
    is_dark.hash(&mut hasher);
    default_color.hash(&mut hasher);
    let key = hasher.finish();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get(&key) {
            return cached.clone();
        }
    }
    let ss = SYNTAX_SET.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines);
    let theme = if is_dark {
        PAPER_DARK_THEME.get_or_init(|| build_paper_syntax_theme(true))
    } else {
        PAPER_LIGHT_THEME.get_or_init(|| build_paper_syntax_theme(false))
    };
    let syntax = ss
        .find_syntax_by_extension(effective_ext)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let theme_default = theme.settings.foreground;
    let mut h = syntect::easy::HighlightLines::new(syntax, theme);
    let Ok(ranges) = h.highlight_line(text, ss) else {
        return vec![(default_color, 0, text.len())];
    };
    let mut out = Vec::new();
    let mut pos = 0;
    for (style, s) in ranges {
        let len = s.len();
        let c = style.foreground;
        let color = if theme_default == Some(c) {
            default_color
        } else {
            ((c.r as u32) << 24)
                | ((c.g as u32) << 16)
                | ((c.b as u32) << 8)
                | if c.a == 0 { 0xff } else { c.a as u32 }
        };
        out.push((color, pos, pos + len));
        pos += len;
    }
    if out.is_empty() {
        out.push((default_color, 0, text.len()));
    }
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= MAX_ENTRIES {
            guard.clear();
        }
        guard.insert(key, out.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_syntax_themes_parse_and_highlight() {
        let light = build_paper_syntax_theme(false);
        let dark = build_paper_syntax_theme(true);
        assert_eq!(light.name.as_deref(), Some("Paper Light"));
        assert_eq!(dark.name.as_deref(), Some("Paper Dark"));
        let spans = highlight_line_spans("fn main() { let x = \"hi\"; }", "rs", false, 0x30302fff);
        assert!(spans.len() > 2, "expected multiple highlighted spans");
        let svg_spans = highlight_line_spans("<svg viewBox=\"0 0 24 24\">", "svg", false, 0x30302fff);
        assert!(svg_spans.len() > 2, "expected SVG to highlight as XML");
    }
}
