//! Serve the viewer page, the latest SVG and its live updates over HTTP on 127.0.0.1.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use miette::{IntoDiagnostic, WrapErr};

use super::live::{LiveDiagram, LiveUpdate};
use crate::failure::Failure;

/// Embed the viewer page, which shows the SVG full-window and follows the event stream.
const VIEWER_PAGE: &str = include_str!("page.html");

/// Accept connections on 127.0.0.1 and answer each, on a thread of its own, from the live diagram.
pub struct ViewServer {
    listener: TcpListener,
    url: String,
    live_diagram: Arc<LiveDiagram>,
}

/// Name the resource a request line asks for.
enum Route {
    ViewerPage,
    DiagramSvg,
    Events,
    NotFound,
}

impl ViewServer {
    /// Bind 127.0.0.1:`port`, port 0 letting the OS pick a free one.
    pub fn bind(port: u16, live_diagram: Arc<LiveDiagram>) -> Result<Self, Failure> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot listen on 127.0.0.1:{port}"))
            .map_err(Failure::Io)?;
        let local_address = listener
            .local_addr()
            .into_diagnostic()
            .wrap_err_with(|| "cannot read the address the viewer listens on")
            .map_err(Failure::Io)?;
        let url = format!("http://{local_address}");
        Ok(Self {
            listener,
            url,
            live_diagram,
        })
    }

    /// Accept connections on a background thread, answering each on a thread of its own.
    pub fn serve_in_background(self) {
        thread::spawn(move || {
            for accepted in self.listener.incoming() {
                let Ok(stream) = accepted else {
                    continue;
                };
                let live_diagram = Arc::clone(&self.live_diagram);
                thread::spawn(move || Self::answer(stream, &live_diagram));
            }
        });
    }

    /// Answer one request with the resource its route names; every response is `Cache-Control: no-store`.
    fn answer(mut stream: TcpStream, live_diagram: &LiveDiagram) -> io::Result<()> {
        let request_line = Self::read_request_line(&stream)?;
        let route = Route::of(&request_line);
        match route {
            Route::ViewerPage => Self::write_document(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                VIEWER_PAGE,
            ),
            Route::DiagramSvg => {
                let svg = live_diagram.svg();
                Self::write_document(&mut stream, "200 OK", "image/svg+xml", &svg)
            }
            Route::Events => Self::stream_events(&mut stream, live_diagram),
            Route::NotFound => Self::write_document(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                "not found\n",
            ),
        }
    }

    /// Read the request line, then read past the headers unparsed, since closing a socket with unread bytes resets the connection.
    fn read_request_line(stream: &TcpStream) -> io::Result<String> {
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        let header_lines = reader.lines().map_while(Result::ok);
        let _header_count = header_lines
            .take_while(|header_line| !header_line.is_empty())
            .count();
        Ok(request_line)
    }

    /// Write a complete response carrying `body`, after which the connection closes.
    fn write_document(
        stream: &mut TcpStream,
        status: &str,
        content_type: &str,
        body: &str,
    ) -> io::Result<()> {
        let body_length = body.len();
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {body_length}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(head.as_bytes())?;
        stream.write_all(body.as_bytes())
    }

    /// Write server-sent events, one batch per live update, until the client hangs up.
    fn stream_events(stream: &mut TcpStream, live_diagram: &LiveDiagram) -> io::Result<()> {
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n";
        stream.write_all(head.as_bytes())?;
        let mut subscription = live_diagram.subscribe();
        loop {
            let update = subscription.next_update();
            let events = Self::server_sent_events_of(update);
            stream.write_all(events.as_bytes())?;
        }
    }

    /// Format `update` as `reload` with the SVG's generation, then `diagnostics` with one `data:` line per report line.
    fn server_sent_events_of(update: LiveUpdate) -> String {
        let LiveUpdate {
            reloaded_generation,
            diagnostics,
        } = update;
        let mut events = String::new();
        if let Some(generation) = reloaded_generation {
            let reload_event = format!("event: reload\ndata: {generation}\n\n");
            events.push_str(&reload_event);
        }
        if let Some(diagnostics) = diagnostics {
            events.push_str("event: diagnostics\n");
            for report_line in diagnostics.lines() {
                let data_line = format!("data: {report_line}\n");
                events.push_str(&data_line);
            }
            events.push('\n');
        }
        events
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Route {
    /// Route a `METHOD TARGET VERSION` request line by its method and its target's path, the query ignored.
    fn of(request_line: &str) -> Self {
        let mut request_words = request_line.split_whitespace();
        let method = request_words.next();
        let target = request_words.next().unwrap_or_default();
        let (path, _query) = target.split_once('?').unwrap_or((target, ""));
        match (method, path) {
            (Some("GET"), "/") => Self::ViewerPage,
            (Some("GET"), "/diagram.svg") => Self::DiagramSvg,
            (Some("GET"), "/events") => Self::Events,
            _unknown_route => Self::NotFound,
        }
    }
}
