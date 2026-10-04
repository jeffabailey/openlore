//! A JS-less browser for the review app: cookie jar, manual redirect following
//! (including the hop out to the user's PDS authorization screen and back),
//! and form submission by visible button label.
//!
//! It drives the app ONLY through its HTTP driving port, the way a person with
//! a browser does. It never reads the app's database or calls its internals.

use std::collections::BTreeMap;

use reqwest::blocking::Client;

use super::html::{self, Form};

/// The page currently shown.
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub status: u16,
    pub url: String,
    pub html: String,
    /// Response headers of the final hop (lower-cased names).
    pub headers: Vec<(String, String)>,
    /// `Set-Cookie` headers seen across every hop of the last navigation.
    pub set_cookies: Vec<String>,
}

impl Page {
    /// Visible text (tags stripped, whitespace collapsed).
    pub fn text(&self) -> String {
        html::visible_text(&self.html)
    }

    pub fn shows(&self, phrase: &str) -> bool {
        self.text().contains(phrase)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Phrases "<login>/<repo> embodies <slug>" of cards that still offer
    /// "Approve" — i.e. the pending suggestions visible to the viewer.
    pub fn pending_cards(&self) -> Vec<String> {
        let mut phrases: Vec<String> = html::elements(&self.html, "article")
            .into_iter()
            .filter(|card| {
                html::forms(card).iter().any(|f| {
                    f.buttons
                        .iter()
                        .any(|(label, _, _)| html::label_matches(label, "Approve"))
                })
            })
            .flat_map(|card| html::embodies_phrases(&html::visible_text(card)))
            .collect();
        phrases.sort();
        phrases.dedup();
        phrases
    }

    /// Every "<login>/<repo> embodies <slug>" phrase anywhere on the page.
    pub fn embodies_phrases(&self) -> Vec<String> {
        let mut phrases = html::embodies_phrases(&self.text());
        phrases.sort();
        phrases.dedup();
        phrases
    }

    /// `true` if any button or link on the page carries `label`.
    pub fn offers(&self, label: &str) -> bool {
        html::forms(&self.html).iter().any(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, label))
        }) || html::links(&self.html)
            .iter()
            .any(|(l, _)| html::label_matches(l, label))
    }

    /// The `data-scan-status` value of the scan-status fragment, if present.
    pub fn scan_status(&self) -> Option<String> {
        html::opening_tags(&self.html, "div")
            .into_iter()
            .chain(html::opening_tags(&self.html, "section"))
            .chain(html::opening_tags(&self.html, "p"))
            .find_map(|(_, tag)| html::attr(tag, "data-scan-status"))
    }
}

/// One user's browser.
pub struct Browser {
    client: Client,
    origin: String,
    jar: BTreeMap<String, BTreeMap<String, String>>,
    pending_fills: Vec<(String, String)>,
    pub page: Page,
}

