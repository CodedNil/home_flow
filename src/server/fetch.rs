use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Debug, Default)]
pub struct Headers {
    pub headers: Vec<(String, String)>,
}

impl Headers {
    fn new(headers: &[(&str, &str)]) -> Self {
        Self {
            headers: headers
                .iter()
                .map(|e| (e.0.to_owned(), e.1.to_owned()))
                .collect(),
        }
    }

    fn insert(&mut self, key: impl ToString, value: impl ToString) {
        self.headers.push((key.to_string(), value.to_string()));
    }
}

impl IntoIterator for Headers {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.headers.into_iter()
    }
}

impl<'h> IntoIterator for &'h Headers {
    type Item = &'h (String, String);
    type IntoIter = std::slice::Iter<'h, (String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.headers.iter()
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub body: Vec<u8>,
    pub headers: Headers,
}

impl Request {
    pub fn get(url: impl ToString) -> Self {
        Self {
            method: "GET".to_owned(),
            url: url.to_string(),
            body: vec![],
            headers: Headers::new(&[("Accept", "*/*")]),
        }
    }

    pub fn post(url: impl ToString, body: Vec<u8>) -> Self {
        Self {
            method: "POST".to_owned(),
            url: url.to_string(),
            body,
            headers: Headers::new(&[("Accept", "*/*")]),
        }
    }
}

#[derive(Clone)]
pub struct Response {
    pub url: String,
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: Headers,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            url,
            ok,
            status,
            status_text,
            headers,
            bytes,
        } = self;

        fmt.debug_struct("Response")
            .field("url", url)
            .field("ok", ok)
            .field("status", status)
            .field("status_text", status_text)
            .field("headers", headers)
            .field("bytes", &format!("{} bytes", bytes.len()))
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct PartialResponse {
    url: String,
    ok: bool,
    status: u16,
    status_text: String,
    headers: Headers,
}

pub type Error = String;
pub type CustomResult<T> = std::result::Result<T, Error>;

async fn fetch_async(request: &Request) -> CustomResult<Response> {
    fetch_jsvalue(request)
        .await
        .map_err(string_from_fetch_error)
}

fn string_from_fetch_error(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| {
        if value.has_type::<js_sys::TypeError>() {
            web_sys::console::error_1(&value);
            "Failed to fetch, check the developer console for details".to_owned()
        } else {
            format!("{:#?}", value)
        }
    })
}

async fn fetch_base(request: &Request) -> Result<web_sys::Response, JsValue> {
    let mut opts = web_sys::RequestInit::new();
    opts.method(&request.method);
    opts.mode(web_sys::RequestMode::Cors);

    if !request.body.is_empty() {
        let body_bytes: &[u8] = &request.body;
        let body_array: js_sys::Uint8Array = body_bytes.into();
        let js_value: &JsValue = body_array.as_ref();
        opts.body(Some(js_value));
    }

    let js_request = web_sys::Request::new_with_str_and_init(&request.url, &opts)?;

    for (k, v) in &request.headers {
        js_request.headers().set(k, v)?;
    }

    let window = web_sys::window().unwrap();
    let response = JsFuture::from(window.fetch_with_request(&js_request)).await?;
    let response: web_sys::Response = response.dyn_into()?;

    Ok(response)
}

fn get_response_base(response: &web_sys::Response) -> Result<PartialResponse, JsValue> {
    let js_headers: web_sys::Headers = response.headers();
    let js_iter = js_sys::try_iter(&js_headers)
        .expect("headers try_iter")
        .expect("headers have an iterator");

    let mut headers = Headers::default();
    for item in js_iter {
        let item = item.expect("headers iterator");
        let array: js_sys::Array = item.into();
        let v: Vec<JsValue> = array.to_vec();

        let key = v[0]
            .as_string()
            .ok_or_else(|| JsValue::from_str("headers name"))?;
        let value = v[1]
            .as_string()
            .ok_or_else(|| JsValue::from_str("headers value"))?;

        headers.insert(key, value);
    }

    Ok(PartialResponse {
        url: response.url(),
        ok: response.ok(),
        status: response.status(),
        status_text: response.status_text(),
        headers,
    })
}

async fn fetch_jsvalue(request: &Request) -> Result<Response, JsValue> {
    let response = fetch_base(request).await?;

    let array_buffer = JsFuture::from(response.array_buffer()?).await?;
    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    let bytes = uint8_array.to_vec();

    let base = get_response_base(&response)?;

    Ok(Response {
        url: base.url,
        ok: base.ok,
        status: base.status,
        status_text: base.status_text,
        bytes,
        headers: base.headers,
    })
}

fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

pub fn fetch(request: Request, on_done: Box<dyn FnOnce(CustomResult<Response>) + Send>) {
    spawn_future(async move {
        let result = fetch_async(&request).await;
        on_done(result)
    });
}
