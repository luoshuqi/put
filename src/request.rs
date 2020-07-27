use crate::Group;
use curl::easy::{Easy, Form, List};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
};

static mut REQUEST_COUNTER: u32 = 0;

type Map = HashMap<String, String>;

#[derive(Deserialize, Debug)]
struct YamlRequest {
    path: Option<Map>,
    query: Option<Map>,
    header: Option<Map>,
    params: Option<Map>,
    form: Option<Map>,
    body: Option<String>,
    json: Option<serde_json::Value>,
}

#[derive(Default, Debug, Clone)]
pub struct RequestParam {
    pub path: Option<Map>,
    pub query: Option<Map>,
    pub header: Option<Map>,
    pub body: Option<Body>,
}

impl YamlRequest {
    fn into_param(self) -> serde_json::Result<RequestParam> {
        let body = if self.params.is_some() {
            Some(Body::Params(self.params.unwrap()))
        } else if self.form.is_some() {
            Some(Body::Form(self.form.unwrap()))
        } else if self.json.is_some() {
            Some(Body::Json(serde_json::to_string(&self.json.unwrap())?))
        } else if self.body.is_some() {
            Some(Body::Raw(self.body.unwrap()))
        } else {
            None
        };
        Ok(RequestParam {
            path: self.path,
            query: self.query,
            header: self.header,
            body,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Body {
    Params(Map),
    Form(Map),
    Json(String),
    Raw(String),
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: u32,
    pub method: String,
    pub url: String,
    pub param: RequestParam,
    pub base_url: Option<String>,
    pub raw: String,
}

#[derive(Debug)]
pub struct Response {
    pub body: String,
    pub header: String,
    pub time: u32,
}

impl Response {
    pub fn status(&self) -> Option<&str> {
        let pos = self.header.find("\r\n")?;
        let line = &self.header[..pos];
        let pos = line.find(' ')?;
        Some(&line[pos + 1..])
    }
}

#[derive(Debug)]
pub struct ParseError(String);

impl ParseError {
    fn create(msg: String) -> Box<ParseError> {
        Box::new(ParseError(msg))
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ParseError {}

impl Request {
    pub fn parse(yaml: String, group: Option<Group>) -> Result<Request, Box<dyn Error>> {
        let substituted = substitute_env(&yaml, group.as_ref());
        let s = substituted.as_ref().unwrap_or(&yaml);

        match get_method_url_line(s) {
            (Some(line), remain) => match parse_method_url(line) {
                Some((method, url)) => {
                    if remain.is_empty() {
                        Ok(Request {
                            id: next_id(),
                            method: method.to_uppercase(),
                            url: url.into(),
                            param: Default::default(),
                            base_url: group.and_then(|x| x.base_url),
                            raw: yaml,
                        })
                    } else {
                        let req: YamlRequest = serde_yaml::from_str(remain)?;
                        Ok(Request {
                            id: next_id(),
                            method: method.to_uppercase(),
                            url: url.into(),
                            param: req.into_param()?,
                            base_url: group.and_then(|x| x.base_url),
                            raw: yaml,
                        })
                    }
                }
                None => Err(ParseError::create(format!("invalid format: {}", line))),
            },
            _ => Err(ParseError::create("empty content".into())),
        }
    }

    pub fn perform(&self) -> Result<Response, Box<dyn Error>> {
        let mut easy = Easy::new();
        easy.follow_location(true)?;

        let mut url = match &self.base_url {
            Some(url) if !url_is_absolute(&self.url) => url.clone(),
            _ => String::new(),
        };

        if let Some(path_params) = &self.param.path {
            url.push_str(&replace_path_params(self.url.clone(), path_params));
        } else {
            url.push_str(&self.url);
        }

        if let Some(query) = &self.param.query {
            url.push('?');
            url.push_str(&http_build_query(query));
            easy.url(&url)?;
        } else {
            easy.url(&url)?;
        }

        let mut list = List::new();
        if let Some(m) = &self.param.header {
            for (k, v) in m {
                list.append(&format!("{}: {}", &k, &v))?;
            }
        }

        let method = self.method.to_uppercase();
        easy.custom_request(&method)?;

        match method.as_str() {
            "POST" | "PUT" | "PATCH" => match &self.param.body {
                Some(Body::Form(m)) => {
                    let mut form = Form::new();
                    for (k, v) in m {
                        form.part(&k).contents(v.as_bytes()).add()?;
                    }
                    easy.httppost(form)?;
                }
                Some(Body::Params(m)) => {
                    let s = http_build_query(m);
                    easy.post_fields_copy(s.as_bytes())?;
                    list.append("content-type: application/x-www-form-urlencoded")?;
                }
                Some(Body::Json(s)) => {
                    easy.post_field_size(s.len() as u64)?;
                    easy.post_fields_copy(s.as_bytes())?;
                    list.append("content-type: application/json")?;
                }
                Some(Body::Raw(s)) => {
                    easy.post_field_size(s.len() as u64)?;
                    easy.post_fields_copy(s.as_bytes())?;
                }
                None => (),
            },
            _ => (),
        }

        easy.http_headers(list)?;

        let mut body = Vec::new();
        let mut header = Vec::new();

        {
            let mut transfer = easy.transfer();

            transfer.write_function(|bytes| {
                body.extend_from_slice(bytes);
                Ok(bytes.len())
            })?;

            transfer.header_function(|bytes| {
                header.extend_from_slice(bytes);
                true
            })?;

            transfer.perform()?;
        }

        Ok(Response {
            body: String::from_utf8_lossy(&body).to_string(),
            header: String::from_utf8_lossy(&header).to_string(),
            time: easy.total_time()?.as_millis() as u32,
        })
    }
}

fn parse_method_url(s: &str) -> Option<(&str, &str)> {
    let s = s.trim();
    let i = s.find(" ")?;
    let method = &s[..i];
    let url = (&s[i + 1..]).trim();
    if !url.is_empty() {
        Some((method, url))
    } else {
        None
    }
}

fn get_method_url_line(s: &str) -> (Option<&str>, &str) {
    let s = s.trim();
    match s.find("\n") {
        Some(i) => {
            let line = &s[..i];
            if line.starts_with("#") {
                get_method_url_line(&s[i + 1..])
            } else {
                (Some(line), &s[i + 1..])
            }
        }
        None => {
            if s.is_empty() || s.starts_with("#") {
                (None, s)
            } else {
                (Some(s), "")
            }
        }
    }
}

fn next_id() -> u32 {
    unsafe {
        let id = REQUEST_COUNTER;
        REQUEST_COUNTER += 1;
        id
    }
}

fn http_build_query(query: &Map) -> String {
    let mut s = String::new();
    for (key, value) in query {
        s.push_str(&utf8_percent_encode(key, NON_ALPHANUMERIC).to_string());
        s.push('=');
        s.push_str(&utf8_percent_encode(value, NON_ALPHANUMERIC).to_string());
        s.push('&');
    }
    s.pop();
    s
}

fn substitute_env(s: &str, group: Option<&Group>) -> Option<String> {
    let env = group?.env.as_ref()?;
    if env.is_empty() {
        None
    } else {
        Some(substitute(s, env))
    }
}

fn substitute(s: &str, map: &Map) -> String {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut v = Vec::with_capacity(len);

    let mut i = 0;
    let mut j;
    while i < len - 1 {
        let next = bytes[i + 1];
        let brace = next == b'{';
        if bytes[i] == b'$' && (is_alpha_digit(next) || brace) {
            j = i + if brace { 2 } else { 1 };
            while j < len && (is_alpha_digit(bytes[j]) || bytes[j] == b'_') {
                j += 1;
            }

            let name = if brace {
                if j == len || bytes[j] != b'}' || j == i + 2 {
                    v.extend_from_slice(&bytes[i..j]);
                    i = j;
                    continue;
                } else {
                    &bytes[i + 2..j]
                }
            } else {
                &bytes[i + 1..j]
            };

            let name = String::from_utf8_lossy(name);
            if let Some(value) = map.get(&*name) {
                v.extend_from_slice(value.as_bytes());
            } else {
                if brace {
                    v.extend_from_slice(&bytes[i..=j]);
                } else {
                    v.extend_from_slice(&bytes[i..j]);
                }
            }
            i = j + if brace { 1 } else { 0 };
        } else {
            v.push(bytes[i]);
            i += 1;
        }
    }

    if i == len - 1 {
        v.push(bytes[i]);
    }

    String::from_utf8_lossy(&v).to_string()
}

#[inline]
fn is_alpha_digit(b: u8) -> bool {
    (b >= b'0' && b <= b'9') || (b >= b'a' && b <= b'z') || (b >= b'A' && b <= b'Z')
}

#[inline]
fn url_is_absolute(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn replace_path_params(mut path: String, params: &Map) -> String {
    let end_slash = if let Some(&b'/') = path.as_bytes().last() {
        true
    } else {
        path.push(b'/' as _);
        false
    };

    for (k, v) in params {
        path = path.replace(&format!("/:{}/", k), &format!("/{}/", v));
    }

    if !end_slash {
        path.pop();
    }

    return path;
}