impl Browser {
    pub fn new(origin: &str) -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("build browser client");
        Self {
            client,
            origin: origin.trim_end_matches('/').to_string(),
            jar: BTreeMap::new(),
            pending_fills: Vec::new(),
            page: Page::default(),
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The cookies the browser holds for the app (name → value).
    pub fn app_cookies(&self) -> BTreeMap<String, String> {
        self.jar
            .get(&authority(&self.origin))
            .cloned()
            .unwrap_or_default()
    }

    /// Forget every cookie (a fresh browser profile / cleared cookies).
    pub fn clear_cookies(&mut self) {
        self.jar.clear();
    }

    /// Navigate to a path on the app (or an absolute URL).
    pub fn open(&mut self, path_or_url: &str) -> &Page {
        let url = self.absolute(path_or_url);
        self.page = self.navigate("GET", &url, None);
        &self.page
    }

    /// Set a field value for the next `press`.
    pub fn fill(&mut self, name: &str, value: &str) -> &mut Self {
        self.pending_fills
            .push((name.to_string(), value.to_string()));
        self
    }

    /// Press the button (or follow the link) labelled `label` on the page.
    pub fn press(&mut self, label: &str) -> &Page {
        if let Err(reason) = self.try_press(label).map(|_| ()) {
            panic!(
                "{reason}\n--- page {} ---\n{}",
                self.page.url,
                self.page.text()
            );
        }
        &self.page
    }

    /// Like [`press`] but reports a missing control instead of panicking.
    pub fn try_press(&mut self, label: &str) -> Result<&Page, String> {
        let html = self.page.html.clone();
        self.press_within(&html, label)
    }

    /// Press `label` inside the `<article>` card whose text contains all `parts`.
    pub fn press_in_card(&mut self, parts: &[&str], label: &str) -> &Page {
        let html = self.page.html.clone();
        let cards = html::cards_containing(&html, parts);
        let card = cards.first().unwrap_or_else(|| {
            panic!(
                "no card containing {parts:?} on {}\n--- page ---\n{}",
                self.page.url,
                self.page.text()
            )
        });
        let card = card.to_string();
        if let Err(reason) = self.press_within(&card, label).map(|_| ()) {
            panic!("{reason}\n--- card ---\n{}", html::visible_text(&card));
        }
        &self.page
    }

    /// The forms on the current page (for adversarial replays).
    pub fn forms(&self) -> Vec<Form> {
        html::forms(&self.page.html)
    }

    /// Submit an explicit form (e.g. one captured from ANOTHER user's page —
    /// the cross-user privacy probes).
    pub fn submit(&mut self, form: &Form, button_label: Option<&str>) -> &Page {
        let mut fields = form.fields.clone();
        if let Some(label) = button_label {
            if let Some((_, Some(name), value)) = form
                .buttons
                .iter()
                .find(|(l, _, _)| html::label_matches(l, label))
            {
                fields.push((name.clone(), value.clone().unwrap_or_default()));
            }
        }
        let url = self.absolute(&form.action);
        let method = if form.method == "post" { "POST" } else { "GET" };
        self.page = self.navigate(method, &url, Some(fields));
        &self.page
    }

    /// Replace a field's value in a captured form (adversarial replays).
    pub fn with_field(form: &Form, name: &str, value: &str) -> Form {
        let mut f = form.clone();
        f.fields.retain(|(k, _)| k != name);
        f.fields.push((name.to_string(), value.to_string()));
        f
    }

    /// POST to an app path with explicit fields (no CSRF token unless given).
    pub fn post(&mut self, path: &str, fields: &[(&str, &str)]) -> &Page {
        let url = self.absolute(path);
        let fields = fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.page = self.navigate("POST", &url, Some(fields));
        &self.page
    }

    // ------------------------------------------------------------------

    fn press_within(&mut self, html_fragment: &str, label: &str) -> Result<&Page, String> {
        let fills = std::mem::take(&mut self.pending_fills);
        if let Some(form) = html::forms(html_fragment).into_iter().find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, label))
        }) {
            let mut fields = form.fields.clone();
            for (name, value) in &fills {
                fields.retain(|(k, _)| k != name);
                fields.push((name.clone(), value.clone()));
            }
            if let Some((_, Some(name), value)) = form
                .buttons
                .iter()
                .find(|(l, _, _)| html::label_matches(l, label))
            {
                fields.push((name.clone(), value.clone().unwrap_or_default()));
            }
            let target = if form.action.is_empty() {
                self.page.url.clone()
            } else {
                form.action.clone()
            };
            let url = self.absolute(&target);
            let method = if form.method == "post" { "POST" } else { "GET" };
            self.page = self.navigate(method, &url, Some(fields));
            return Ok(&self.page);
        }
        if let Some((_, href)) = html::links(html_fragment)
            .into_iter()
            .find(|(l, _)| html::label_matches(l, label))
        {
            let url = self.absolute(&href);
            self.page = self.navigate("GET", &url, None);
            return Ok(&self.page);
        }
        Err(format!("no button or link labelled {label:?}"))
    }

    fn absolute(&self, target: &str) -> String {
        if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else if target.starts_with('/') {
            format!("{}{}", self.origin, target)
        } else {
            let base = self
                .page
                .url
                .rsplit_once('/')
                .map(|(b, _)| b)
                .unwrap_or(&self.origin);
            format!("{base}/{target}")
        }
    }

    fn navigate(
        &mut self,
        method: &str,
        start_url: &str,
        form: Option<Vec<(String, String)>>,
    ) -> Page {
        let mut url = start_url.to_string();
        let mut method = method.to_string();
        let mut form = form;
        let mut set_cookies = Vec::new();
        for _ in 0..12 {
            let auth = authority(&url);
            let mut req = if method == "POST" {
                self.client.post(&url)
            } else {
                self.client.get(&url)
            };
            if let Some(cookies) = self.jar.get(&auth) {
                if !cookies.is_empty() {
                    let header = cookies
                        .iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join("; ");
                    req = req.header("cookie", header);
                }
            }
            if method == "POST" {
                let body = form
                    .take()
                    .unwrap_or_default()
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}={}",
                            openlore_test_support::url_encode(k),
                            openlore_test_support::url_encode(v)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                req = req
                    .header("content-type", "application/x-www-form-urlencoded")
                    .header("origin", self.origin.clone())
                    .body(body);
            }
            let resp = match req.send() {
                Ok(r) => r,
                Err(e) => {
                    return Page {
                        status: 0,
                        url,
                        html: format!("<p>network error: {e}</p>"),
                        headers: Vec::new(),
                        set_cookies,
                    }
                }
            };
            let status = resp.status().as_u16();
            let headers: Vec<(String, String)> = resp
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_ascii_lowercase(),
                        v.to_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            for (k, v) in &headers {
                if k == "set-cookie" {
                    set_cookies.push(v.clone());
                    self.store_cookie(&auth, v);
                }
            }
            let location = headers
                .iter()
                .find(|(k, _)| k == "location")
                .map(|(_, v)| v.clone());
            if (300..400).contains(&status) {
                if let Some(loc) = location {
                    url = resolve(&url, &loc);
                    if status != 307 && status != 308 {
                        method = "GET".to_string();
                    }
                    continue;
                }
            }
            let html = resp.text().unwrap_or_default();
            return Page {
                status,
                url,
                html,
                headers,
                set_cookies,
            };
        }
        panic!("too many redirects starting at {start_url}");
    }

    fn store_cookie(&mut self, auth: &str, set_cookie: &str) {
        let first = set_cookie.split(';').next().unwrap_or("");
        let Some((name, value)) = first.split_once('=') else {
            return;
        };
        let lower = set_cookie.to_ascii_lowercase();
        let jar = self.jar.entry(auth.to_string()).or_default();
        if value.is_empty()
            || lower.contains("max-age=0")
            || lower.contains("expires=thu, 01 jan 1970")
        {
            jar.remove(name.trim());
        } else {
            jar.insert(name.trim().to_string(), value.trim().to_string());
        }
    }
}

/// `host:port` of an absolute URL.
pub fn authority(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    after_scheme.split('/').next().unwrap_or("").to_string()
}

fn resolve(current: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        location.to_string()
    } else if location.starts_with('/') {
        let scheme = current.split_once("://").map(|(s, _)| s).unwrap_or("http");
        format!("{scheme}://{}{location}", authority(current))
    } else {
        let base = current.rsplit_once('/').map(|(b, _)| b).unwrap_or(current);
        format!("{base}/{location}")
    }
}
