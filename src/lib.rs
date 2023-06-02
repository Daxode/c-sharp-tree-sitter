use tree_sitter_highlight::{c, Error, Highlight, Highlighter, HighlightEvent, util};
use std::{fs, iter, mem, ops, ptr, slice, str, usize};
use std::ffi::CString;
use std::os::raw::c_char;
use std::process::abort;
use std::sync::atomic::AtomicUsize;

extern fn defeat() {
    tree_sitter_c_sharp::language();
    // let t = tree_sitter_highlight::c::ts_highlighter_new();
    let t = tree_sitter_highlight::c::ts_highlight_buffer_new();
}

#[test]
fn test_highlighting_via_c_api() {
    let highlight_types = vec![
        "tag\0".as_bytes().as_ptr() as *const c_char,
        "function\0".as_bytes().as_ptr() as *const c_char,
        "string\0".as_bytes().as_ptr() as *const c_char,
        "keyword\0".as_bytes().as_ptr() as *const c_char,
    ];
    
    let highlight_colors = vec![
        "ff22ff\0".as_bytes().as_ptr() as *const c_char,
        "55ff33\0".as_bytes().as_ptr() as *const c_char,
        "ffaa66\0".as_bytes().as_ptr() as *const c_char,
        "229955\0".as_bytes().as_ptr() as *const c_char,
    ];
    
    let highlighter = c::ts_highlighter_new(
        &highlight_types[0] as *const *const c_char,
        &highlight_colors[0] as *const *const c_char,
        highlight_types.len() as u32,
    );

    let source_code = c_string("class Foo { bar() { return \"baz\"; } }");
    
    let js_injection_regex = c_string("^javascript");
    let language = get_language("javascript");
    let queries = get_language_queries_path("javascript");
    let highlights_query = fs::read_to_string(queries.join("highlights.scm")).unwrap();
    let injections_query = fs::read_to_string(queries.join("injections.scm")).unwrap();
    let locals_query = fs::read_to_string(queries.join("locals.scm")).unwrap();
    c::ts_highlighter_add_language(
        highlighter,
        "source.cs".as_bytes().as_ptr(),
        js_injection_regex.as_ptr(),
        language,
        highlights_query.as_ptr() as *const c_char,
        injections_query.as_ptr() as *const c_char,
        locals_query.as_ptr() as *const c_char,
        highlights_query.len() as u32,
        injections_query.len() as u32,
        locals_query.len() as u32,
    );

    let html_scope = c_string("text.html.basic");
    let html_injection_regex = c_string("^html");
    let language = get_language("html");
    let queries = get_language_queries_path("html");
    let highlights_query = fs::read_to_string(queries.join("highlights.scm")).unwrap();
    let injections_query = fs::read_to_string(queries.join("injections.scm")).unwrap();
    c::ts_highlighter_add_language(
        highlighter,
        html_scope.as_ptr(),
        html_injection_regex.as_ptr(),
        language,
        highlights_query.as_ptr() as *const c_char,
        injections_query.as_ptr() as *const c_char,
        ptr::null(),
        highlights_query.len() as u32,
        injections_query.len() as u32,
        0,
    );

    let buffer = ts_highlight_buffer_new();

    c::ts_highlighter_highlight(
        highlighter,
        html_scope.as_ptr(),
        source_code.as_ptr(),
        source_code.as_bytes().len() as u32,
        buffer,
        ptr::null_mut(),
    );
    let output_bytes = c::ts_highlight_buffer_content(buffer);
    let output_line_offsets = c::ts_highlight_buffer_line_offsets(buffer);
    let output_len = c::ts_highlight_buffer_len(buffer);
    let output_line_count = c::ts_highlight_buffer_line_count(buffer);

    let output_bytes = unsafe { slice::from_raw_parts(output_bytes, output_len as usize) };
    let output_line_offsets =
        unsafe { slice::from_raw_parts(output_line_offsets, output_line_count as usize) };

    let mut lines = Vec::new();
    for i in 0..(output_line_count as usize) {
        let line_start = output_line_offsets[i] as usize;
        let line_end = output_line_offsets
            .get(i + 1)
            .map(|x| *x as usize)
            .unwrap_or(output_bytes.len());
        lines.push(str::from_utf8(&output_bytes[line_start..line_end]).unwrap());
    }

    assert_eq!(
        lines,
        vec![
            "&lt;<span class=tag>script</span>&gt;\n",
            "<span class=keyword>const</span> a = <span class=function>b</span>(<span class=string>&#39;c&#39;</span>);\n",
            "c.<span class=function>d</span>();\n",
            "&lt;/<span class=tag>script</span>&gt;\n",
        ]
    );

    c::ts_highlighter_delete(highlighter);
    c::ts_highlight_buffer_delete(buffer);
}

