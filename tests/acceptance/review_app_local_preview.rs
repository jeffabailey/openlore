//! fix-review-app-repos-link 01-02 — a dev-only local preview of the review
//! app on the fake Bluesky / PDS / GitHub network.
//!
//! `launch_local_preview` is `#[ignore]`d: `./review-app-preview.sh` runs it
//! with `--ignored --nocapture`. It starts the hermetic world (fake ATProto
//! directory + PDS that is also the OAuth server and approves on its own,
//! fake GitHub, the DEBUG review app on 127.0.0.1 with the development-only
//! `REVIEW_APP_ALLOW_LOOPBACK_HTTP=1`), prints what to do, and blocks until
//! Ctrl-C or stdin EOF. Nothing reaches the network beyond loopback.
//!
//! The sign-in link: the app's CSP (`form-action 'self' https:`) governs
//! where a form submission may be redirected, so a real browser refuses the
//! "Sign in with Bluesky" form's 303 to the fake PDS (plain HTTP, another
//! port). The preview does not touch that header. Instead it serves a sign-in
//! link on another loopback port: it posts the handle to the app's `/signin`
//! itself and sends the browser on to the PDS with a plain 303, which a GET
//! navigation may follow. The session cookie the app sets on its callback is
//! the production one (`__Host-`, Secure, HttpOnly).
//!
//! The non-ignored smoke test builds the SAME world through the SAME
//! [`local_preview`] function and walks the printed instructions with the
//! harness browser, so it proves the launcher's world.

#[path = "support/review_app/mod.rs"]
mod review_app;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use review_app::repos_and_accounts::*;
use review_app::*;

/// The running preview and the persona its banner tells a person to use.
struct LocalPreview {
    world: ReviewWorld,
    persona: Persona,
    sign_in_link_origin: String,
}

impl LocalPreview {
    fn origin(&self) -> &str {
        self.world.app.origin()
    }

    fn sign_in_handle(&self) -> &'static str {
        self.persona.handle()
    }

    fn github_login(&self) -> &'static str {
        self.persona.github_login()
    }

    /// Opening this signs `handle` in (the fake PDS approves at once).
    fn sign_in_link(&self, handle: &str) -> String {
        format!("{}/?handle={handle}", self.sign_in_link_origin)
    }
}

/// The one setup the launcher and its smoke test share: the
/// review-app-repos-and-accounts world (Priya owns 7 public repos and her
/// fake GitHub bio already carries her DID, so Verify succeeds at once) and
/// the sign-in link.
fn local_preview() -> LocalPreview {
    let world = rra_world();
    let sign_in_link_origin = serve_sign_in_link(world.app.origin());
    LocalPreview {
        world,
        persona: Persona::Priya,
        sign_in_link_origin,
    }
}

/// What the launcher prints: where to go and what to type.
fn banner(preview: &LocalPreview) -> String {
    let others = Persona::ALL
        .iter()
        .filter(|p| **p != preview.persona)
        .map(|p| preview.sign_in_link(p.handle()))
        .collect::<Vec<_>>();
    let mut lines = vec![
        String::new(),
        "OpenLore review app: local preview (fake Bluesky, PDS and GitHub on loopback)".into(),
        String::new(),
        format!("  App        {}", preview.origin()),
        format!(
            "  1. Sign in  open {}",
            preview.sign_in_link(preview.sign_in_handle())
        ),
        format!(
            "              (signs in {} at the fake PDS: no password, no consent screen;",
            preview.sign_in_handle()
        ),
        "               the app's own \"Sign in with Bluesky\" form is refused by its CSP".into(),
        "               on plain-HTTP loopback, so use this link)".into(),
        format!(
            "  2. GitHub   follow \"Verify GitHub ownership\", enter {}, press \"Verify\"",
            preview.github_login()
        ),
        format!(
            "              (the fake github.com/{} bio already carries {})",
            preview.github_login(),
            preview.persona.did()
        ),
        "  3. Then     press \"Scan my repos\", or follow \"Choose repos\" in the footer".into(),
        String::new(),
        "  Other fixture personas:".into(),
    ];
    lines.extend(others.into_iter().map(|link| format!("    {link}")));
    lines.push(String::new());
    lines.push("Press Ctrl-C (or close stdin) to stop the app and every fake server.".into());
    lines.join("\n")
}

