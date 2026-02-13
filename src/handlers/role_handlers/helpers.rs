/// Decode a URL-encoded string (form data): `+` → space, `%HH` → byte.
pub fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i+1..i+3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Parse URL-encoded form body, supporting duplicate keys (e.g. checkboxes).
pub fn parse_form_body(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((url_decode(k), url_decode(v)))
        })
        .collect()
}

pub fn get_field<'a>(params: &'a [(String, String)], key: &str) -> &'a str {
    params.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

pub fn get_all<'a>(params: &'a [(String, String)], key: &str) -> Vec<&'a str> {
    params.iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect()
}
