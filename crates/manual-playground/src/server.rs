use crate::state::{PlaygroundState, ResetRequest, StepRequest};
use serde::de::DeserializeOwned;
use std::{error::Error, io::Read};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLE_CSS: &str = include_str!("../static/style.css");
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

pub fn run(bind: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let server = Server::http(bind)?;
    println!("Manual playground: http://{bind}");
    let mut state = PlaygroundState::default();

    for request in server.incoming_requests() {
        let result = route(request, &mut state);
        if let Err(error) = result {
            eprintln!("request failed: {error}");
        }
    }
    Ok(())
}

fn route(
    mut request: Request,
    state: &mut PlaygroundState,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match (request.method(), request.url()) {
        (&Method::Get, "/") => respond_text(
            request,
            StatusCode(200),
            "text/html; charset=utf-8",
            INDEX_HTML,
        ),
        (&Method::Get, "/app.js") => respond_text(
            request,
            StatusCode(200),
            "text/javascript; charset=utf-8",
            APP_JS,
        ),
        (&Method::Get, "/style.css") => respond_text(
            request,
            StatusCode(200),
            "text/css; charset=utf-8",
            STYLE_CSS,
        ),
        (&Method::Get, "/api/state") => {
            let body = serde_json::to_string(&state.view())?;
            respond_json(request, StatusCode(200), &body)
        }
        (&Method::Post, "/api/step") => {
            let parsed = parse_json::<StepRequest>(&mut request);
            match parsed.and_then(|input| state.step(input).map_err(|error| error.to_string())) {
                Ok(view) => {
                    let body = serde_json::to_string(&view)?;
                    respond_json(request, StatusCode(200), &body)
                }
                Err(message) => respond_json(request, StatusCode(400), &error_json(&message)),
            }
        }
        (&Method::Post, "/api/reset") => {
            let parsed = parse_json::<ResetRequest>(&mut request);
            match parsed
                .and_then(|input| state.reset(input.seed).map_err(|error| error.to_string()))
            {
                Ok(view) => {
                    let body = serde_json::to_string(&view)?;
                    respond_json(request, StatusCode(200), &body)
                }
                Err(message) => respond_json(request, StatusCode(400), &error_json(&message)),
            }
        }
        _ => respond_text(
            request,
            StatusCode(404),
            "text/plain; charset=utf-8",
            "Not found",
        ),
    }
}

fn parse_json<T: DeserializeOwned>(request: &mut Request) -> Result<T, String> {
    let mut body = String::new();
    request
        .as_reader()
        .take(MAX_REQUEST_BYTES)
        .read_to_string(&mut body)
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&body).map_err(|error| error.to_string())
}

fn respond_json(
    request: Request,
    status: StatusCode,
    body: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    respond_text(request, status, "application/json; charset=utf-8", body)
}

fn respond_text(
    request: Request,
    status: StatusCode,
    content_type: &'static str,
    body: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let header = Header::from_bytes("Content-Type", content_type)
        .map_err(|_| "invalid content-type header")?;
    request.respond(
        Response::from_string(body)
            .with_status_code(status)
            .with_header(header),
    )?;
    Ok(())
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}
