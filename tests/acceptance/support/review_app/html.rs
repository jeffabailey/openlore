//! Minimal, dependency-free reading of the review app's server-rendered HTML.
//!
//! The app is progressively enhanced (works without JS; ADR-072 / arch §8), so
//! a scenario "presses a button" by submitting the plain `<form>` that holds
//! it, exactly as a browser without JS would. The page contract this relies
//! on is small and semantic (see `distill/wave-decisions.md` DWD-6):
//!
//! * every action is a `<form method="post" action=…>` with a `<button>`
//!   (or a plain `<a href>` for navigation);
//! * each suggestion / claim is one `<article>`;
//! * the scan-status fragment carries `data-scan-status="<status>"`.

/// A form found on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub action: String,
    pub method: String,
    /// Named fields with their current (default) values, in document order.
    pub fields: Vec<(String, String)>,
    /// Submit buttons: (visible label, optional name, optional value).
    pub buttons: Vec<(String, Option<String>, Option<String>)>,
}

/// Decode the handful of HTML entities maud emits.
pub fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&hellip;", "…")
        .replace("&amp;", "&")
}

/// Remove a whole element kind (e.g. `script`) including its content.
fn strip_blocks(html: &str, tag: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    while let Some(start) = rest.to_ascii_lowercase().find(&open) {
        out.push_str(&rest[..start]);
        match rest[start..].to_ascii_lowercase().find(&close) {
            Some(end) => rest = &rest[start + end + close.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The visible text of an HTML fragment: tags removed, entities decoded,
/// whitespace collapsed.
pub fn visible_text(html: &str) -> String {
    let cleaned = strip_blocks(&strip_blocks(html, "script"), "style");
    let mut text = String::with_capacity(cleaned.len());
    let mut in_tag = false;
    for c in cleaned.chars() {
        match c {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    decode_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The value of attribute `name` inside an opening-tag string like
/// `<input type="hidden" name="csrf" value="x">`.
pub fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!(" {name}=");
    if let Some(pos) = lower.find(&needle) {
        let rest = &tag[pos + needle.len()..];
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or("").to_string(),
            _ => rest
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or("")
                .to_string(),
        };
        return Some(decode_entities(&value));
    }
    // A boolean attribute (`disabled`, `selected`, `checked`).
    let bare = lower.trim_end_matches('>').trim_end_matches('/');
    let is_boolean = bare.split_whitespace().any(|token| token == name);
    is_boolean.then(String::new)
}

/// Every opening tag `<tag …>` in `html`, with its byte range.
pub fn opening_tags<'a>(html: &'a str, tag: &str) -> Vec<(usize, &'a str)> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&open) {
        let start = from + pos;
        let after = lower.as_bytes().get(start + open.len()).copied();
        if matches!(after, Some(b' ' | b'>' | b'/' | b'\n' | b'\t')) {
            let end = html[start..]
                .find('>')
                .map(|e| start + e + 1)
                .unwrap_or(html.len());
            found.push((start, &html[start..end]));
            from = end;
        } else {
            from = start + open.len();
        }
    }
    found
}

/// The inner HTML of each `<tag …>…</tag>` element (non-nested use only).
pub fn elements<'a>(html: &'a str, tag: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let close = format!("</{tag}>");
    opening_tags(html, tag)
        .into_iter()
        .filter_map(|(start, open_tag)| {
            let inner_start = start + open_tag.len();
            lower[inner_start..]
                .find(&close)
                .map(|end| &html[start..inner_start + end + close.len()])
        })
        .collect()
}

/// Every form on the page (or fragment).
pub fn forms(html: &str) -> Vec<Form> {
    elements(html, "form")
        .into_iter()
        .map(|element| {
            let open = &element[..element.find('>').map(|e| e + 1).unwrap_or(0)];
            let mut fields = Vec::new();
            for (_, input) in opening_tags(element, "input") {
                let kind = attr(input, "type")
                    .unwrap_or_else(|| "text".into())
                    .to_ascii_lowercase();
                if kind == "submit" || kind == "button" {
                    continue;
                }
                if (kind == "checkbox" || kind == "radio") && attr(input, "checked").is_none() {
                    continue;
                }
                if let Some(name) = attr(input, "name") {
                    fields.push((name, attr(input, "value").unwrap_or_default()));
                }
            }
            for textarea in elements(element, "textarea") {
                let open_t = &textarea[..textarea.find('>').map(|e| e + 1).unwrap_or(0)];
                if let Some(name) = attr(open_t, "name") {
                    let inner = &textarea[open_t.len()..textarea.len() - "</textarea>".len()];
                    fields.push((name, decode_entities(inner)));
                }
            }
            for select in elements(element, "select") {
                let open_s = &select[..select.find('>').map(|e| e + 1).unwrap_or(0)];
                if let Some(name) = attr(open_s, "name") {
                    let options = opening_tags(select, "option");
                    let chosen = options
                        .iter()
                        .find(|(_, o)| attr(o, "selected").is_some())
                        .or_else(|| options.first())
                        .and_then(|(_, o)| attr(o, "value"))
                        .unwrap_or_default();
                    fields.push((name, chosen));
                }
            }
            let buttons = elements(element, "button")
                .into_iter()
                .filter(|b| {
                    let open_b = &b[..b.find('>').map(|e| e + 1).unwrap_or(0)];
                    attr(open_b, "type")
                        .map(|t| t.eq_ignore_ascii_case("submit"))
                        .unwrap_or(true)
                })
                .map(|b| {
                    let open_b = &b[..b.find('>').map(|e| e + 1).unwrap_or(0)];
                    (visible_text(b), attr(open_b, "name"), attr(open_b, "value"))
                })
                .collect();
            Form {
                action: attr(open, "action").unwrap_or_default(),
                method: attr(open, "method")
                    .unwrap_or_else(|| "get".into())
                    .to_ascii_lowercase(),
                fields,
                buttons,
            }
        })
        .collect()
}

/// Every `<a href>` link: (visible label, href).
pub fn links(html: &str) -> Vec<(String, String)> {
    elements(html, "a")
        .into_iter()
        .filter_map(|a| {
            let open = &a[..a.find('>').map(|e| e + 1).unwrap_or(0)];
            attr(open, "href").map(|href| (visible_text(a), href))
        })
        .collect()
}

/// Labels match after whitespace normalisation; a trailing ellipsis or arrow
/// in the page label is tolerated ("Share on Bluesky…", "Review suggestions →").
pub fn label_matches(found: &str, wanted: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_end_matches(['…', '→', '.'])
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    norm(found) == norm(wanted)
}

/// The `<article>` elements whose visible text contains every part.
pub fn cards_containing<'a>(html: &'a str, parts: &[&str]) -> Vec<&'a str> {
    elements(html, "article")
        .into_iter()
        .filter(|card| {
            let text = visible_text(card);
            parts.iter().all(|p| text.contains(p))
        })
        .collect()
}

/// Every "<login>/<repo> embodies <slug>" phrase present in `text`.
pub fn embodies_phrases(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut found = Vec::new();
    for window in words.windows(3) {
        if window[1] == "embodies" && window[0].contains('/') {
            let slug = window[2].trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
            found.push(format!("{} embodies {}", window[0], slug));
        }
    }
    found
}