pub struct TSHighlightBuffer {
    highlighter: Highlighter,
    renderer: ColorTagRenderer,
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_new() -> *mut TSHighlightBuffer {
    Box::into_raw(Box::new(TSHighlightBuffer {
        highlighter: Highlighter::new(),
        renderer: ColorTagRenderer::new(),
    }))
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_delete(this: *mut TSHighlightBuffer) {
    drop(unsafe { Box::from_raw(this) })
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_content(this: *const TSHighlightBuffer) -> *const u8 {
    let this = unwrap_ptr(this);
    this.renderer.html.as_slice().as_ptr()
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_line_offsets(this: *const TSHighlightBuffer) -> *const u32 {
    let this = unwrap_ptr(this);
    this.renderer.line_offsets.as_slice().as_ptr()
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_len(this: *const TSHighlightBuffer) -> u32 {
    let this = unwrap_ptr(this);
    this.renderer.html.len() as u32
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_line_count(this: *const TSHighlightBuffer) -> u32 {
    let this = unwrap_ptr(this);
    this.renderer.line_offsets.len() as u32
}

/// Converts a general-purpose syntax highlighting iterator into a sequence of lines of HTML.
pub struct ColorTagRenderer {
    pub html: Vec<u8>,
    pub line_offsets: Vec<u32>,
    carriage_return_highlight: Option<Highlight>,
}

const CANCELLATION_CHECK_INTERVAL: usize = 100;
const BUFFER_HTML_RESERVE_CAPACITY: usize = 10 * 1024;
const BUFFER_LINES_RESERVE_CAPACITY: usize = 1000;

impl ColorTagRenderer {
    pub fn new() -> Self {
        let mut result = ColorTagRenderer {
            html: Vec::with_capacity(BUFFER_HTML_RESERVE_CAPACITY),
            line_offsets: Vec::with_capacity(BUFFER_LINES_RESERVE_CAPACITY),
            carriage_return_highlight: None,
        };
        result.line_offsets.push(0);
        result
    }

    pub fn set_carriage_return_highlight(&mut self, highlight: Option<Highlight>) {
        self.carriage_return_highlight = highlight;
    }

    pub fn reset(&mut self) {
        shrink_and_clear(&mut self.html, BUFFER_HTML_RESERVE_CAPACITY);
        shrink_and_clear(&mut self.line_offsets, BUFFER_LINES_RESERVE_CAPACITY);
        self.line_offsets.push(0);
    }

    pub fn render<'a, F>(
        &mut self,
        highlighter: impl Iterator<Item = Result<HighlightEvent, Error>>,
        source: &'a [u8],
        hexcolor_callback: &F,
    ) -> Result<(), Error>
        where
            F: Fn(Highlight) -> &'a [u8],
    {
        let mut highlights = Vec::new();
        for event in highlighter {
            match event {
                Ok(HighlightEvent::HighlightStart(s)) => {
                    highlights.push(s);
                    self.start_highlight(s, hexcolor_callback);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    highlights.pop();
                    self.end_highlight();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    self.add_text(&source[start..end], &highlights, hexcolor_callback);
                }
                Err(a) => return Err(a),
            }
        }
        if self.html.last() != Some(&b'\n') {
            self.html.push(b'\n');
        }
        if self.line_offsets.last() == Some(&(self.html.len() as u32)) {
            self.line_offsets.pop();
        }
        Ok(())
    }

    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.line_offsets
            .iter()
            .enumerate()
            .map(move |(i, line_start)| {
                let line_start = *line_start as usize;
                let line_end = if i + 1 == self.line_offsets.len() {
                    self.html.len()
                } else {
                    self.line_offsets[i + 1] as usize
                };
                str::from_utf8(&self.html[line_start..line_end]).unwrap()
            })
    }

    fn add_carriage_return<'a, F>(&mut self, hexcolor_callback: &F)
        where
            F: Fn(Highlight) -> &'a [u8],
    {
        if let Some(highlight) = self.carriage_return_highlight {
            let hexcolor_string = (hexcolor_callback)(highlight);
            if !hexcolor_string.is_empty() {
                self.html.extend(b"<color=");
                self.html.extend(hexcolor_string);
                self.html.extend(b"></color>");
            }
        }
    }

    fn start_highlight<'a, F>(&mut self, h: Highlight, hexcolor_callback: &F)
        where
            F: Fn(Highlight) -> &'a [u8],
    {
        let hexcolor_string = (hexcolor_callback)(h);
        if !hexcolor_string.is_empty() {
            self.html.extend(b"<color=#");
            self.html.extend(hexcolor_string);
            self.html.extend(b">");
        }
    }

    fn end_highlight(&mut self) {
        self.html.extend(b"</color>");
    }

    fn add_text<'a, F>(&mut self, src: &[u8], highlights: &Vec<Highlight>, hexcolor_callback: &F)
        where
            F: Fn(Highlight) -> &'a [u8],
    {
        let mut last_char_was_cr = false;
        for c in LossyUtf8::new(src).flat_map(|p| p.bytes()) {
            // Don't render carriage return characters, but allow lone carriage returns (not
            // followed by line feeds) to be styled via the hexcolor callback.
            if c == b'\r' {
                last_char_was_cr = true;
                continue;
            }
            if last_char_was_cr {
                if c != b'\n' {
                    self.add_carriage_return(hexcolor_callback);
                }
                last_char_was_cr = false;
            }

            // At line boundaries, close and re-open all of the open tags.
            if c == b'\n' {
                highlights.iter().for_each(|_| self.end_highlight());
                self.html.push(c);
                self.line_offsets.push(self.html.len() as u32);
                highlights
                    .iter()
                    .for_each(|scope| self.start_highlight(*scope, hexcolor_callback));
            } else if let Some(escape) = util::html_escape(c) {
                self.html.extend_from_slice(escape);
            } else {
                self.html.push(c);
            }
        }
    }
}

fn shrink_and_clear<T>(vec: &mut Vec<T>, capacity: usize) {
    if vec.len() > capacity {
        vec.truncate(capacity);
        vec.shrink_to_fit();
    }
    vec.clear();
}

// TODO: Remove this struct at at some point. If `core::str::lossy::Utf8Lossy`
// is ever stabilized.
pub struct LossyUtf8<'a> {
    bytes: &'a [u8],
    in_replacement: bool,
}

impl<'a> LossyUtf8<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        LossyUtf8 {
            bytes,
            in_replacement: false,
        }
    }
}

