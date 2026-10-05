//! Token-free correlation for CLI requests, structured Workers logs and W3C traces.
use wasm_bindgen::{JsCast, JsValue};
use worker::{Context, Date, Request, Response};

/// A request-local context; it never contains identity subjects or credentials.
pub struct RequestContext {
    /// Stable request correlation ID returned to the client.
    pub request_id: String,
    /// Valid W3C context with the incoming trace ID and a fresh service span ID.
    pub traceparent: String,
    /// Starting platform clock used for latency measurements.
    started_at: u64,
}

impl RequestContext {
    /// Preserve bounded client correlation, otherwise generate fresh identifiers.
    pub fn new(request: &Request) -> worker::Result<Self> {
        let supplied_id = request.headers().get("x-mskill-request-id")?.filter(|id| {
            !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });
        let request_id = match supplied_id {
            Some(id) => id,
            None => random_hex(16)?,
        };
        let incoming = request.headers().get("traceparent")?;
        let (trace_id, flags) = match incoming.as_deref().filter(|v| valid_traceparent(v)) {
            Some(value) => (value[3..35].to_owned(), value[53..55].to_owned()),
            None => (random_hex(16)?, "01".to_owned()),
        };
        Ok(Self {
            request_id,
            traceparent: format!("00-{trace_id}-{}-{flags}", random_hex(8)?),
            started_at: Date::now().as_millis(),
        })
    }

    /// Attach trace/correlation and baseline API security headers.
    pub fn finish(
        &self,
        mut response: Response,
        route: &str,
        method: &str,
    ) -> worker::Result<Response> {
        response
            .headers_mut()
            .set("x-mskill-request-id", &self.request_id)?;
        response
            .headers_mut()
            .set("traceparent", &self.traceparent)?;
        response
            .headers_mut()
            .set("x-content-type-options", "nosniff")?;
        response
            .headers_mut()
            .set("referrer-policy", "no-referrer")?;
        response.headers_mut().set(
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        )?;
        if route.starts_with("/v1") || route == "/health" {
            response.headers_mut().set("cache-control", "no-store")?;
        }
        let event = serde_json::json!({"event":"http_request", "route":route, "method":method,
            "status":response.status_code(), "duration_ms":Date::now().as_millis().saturating_sub(self.started_at),
            "request_id":self.request_id,"trace_id":&self.traceparent[3..35],"span_id":&self.traceparent[36..52],
            "error_code":response.headers().get("x-mskill-error-code")?});
        log_event(&event, response.status_code() >= 500);
        Ok(response)
    }
}

/// Join Cloudflare's automatic span graph to the CLI's correlation ID without
/// depending on account-specific incoming-trace propagation beta support.
/// Older/local runtimes lacking the API keep logs and requests fully functional.
pub fn annotate_platform(context: &Context, correlation: &RequestContext, route: &str) {
    let annotate = || -> std::result::Result<(), JsValue> {
        let raw: &JsValue = context.as_ref().as_ref();
        let tracing = js_sys::Reflect::get(raw, &"tracing".into())?;
        let get: js_sys::Function =
            js_sys::Reflect::get(&tracing, &"getActiveSpan".into())?.dyn_into()?;
        let span = get.call0(&tracing)?;
        let set: js_sys::Function =
            js_sys::Reflect::get(&span, &"setAttribute".into())?.dyn_into()?;
        set.call2(
            &span,
            &"mskill.request_id".into(),
            &correlation.request_id.as_str().into(),
        )?;
        set.call2(
            &span,
            &"mskill.client_trace_id".into(),
            &JsValue::from_str(&correlation.traceparent[3..35]),
        )?;
        set.call2(&span, &"http.route".into(), &route.into())?;
        Ok(())
    };
    let _ = annotate();
}

/// Emit an actual JavaScript JSON object so Workers Logs indexes its fields.
/// Logging is best effort and cannot fail an otherwise successful user request.
pub fn log_event(event: &serde_json::Value, error: bool) {
    if let Ok(object) = js_sys::JSON::parse(&event.to_string()) {
        if error {
            web_sys::console::error_1(&object);
        } else {
            web_sys::console::log_1(&object);
        }
    }
}

/// Generate secure opaque identifiers with operating-platform WebCrypto.
pub fn random_hex(size: usize) -> worker::Result<String> {
    let crypto: web_sys::Crypto =
        js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))?
            .dyn_into()
            .map_err(|_| worker::Error::RustError("WebCrypto unavailable".into()))?;
    let mut bytes = vec![0; size];
    crypto.get_random_values_with_u8_array(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Accept only canonical v00 W3C trace headers, not unbounded arbitrary strings.
fn valid_traceparent(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.len() == 55
        && value.starts_with("00-")
        && bytes[35] == b'-'
        && bytes[52] == b'-'
        && bytes.iter().enumerate().all(|(i, b)| {
            matches!(i, 2 | 35 | 52) || b.is_ascii_digit() || (b'a'..=b'f').contains(b)
        })
        && bytes[3..35].iter().any(|b| *b != b'0')
        && bytes[36..52].iter().any(|b| *b != b'0')
}
