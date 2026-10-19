//! Checks that a translation kept everything that must not change.
//!
//! If a check fails, the caller keeps the original text instead of showing a
//! translation that may have broken a link, command or name.

use regex::Regex;
use std::sync::OnceLock;

fn token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"https?://[^\s<>()\[\]]+[^\s<>()\[\].,;:!?]", // URLs
            r"|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", // emails
            r"|`[^`\n]+`",                              // inline code
            r"|\{\{[^{}]*\}\}|\{[A-Za-z0-9_]*\}",        // {{x}} and {x} placeholders
            r"|%[sd]|%\d\$[sd]",                       // printf placeholders
            r"|</?g\d+>|<x\d+/>",                     // inline-tag markers from scripts/translate_site.py
        ))
        .unwrap()
    })
}

/// Tokens in `source` that must appear unchanged in a translation.
pub fn protected(source: &str, keep: &[String]) -> Vec<String> {
    let mut out: Vec<String> = token_re().find_iter(source).map(|m| m.as_str().to_string()).collect();
    for term in keep {
        if !term.is_empty() && contains_word(source, term) {
            out.push(term.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// True when `term` occurs in `text` and is not glued to other letters.
fn contains_word(text: &str, term: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = text[start..].find(term) {
        let i = start + pos;
        let before = text[..i].chars().next_back();
        let after = text[i + term.len()..].chars().next();
        let edge = |c: Option<char>| c.map_or(true, |c| !c.is_alphanumeric());
        if edge(before) && edge(after) {
            return true;
        }
        start = i + term.len();
    }
    false
}

/// Undo additions the model sometimes makes: code backticks around words the
/// source did not mark as code.
pub fn tidy(source: &str, translated: &str) -> String {
    let t = translated.trim();
    if !source.contains('`') && t.contains('`') {
        t.replace('`', "")
    } else {
        t.to_string()
    }
}

/// Why a translation was rejected, or None if it passed.
pub fn check(source: &str, translated: &str, keep: &[String]) -> Option<String> {
    let t = translated.trim();
    if t.is_empty() {
        return Some("empty translation".into());
    }
    let limit = source.chars().count() * 4 + 80;
    if t.chars().count() > limit {
        return Some("translation far longer than the source".into());
    }
    for tok in protected(source, keep) {
        if !translated.contains(&tok) {
            return Some(format!("lost protected text: {tok}"));
        }
    }
    None
}

/// Text with nothing to translate (numbers, symbols, a lone URL or code span).
pub fn nothing_to_translate(text: &str) -> bool {
    let stripped = token_re().replace_all(text, "");
    !stripped.chars().any(|c| c.is_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keep() -> Vec<String> {
        vec!["AIEN".into(), "Omega".into()]
    }

    #[test]
    fn finds_urls_code_and_names() {
        let p = protected("See https://aienos.com/docs. Run `aien start` with Omega {name}.", &keep());
        assert!(p.contains(&"https://aienos.com/docs".to_string()));
        assert!(p.contains(&"`aien start`".to_string()));
        assert!(p.contains(&"Omega".to_string()));
        assert!(p.contains(&"{name}".to_string()));
    }

    #[test]
    fn name_inside_a_word_is_not_protected() {
        assert!(protected("AIENOS boots", &keep()).is_empty());
    }

    #[test]
    fn rejects_lost_link() {
        assert!(check("Visit https://x.io now", "Besuchen Sie jetzt", &keep()).is_some());
        assert!(check("Visit https://x.io now", "Besuchen Sie jetzt https://x.io", &keep()).is_none());
    }

    #[test]
    fn rejects_runaway_output() {
        let long = "a".repeat(500);
        assert!(check("hi", &long, &keep()).is_some());
    }

    #[test]
    fn tidy_strips_added_backticks_only() {
        assert_eq!(tidy("Run aien start", "Führen Sie `aien start` aus"), "Führen Sie aien start aus");
        assert_eq!(tidy("Run `aien start`", " `aien start` ausführen "), "`aien start` ausführen");
    }

    #[test]
    fn inline_tag_markers_must_survive() {
        let src = "Read the <g1>docs</g1> and run <x2/>.";
        assert!(check(src, "Lies die <g1>Doku</g1> und führe <x2/> aus.", &keep()).is_none());
        assert!(check(src, "Lies die Doku und führe <x2/> aus.", &keep()).is_some());
    }

    #[test]
    fn detects_untranslatable() {
        assert!(nothing_to_translate("12,345 / 67%"));
        assert!(nothing_to_translate("https://aienos.com"));
        assert!(!nothing_to_translate("Hello"));
    }
}