/// Serve the sign-in link on a free loopback port; returns its origin.
fn serve_sign_in_link(app_origin: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the sign-in link");
    let origin = format!(
        "http://{}",
        listener.local_addr().expect("sign-in link address")
    );
    let app_origin = app_origin.to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            answer_sign_in_link(stream, &app_origin);
        }
    });
    origin
}

/// One request: post `?handle=` to the app's `/signin` as its own form would,
/// then send the browser on to wherever the app sent it (the PDS's authorize).
fn answer_sign_in_link(mut stream: TcpStream, app_origin: &str) {
    let mut request_line = String::new();
    let _ = BufReader::new(&stream).read_line(&mut request_line);
    let reply = match handle_param(&request_line) {
        Some(handle) => match app_sign_in_redirect(app_origin, &handle) {
            Some(location) => format!(
                "HTTP/1.1 303 See Other\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ),
            None => plain_reply("502 Bad Gateway", "the review app did not start a sign-in"),
        },
        None => plain_reply("404 Not Found", "open /?handle=<a fixture handle>"),
    };
    let _ = stream.write_all(reply.as_bytes());
}

fn plain_reply(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// `GET /?handle=X HTTP/1.1` → `X` (fixture handles need no decoding).
fn handle_param(request_line: &str) -> Option<String> {
    let target = request_line.split_whitespace().nth(1)?;
    let query = target.split_once('?')?.1;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("handle="))
        .filter(|handle| !handle.is_empty())
        .map(str::to_string)
}

/// The app's answer to its own sign-in form: the `Location` of its 303.
fn app_sign_in_redirect(app_origin: &str, handle: &str) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let response = client
        .post(format!("{app_origin}/signin"))
        .header("origin", app_origin)
        .form(&[("handle", handle)])
        .send()
        .ok()?;
    let location = response.headers().get("location")?.to_str().ok()?;
    Some(location.to_string())
}

/// Blocks until stdin reaches EOF (Ctrl-C ends the whole process group).
fn wait_for_stdin_eof() {
    let mut sink = std::io::sink();
    let _ = std::io::copy(&mut std::io::stdin().lock(), &mut sink);
}

/// @driving_port @real-io
/// Scenario: local preview signs in a fixture persona on loopback and reaches Choose repos
///   Given the local preview is running and has printed its banner
///   When a person opens the printed sign-in link
///   And verifies GitHub with the printed login
///   Then they land on their review queue
///   When they press "Choose repos"
///   Then they see the Choose repos page
///   And a scan of their repos completes
#[test]
fn local_preview_signs_in_a_fixture_persona_on_loopback_and_reaches_choose_repos() {
    let preview = local_preview();
    let printed = banner(&preview);
    let sign_in_link = preview.sign_in_link(preview.sign_in_handle());
    assert!(preview.origin().starts_with("http://127.0.0.1:"));
    assert!(printed.contains(preview.origin()), "{printed}");
    assert!(printed.contains(&sign_in_link), "{printed}");
    assert!(printed.contains(preview.github_login()), "{printed}");

    let mut browser = Browser::new(preview.origin());
    let signed_in = browser.open(&sign_in_link).clone();
    assert!(
        signed_in.url.starts_with(preview.origin()),
        "{}",
        signed_in.url
    );
    then_sees_signed_in_as(&signed_in, preview.persona);
    let verified = when_verifies_github(&mut browser, preview.github_login()).clone();
    then_sees_verified(&verified, preview.persona);

    let queue = browser.open("/review").clone();
    assert!(queue.shows("Your suggestions"), "{}", queue.text());
    let repos = browser.press("Choose repos").clone();
    assert!(repos.url.ends_with("/repos"), "{}", repos.url);
    assert!(
        repos.html.contains("<h1>Choose repos</h1>"),
        "{}",
        repos.text()
    );

    assert_eq!(
        when_scan_finishes(&mut browser),
        "completed",
        "{}",
        browser.page.text()
    );
}

/// The launcher. Run it through `./review-app-preview.sh`.
#[test]
#[ignore = "dev-only launcher: ./review-app-preview.sh"]
fn launch_local_preview() {
    let preview = local_preview();
    println!("{}", banner(&preview));
    wait_for_stdin_eof();
    println!("Stopping the review app and the fake servers.");
    drop(preview);
}
