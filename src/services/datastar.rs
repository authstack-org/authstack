use axum::{
    http::header,
    response::{IntoResponse, Response},
};

pub fn sse_patch_elements(selector: &str, mode: &str, elements: &str) -> String {
    let mut body = format!(
        "event: datastar-patch-elements\ndata: selector {}\ndata: mode {}\n",
        selector, mode
    );
    if !elements.is_empty() {
        body += &format!("data: elements {}\n", elements.replace('\n', " "));
    }
    body += "\n";
    body
}

pub fn sse_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}