impl<'a> Iterator for LossyUtf8<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.bytes.is_empty() {
            return None;
        }
        if self.in_replacement {
            self.in_replacement = false;
            return Some("\u{fffd}");
        }
        match std::str::from_utf8(self.bytes) {
            Ok(valid) => {
                self.bytes = &[];
                Some(valid)
            }
            Err(error) => {
                if let Some(error_len) = error.error_len() {
                    let error_start = error.valid_up_to();
                    if error_start > 0 {
                        let result =
                            unsafe { std::str::from_utf8_unchecked(&self.bytes[..error_start]) };
                        self.bytes = &self.bytes[(error_start + error_len)..];
                        self.in_replacement = true;
                        Some(result)
                    } else {
                        self.bytes = &self.bytes[error_len..];
                        Some("\u{fffd}")
                    }
                } else {
                    None
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn ts_highlighter_highlight(
    this: *const TSHighlighter,
    scope_name: *const c_char,
    source_code: *const c_char,
    source_code_len: u32,
    output: *mut TSHighlightBuffer,
    cancellation_flag: *const AtomicUsize,
) -> ErrorCode {
    let this = unwrap_ptr(this);
    let output = unwrap_mut_ptr(output);
    let scope_name = unwrap(unsafe { CStr::from_ptr(scope_name).to_str() });
    let source_code =
        unsafe { slice::from_raw_parts(source_code as *const u8, source_code_len as usize) };
    let cancellation_flag = unsafe { cancellation_flag.as_ref() };
    this.highlight(source_code, scope_name, output, cancellation_flag)
}

impl c::TSHighlighter {
    fn highlight(
        &self,
        source_code: &[u8],
        scope_name: &str,
        output: &mut TSHighlightBuffer,
        cancellation_flag: Option<&AtomicUsize>,
    ) -> ErrorCode {
        let entry = self.languages.get(scope_name);
        if entry.is_none() {
            return ErrorCode::UnknownScope;
        }
        let (_, configuration) = entry.unwrap();
        let languages = &self.languages;

        let highlights = output.highlighter.highlight(
            configuration,
            source_code,
            cancellation_flag,
            move |injection_string| {
                languages.values().find_map(|(injection_regex, config)| {
                    injection_regex.as_ref().and_then(|regex| {
                        if regex.is_match(injection_string) {
                            Some(config)
                        } else {
                            None
                        }
                    })
                })
            },
        );

        if let Ok(highlights) = highlights {
            output.renderer.reset();
            output
                .renderer
                .set_carriage_return_highlight(self.carriage_return_index.map(Highlight));
            let result = output
                .renderer
                .render(highlights, source_code, &|s| self.attribute_strings[s.0]);
            match result {
                Err(Error::Cancelled) => {
                    return ErrorCode::Timeout;
                }
                Err(Error::InvalidLanguage) => {
                    return ErrorCode::InvalidLanguage;
                }
                Err(Error::Unknown) => {
                    return ErrorCode::Timeout;
                }
                Ok(()) => ErrorCode::Ok,
            }
        } else {
            ErrorCode::Timeout
        }
    }
}


fn c_string(s: &str) -> CString {
    CString::new(s.as_bytes().to_vec()).unwrap()
}

fn unwrap_ptr<'a, T>(result: *const T) -> &'a T {
    unsafe { result.as_ref() }.unwrap_or_else(|| {
        eprintln!("{}:{} - pointer must not be null", file!(), line!());
        abort();
    })
}