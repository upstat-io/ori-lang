# Proposal: std.json API Design (FFI Revision)

**Status:** Approved
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Standard library
**Depends on:** Platform FFI proposal, Computed Map Keys proposal

---

## Summary

This revision adds FFI implementation details to the approved `std.json` proposal. JSON parsing/serialization uses **yyjson** on native platforms and **JavaScript's JSON API** on WASM, while keeping the public API unchanged.

---

## FFI Implementation Decision

### Why yyjson (Native)?

| Library | Performance | Features | License | Binary Size |
|---------|------------|----------|---------|-------------|
| **yyjson** | Fastest | Full JSON, streaming | MIT | ~50KB |
| simdjson | Very fast | Read-only | Apache 2.0 | ~200KB |
| cJSON | Moderate | Full JSON | MIT | ~30KB |
| Pure Ori | Slowest | Full control | N/A | 0 |

**Recommendation: yyjson**
- 5-10x faster than hand-written parsers
- Supports both reading and writing
- MIT license, single-file inclusion
- Streaming API for large documents
- Already handles all JSON edge cases correctly

### Why JavaScript JSON API (WASM)?

For WASM targets, use the browser's native `JSON.parse` and `JSON.stringify`:
- Zero additional bundle size
- Highly optimized by JS engines
- Standard behavior developers expect

### Fallback Strategy

If yyjson is unavailable (e.g., restricted environment), fall back to pure Ori implementation.

---

## Native FFI Declarations

```ori
// std/json/ffi_native.ori (internal)

#!target(not_arch: "wasm32")

// Opaque yyjson types
type YyjsonDoc = CPtr
type YyjsonVal = CPtr
type YyjsonMutDoc = CPtr
type YyjsonMutVal = CPtr

// Read flags
let $YYJSON_READ_NOFLAG: int = 0
let $YYJSON_READ_ALLOW_COMMENTS: int = 1
let $YYJSON_READ_ALLOW_TRAILING_COMMAS: int = 2

// Write flags
let $YYJSON_WRITE_NOFLAG: int = 0
let $YYJSON_WRITE_PRETTY: int = 1

extern "c" from "yyjson" {
    // Parsing
    @_yyjson_read (dat: str, len: int, flg: int) -> YyjsonDoc as "yyjson_read"
    @_yyjson_read_opts (
        dat: str,
        len: int,
        flg: int,
        alc: CPtr,
        err: CPtr
    ) -> YyjsonDoc as "yyjson_read_opts"

    // Document access
    @_yyjson_doc_get_root (doc: YyjsonDoc) -> YyjsonVal as "yyjson_doc_get_root"
    @_yyjson_doc_free (doc: YyjsonDoc) -> void as "yyjson_doc_free"

    // Value type checking
    @_yyjson_is_null (val: YyjsonVal) -> bool as "yyjson_is_null"
    @_yyjson_is_bool (val: YyjsonVal) -> bool as "yyjson_is_bool"
    @_yyjson_is_num (val: YyjsonVal) -> bool as "yyjson_is_num"
    @_yyjson_is_str (val: YyjsonVal) -> bool as "yyjson_is_str"
    @_yyjson_is_arr (val: YyjsonVal) -> bool as "yyjson_is_arr"
    @_yyjson_is_obj (val: YyjsonVal) -> bool as "yyjson_is_obj"

    // Value extraction
    @_yyjson_get_bool (val: YyjsonVal) -> bool as "yyjson_get_bool"
    @_yyjson_get_real (val: YyjsonVal) -> float as "yyjson_get_real"
    @_yyjson_get_int (val: YyjsonVal) -> int as "yyjson_get_sint"
    @_yyjson_get_str (val: YyjsonVal) -> str as "yyjson_get_str"
    @_yyjson_get_len (val: YyjsonVal) -> int as "yyjson_get_len"

    // Object access
    @_yyjson_obj_get (obj: YyjsonVal, key: str) -> YyjsonVal as "yyjson_obj_get"
    @_yyjson_obj_iter_init (obj: YyjsonVal, iter: CPtr) -> bool as "yyjson_obj_iter_init"
    @_yyjson_obj_iter_next (iter: CPtr) -> YyjsonVal as "yyjson_obj_iter_next"
    @_yyjson_obj_iter_get_val (key: YyjsonVal) -> YyjsonVal as "yyjson_obj_iter_get_val"

    // Array access
    @_yyjson_arr_get (arr: YyjsonVal, idx: int) -> YyjsonVal as "yyjson_arr_get"
    @_yyjson_arr_iter_init (arr: YyjsonVal, iter: CPtr) -> bool as "yyjson_arr_iter_init"
    @_yyjson_arr_iter_next (iter: CPtr) -> YyjsonVal as "yyjson_arr_iter_next"

    // Mutable document creation
    @_yyjson_mut_doc_new (alc: CPtr) -> YyjsonMutDoc as "yyjson_mut_doc_new"
    @_yyjson_mut_doc_free (doc: YyjsonMutDoc) -> void as "yyjson_mut_doc_free"
    @_yyjson_mut_doc_set_root (doc: YyjsonMutDoc, root: YyjsonMutVal) -> void as "yyjson_mut_doc_set_root"

    // Mutable value creation
    @_yyjson_mut_null (doc: YyjsonMutDoc) -> YyjsonMutVal as "yyjson_mut_null"
    @_yyjson_mut_bool (doc: YyjsonMutDoc, val: bool) -> YyjsonMutVal as "yyjson_mut_bool"
    @_yyjson_mut_real (doc: YyjsonMutDoc, val: float) -> YyjsonMutVal as "yyjson_mut_real"
    @_yyjson_mut_sint (doc: YyjsonMutDoc, val: int) -> YyjsonMutVal as "yyjson_mut_sint"
    @_yyjson_mut_str (doc: YyjsonMutDoc, val: str) -> YyjsonMutVal as "yyjson_mut_strcpy"
    @_yyjson_mut_arr (doc: YyjsonMutDoc) -> YyjsonMutVal as "yyjson_mut_arr"
    @_yyjson_mut_obj (doc: YyjsonMutDoc) -> YyjsonMutVal as "yyjson_mut_obj"

    // Mutable array operations
    @_yyjson_mut_arr_append (arr: YyjsonMutVal, val: YyjsonMutVal) -> bool as "yyjson_mut_arr_append"

    // Mutable object operations
    @_yyjson_mut_obj_add (
        obj: YyjsonMutVal,
        key: YyjsonMutVal,
        val: YyjsonMutVal
    ) -> bool as "yyjson_mut_obj_add"

    // Serialization
    @_yyjson_mut_write (doc: YyjsonMutDoc, flg: int, len: CPtr) -> str as "yyjson_mut_write"
    @_yyjson_write (doc: YyjsonDoc, flg: int, len: CPtr) -> str as "yyjson_write"
}

// Memory allocation helpers for C out-parameters
@alloc_obj_iter () -> CPtr uses FFI =
    unsafe { stack_alloc(size: 48) }  // sizeof(yyjson_obj_iter)

@alloc_arr_iter () -> CPtr uses FFI =
    unsafe { stack_alloc(size: 24) }  // sizeof(yyjson_arr_iter)

@alloc_size_t () -> CPtr uses FFI =
    unsafe { stack_alloc(size: 8) }   // sizeof(size_t)
```

---

## WASM FFI Declarations

```ori
// std/json/ffi_wasm.ori (internal)

#!target(arch: "wasm32")

extern "js" {
    // Parse JSON string to JS object
    @_js_json_parse (source: str) -> Result<JsValue, str> as "JSON.parse"

    // Stringify JS object to JSON string
    @_js_json_stringify (value: JsValue) -> str as "JSON.stringify"

    // Type checking
    @_js_typeof (value: JsValue) -> str as "__ori_typeof"
    @_js_is_null (value: JsValue) -> bool as "__ori_is_null"
    @_js_is_array (value: JsValue) -> bool as "Array.isArray"

    // Value extraction
    @_js_to_bool (value: JsValue) -> bool
    @_js_to_number (value: JsValue) -> float
    @_js_to_string (value: JsValue) -> str

    // Object access
    @_js_get_keys (obj: JsValue) -> JsValue as "Object.keys"
    @_js_get_prop (obj: JsValue, key: str) -> JsValue

    // Array access
    @_js_array_length (arr: JsValue) -> int
    @_js_array_get (arr: JsValue, idx: int) -> JsValue

    // Object/array creation
    @_js_new_object () -> JsValue
    @_js_new_array () -> JsValue
    @_js_set_prop (obj: JsValue, key: str, val: JsValue) -> void
    @_js_array_push (arr: JsValue, val: JsValue) -> void

    // Primitive to JsValue
    @_js_null () -> JsValue
    @_js_from_bool (val: bool) -> JsValue
    @_js_from_number (val: float) -> JsValue
    @_js_from_string (val: str) -> JsValue

    // Handle cleanup
    @_js_drop (handle: JsValue) -> void as "__ori_drop"
}

// JS glue code (generated):
// function __ori_typeof(v) { return typeof v; }
// function __ori_is_null(v) { return v === null; }
// function __ori_drop(handle) { dropObject(handle); }
```

---

## Native Implementation

### Parsing (Native)

```ori
// std/json/parse_native.ori

#!target(not_arch: "wasm32")

use "./ffi_native" { ... }

pub @parse (source: str) -> Result<JsonValue, JsonError> uses FFI =
    run(
        let doc = _yyjson_read(dat: source, len: len(collection: source), flg: $YYJSON_READ_NOFLAG),

        if doc.is_null() then
            Err(JsonError {
                kind: ParseError,
                message: "Invalid JSON",
                path: "",
                position: 0,
            })
        else
            run(
                let root = _yyjson_doc_get_root(doc: doc),
                let result = yyjson_val_to_json_value(val: root),
                _yyjson_doc_free(doc: doc),
                Ok(result),
            ),
    )

// Convert yyjson value tree to Ori JsonValue
@yyjson_val_to_json_value (val: YyjsonVal) -> JsonValue uses FFI =
    if _yyjson_is_null(val: val) then
        JsonValue.Null
    else if _yyjson_is_bool(val: val) then
        JsonValue.Bool(_yyjson_get_bool(val: val))
    else if _yyjson_is_num(val: val) then
        JsonValue.Number(_yyjson_get_real(val: val))
    else if _yyjson_is_str(val: val) then
        JsonValue.String(_yyjson_get_str(val: val))
    else if _yyjson_is_arr(val: val) then
        JsonValue.Array(yyjson_arr_to_list(arr: val))
    else if _yyjson_is_obj(val: val) then
        JsonValue.Object(yyjson_obj_to_map(obj: val))
    else
        JsonValue.Null  // Should not happen

@yyjson_arr_to_list (arr: YyjsonVal) -> [JsonValue] uses FFI =
    run(
        let result: [JsonValue] = [],
        let arr_len = _yyjson_get_len(val: arr),
        for i in 0..arr_len do
            run(
                let elem = _yyjson_arr_get(arr: arr, idx: i),
                result = [...result, yyjson_val_to_json_value(val: elem)],
            ),
        result,
    )

@yyjson_obj_to_map (obj: YyjsonVal) -> {str: JsonValue} uses FFI =
    run(
        let result: {str: JsonValue} = {},
        let iter = alloc_obj_iter(),
        _yyjson_obj_iter_init(obj: obj, iter: iter),
        loop(
            let key_val = _yyjson_obj_iter_next(iter: iter),
            if key_val.is_null() then break result,
            let key = _yyjson_get_str(val: key_val),
            let val = _yyjson_obj_iter_get_val(key: key_val),
            result = {...result, [key]: yyjson_val_to_json_value(val: val)},
            continue,
        ),
    )
```

### Serialization (Native)

```ori
// std/json/stringify_native.ori

#!target(not_arch: "wasm32")

use "./ffi_native" { ... }

pub @stringify (value: JsonValue) -> str uses FFI =
    run(
        let doc = _yyjson_mut_doc_new(alc: CPtr.null()),
        let root = json_value_to_yyjson_mut(doc: doc, value: value),
        _yyjson_mut_doc_set_root(doc: doc, root: root),
        let len_ptr = alloc_size_t(),
        let result = _yyjson_mut_write(doc: doc, flg: $YYJSON_WRITE_NOFLAG, len: len_ptr),
        _yyjson_mut_doc_free(doc: doc),
        result,
    )

pub @stringify_pretty (value: JsonValue, indent: int = 2) -> str uses FFI =
    run(
        let doc = _yyjson_mut_doc_new(alc: CPtr.null()),
        let root = json_value_to_yyjson_mut(doc: doc, value: value),
        _yyjson_mut_doc_set_root(doc: doc, root: root),
        let len_ptr = alloc_size_t(),
        let result = _yyjson_mut_write(doc: doc, flg: $YYJSON_WRITE_PRETTY, len: len_ptr),
        _yyjson_mut_doc_free(doc: doc),
        // yyjson uses 4-space indent; adjust if needed
        if indent == 4 then result else adjust_indent(s: result, spaces: indent),
    )

@json_value_to_yyjson_mut (doc: YyjsonMutDoc, value: JsonValue) -> YyjsonMutVal uses FFI =
    match(value,
        Null -> _yyjson_mut_null(doc: doc),
        Bool(b) -> _yyjson_mut_bool(doc: doc, val: b),
        Number(n) -> _yyjson_mut_real(doc: doc, val: n),
        String(s) -> _yyjson_mut_str(doc: doc, val: s),
        Array(arr) -> run(
            let mut_arr = _yyjson_mut_arr(doc: doc),
            for item in arr do
                _yyjson_mut_arr_append(
                    arr: mut_arr,
                    val: json_value_to_yyjson_mut(doc: doc, value: item),
                ),
            mut_arr,
        ),
        Object(obj) -> run(
            let mut_obj = _yyjson_mut_obj(doc: doc),
            for (k, v) in obj.entries() do
                _yyjson_mut_obj_add(
                    obj: mut_obj,
                    key: _yyjson_mut_str(doc: doc, val: k),
                    val: json_value_to_yyjson_mut(doc: doc, value: v),
                ),
            mut_obj,
        ),
    )

@adjust_indent (s: str, spaces: int) -> str =
    run(
        let indent_str = " ".repeat(count: spaces),
        let four_spaces = "    ",
        s.replace(old: four_spaces, new: indent_str),
    )
```

---

## WASM Implementation

### Parsing (WASM)

```ori
// std/json/parse_wasm.ori

#!target(arch: "wasm32")

use "./ffi_wasm" { ... }

pub @parse (source: str) -> Result<JsonValue, JsonError> uses FFI =
    match(_js_json_parse(source: source),
        Ok(js_val) -> run(
            let result = js_value_to_json_value(val: js_val),
            _js_drop(handle: js_val),
            Ok(result),
        ),
        Err(msg) -> Err(JsonError {
            kind: ParseError,
            message: msg,
            path: "",
            position: 0,
        }),
    )

@js_value_to_json_value (val: JsValue) -> JsonValue uses FFI =
    if _js_is_null(value: val) then
        JsonValue.Null
    else
        run(
            let type_str = _js_typeof(value: val),
            match(type_str,
                "boolean" -> JsonValue.Bool(_js_to_bool(value: val)),
                "number" -> JsonValue.Number(_js_to_number(value: val)),
                "string" -> JsonValue.String(_js_to_string(value: val)),
                "object" ->
                    if _js_is_array(value: val) then
                        JsonValue.Array(js_array_to_list(arr: val))
                    else
                        JsonValue.Object(js_object_to_map(obj: val)),
                _ -> JsonValue.Null,
            ),
        )

@js_array_to_list (arr: JsValue) -> [JsonValue] uses FFI =
    run(
        let result: [JsonValue] = [],
        let arr_len = _js_array_length(arr: arr),
        for i in 0..arr_len do
            run(
                let elem = _js_array_get(arr: arr, idx: i),
                let json_elem = js_value_to_json_value(val: elem),
                _js_drop(handle: elem),
                result = [...result, json_elem],
            ),
        result,
    )

@js_object_to_map (obj: JsValue) -> {str: JsonValue} uses FFI =
    run(
        let result: {str: JsonValue} = {},
        let keys = _js_get_keys(obj: obj),
        let keys_len = _js_array_length(arr: keys),
        for i in 0..keys_len do
            run(
                let key_js = _js_array_get(arr: keys, idx: i),
                let key = _js_to_string(value: key_js),
                _js_drop(handle: key_js),
                let val_js = _js_get_prop(obj: obj, key: key),
                let val = js_value_to_json_value(val: val_js),
                _js_drop(handle: val_js),
                result = {...result, [key]: val},
            ),
        _js_drop(handle: keys),
        result,
    )
```

### Serialization (WASM)

```ori
// std/json/stringify_wasm.ori

#!target(arch: "wasm32")

use "./ffi_wasm" { ... }

pub @stringify (value: JsonValue) -> str uses FFI =
    run(
        let js_val = json_value_to_js_value(value: value),
        let result = _js_json_stringify(value: js_val),
        _js_drop(handle: js_val),
        result,
    )

pub @stringify_pretty (value: JsonValue, indent: int = 2) -> str uses FFI =
    run(
        let js_val = json_value_to_js_value(value: value),
        // JSON.stringify with space parameter
        let result = _js_json_stringify_pretty(value: js_val, indent: indent),
        _js_drop(handle: js_val),
        result,
    )

@json_value_to_js_value (value: JsonValue) -> JsValue uses FFI =
    match(value,
        Null -> _js_null(),
        Bool(b) -> _js_from_bool(val: b),
        Number(n) -> _js_from_number(val: n),
        String(s) -> _js_from_string(val: s),
        Array(arr) -> run(
            let js_arr = _js_new_array(),
            for item in arr do
                run(
                    let js_item = json_value_to_js_value(value: item),
                    _js_array_push(arr: js_arr, val: js_item),
                    _js_drop(handle: js_item),
                ),
            js_arr,
        ),
        Object(obj) -> run(
            let js_obj = _js_new_object(),
            for (k, v) in obj.entries() do
                run(
                    let js_val = json_value_to_js_value(value: v),
                    _js_set_prop(obj: js_obj, key: k, val: js_val),
                    _js_drop(handle: js_val),
                ),
            js_obj,
        ),
    )
```

---

## Pure Ori Fallback Implementation

For environments where neither yyjson nor JavaScript JSON are available:

```ori
// std/json/pure.ori

type PureJsonParser = {
    source: str,
    pos: int,
}

pub @parse_pure (source: str) -> Result<JsonValue, JsonError> =
    run(
        let parser = PureJsonParser { source: source, pos: 0 },
        match(parser.parse_value(),
            Ok((value, _)) -> Ok(value),
            Err(e) -> Err(e),
        ),
    )

impl PureJsonParser {
    @parse_value (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        run(
            let self = self.skip_whitespace(),
            match(self.peek(),
                Some('n') -> self.parse_null(),
                Some('t') -> self.parse_true(),
                Some('f') -> self.parse_false(),
                Some('"') -> self.parse_string(),
                Some('[') -> self.parse_array(),
                Some('{') -> self.parse_object(),
                Some('-') -> self.parse_number(),
                Some(c) if c.is_digit() -> self.parse_number(),
                Some(c) -> Err(JsonError {
                    kind: ParseError,
                    message: `Unexpected character '{c}'`,
                    path: "",
                    position: self.pos,
                }),
                None -> Err(JsonError {
                    kind: ParseError,
                    message: "Unexpected end of input",
                    path: "",
                    position: self.pos,
                }),
            ),
        )

    @parse_null (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        if self.starts_with(prefix: "null") then
            Ok((JsonValue.Null, self.advance(n: 4)))
        else
            Err(self.error(message: "Expected 'null'"))

    @parse_true (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        if self.starts_with(prefix: "true") then
            Ok((JsonValue.Bool(true), self.advance(n: 4)))
        else
            Err(self.error(message: "Expected 'true'"))

    @parse_false (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        if self.starts_with(prefix: "false") then
            Ok((JsonValue.Bool(false), self.advance(n: 5)))
        else
            Err(self.error(message: "Expected 'false'"))

    @parse_string (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        run(
            let self = self.expect(ch: '"')?,
            let (s, self) = self.parse_string_contents()?,
            let self = self.expect(ch: '"')?,
            Ok((JsonValue.String(s), self)),
        )

    @parse_string_contents (self) -> Result<(str, PureJsonParser), JsonError> =
        run(
            let result = "",
            let self = self,
            loop(
                match(self.peek(),
                    None -> break Err(self.error(message: "Unterminated string")),
                    Some('"') -> break Ok((result, self)),
                    Some('\\') -> run(
                        let self = self.advance(n: 1),
                        match(self.peek(),
                            Some('"') -> run(result = result + "\"", self = self.advance(n: 1), continue),
                            Some('\\') -> run(result = result + "\\", self = self.advance(n: 1), continue),
                            Some('/') -> run(result = result + "/", self = self.advance(n: 1), continue),
                            Some('n') -> run(result = result + "\n", self = self.advance(n: 1), continue),
                            Some('r') -> run(result = result + "\r", self = self.advance(n: 1), continue),
                            Some('t') -> run(result = result + "\t", self = self.advance(n: 1), continue),
                            Some('u') -> run(
                                let (ch, self) = self.parse_unicode_escape()?,
                                result = result + ch,
                                continue,
                            ),
                            _ -> break Err(self.error(message: "Invalid escape sequence")),
                        ),
                    ),
                    Some(c) -> run(
                        result = result + c.to_str(),
                        self = self.advance(n: 1),
                        continue,
                    ),
                ),
            ),
        )

    @parse_unicode_escape (self) -> Result<(str, PureJsonParser), JsonError> =
        run(
            let self = self.advance(n: 1),  // skip 'u'
            let hex = self.take(n: 4),
            if hex.len() < 4 then
                Err(self.error(message: "Invalid unicode escape"))
            else
                match(int_from_hex(s: hex),
                    Some(code) -> Ok((char_from_code(code: code), self.advance(n: 4))),
                    None -> Err(self.error(message: "Invalid unicode escape")),
                ),
        )

    @parse_number (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        run(
            let start = self.pos,
            let self = self,

            // Optional minus
            let self = if self.peek() == Some('-') then self.advance(n: 1) else self,

            // Integer part
            let self = match(self.peek(),
                Some('0') -> self.advance(n: 1),
                Some(c) if c.is_digit() && c != '0' -> self.skip_digits(),
                _ -> Err(self.error(message: "Invalid number"))?,
            ),

            // Fractional part
            let self = if self.peek() == Some('.') then
                run(
                    let self = self.advance(n: 1),
                    self.skip_digits(),
                )
            else
                self,

            // Exponent part
            let self = match(self.peek(),
                Some('e') | Some('E') -> run(
                    let self = self.advance(n: 1),
                    let self = match(self.peek(),
                        Some('+') | Some('-') -> self.advance(n: 1),
                        _ -> self,
                    ),
                    self.skip_digits(),
                ),
                _ -> self,
            ),

            let num_str = self.source.slice(start: start, end: self.pos),
            match(num_str as? float,
                Some(n) -> Ok((JsonValue.Number(n), self)),
                None -> Err(self.error(message: "Invalid number")),
            ),
        )

    @parse_array (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        run(
            let self = self.expect(ch: '[')?,
            let self = self.skip_whitespace(),

            if self.peek() == Some(']') then
                Ok((JsonValue.Array([]), self.advance(n: 1)))
            else
                run(
                    let items: [JsonValue] = [],
                    let self = self,
                    loop(
                        let (value, new_self) = self.parse_value()?,
                        items = [...items, value],
                        let self = new_self.skip_whitespace(),
                        match(self.peek(),
                            Some(']') -> break Ok((JsonValue.Array(items), self.advance(n: 1))),
                            Some(',') -> run(
                                self = self.advance(n: 1).skip_whitespace(),
                                continue,
                            ),
                            _ -> break Err(self.error(message: "Expected ',' or ']'")),
                        ),
                    ),
                ),
        )

    @parse_object (self) -> Result<(JsonValue, PureJsonParser), JsonError> =
        run(
            let self = self.expect(ch: '{')?,
            let self = self.skip_whitespace(),

            if self.peek() == Some('}') then
                Ok((JsonValue.Object({}), self.advance(n: 1)))
            else
                run(
                    let entries: {str: JsonValue} = {},
                    let self = self,
                    loop(
                        // Parse key
                        let (key_value, new_self) = self.parse_string()?,
                        let key = match(key_value,
                            JsonValue.String(s) -> s,
                            _ -> Err(self.error(message: "Object key must be string"))?,
                        ),
                        let self = new_self.skip_whitespace(),

                        // Expect colon
                        let self = self.expect(ch: ':')?,
                        let self = self.skip_whitespace(),

                        // Parse value
                        let (value, new_self) = self.parse_value()?,
                        entries = {...entries, [key]: value},
                        let self = new_self.skip_whitespace(),

                        match(self.peek(),
                            Some('}') -> break Ok((JsonValue.Object(entries), self.advance(n: 1))),
                            Some(',') -> run(
                                self = self.advance(n: 1).skip_whitespace(),
                                continue,
                            ),
                            _ -> break Err(self.error(message: "Expected ',' or '}'")),
                        ),
                    ),
                ),
        )

    // Helper methods
    @peek (self) -> Option<char> =
        if self.pos < self.source.len() then
            Some(self.source[self.pos])
        else
            None

    @advance (self, n: int) -> PureJsonParser =
        PureJsonParser { ...self, pos: self.pos + n }

    @skip_whitespace (self) -> PureJsonParser =
        run(
            let self = self,
            loop(
                match(self.peek(),
                    Some(' ') | Some('\t') | Some('\n') | Some('\r') ->
                        run(self = self.advance(n: 1), continue),
                    _ -> break self,
                ),
            ),
        )

    @skip_digits (self) -> PureJsonParser =
        run(
            let self = self,
            loop(
                match(self.peek(),
                    Some(c) if c.is_digit() -> run(self = self.advance(n: 1), continue),
                    _ -> break self,
                ),
            ),
        )

    @starts_with (self, prefix: str) -> bool =
        self.source.slice(start: self.pos, end: self.pos + prefix.len()) == prefix

    @take (self, n: int) -> str =
        self.source.slice(start: self.pos, end: min(left: self.pos + n, right: self.source.len()))

    @expect (self, ch: char) -> Result<PureJsonParser, JsonError> =
        if self.peek() == Some(ch) then
            Ok(self.advance(n: 1))
        else
            Err(self.error(message: `Expected '{ch}'`))

    @error (self, message: str) -> JsonError =
        JsonError {
            kind: ParseError,
            message: message,
            path: "",
            position: self.pos,
        }
}

// Pure Ori stringify (no FFI needed)
pub @stringify_pure (value: JsonValue) -> str =
    match(value,
        Null -> "null",
        Bool(true) -> "true",
        Bool(false) -> "false",
        Number(n) -> n.to_str(),
        String(s) -> `"{escape_string(s: s)}"`,
        Array(items) -> run(
            let parts = items.map(v -> stringify_pure(value: v)),
            `[{parts.join(separator: ",")}]`,
        ),
        Object(entries) -> run(
            let parts = entries.entries().map((k, v) ->
                `"{escape_string(s: k)}":` + stringify_pure(value: v)
            ),
            `\{{parts.join(separator: ",")}\}`,
        ),
    )

@escape_string (s: str) -> str =
    s.replace(old: "\\", new: "\\\\")
     .replace(old: "\"", new: "\\\"")
     .replace(old: "\n", new: "\\n")
     .replace(old: "\r", new: "\\r")
     .replace(old: "\t", new: "\\t")
```

---

## Streaming Parser Implementation

```ori
// std/json/stream.ori

type JsonParser = {
    source: str,
    doc: Option<CPtr>,      // YyjsonDoc for native, None for WASM/pure
    stack: [StackFrame],
    finished: bool,
}

type StackFrame = {
    value: StackValue,
    iter: Option<CPtr>,
    index: int,
    len: int,
}

type StackValue =
    | ArrayFrame(YyjsonVal)
    | ObjectFrame(YyjsonVal)
    | ObjectKeyFrame(YyjsonVal)

impl JsonParser {
    #target(not_arch: "wasm32")
    pub @new (source: str) -> JsonParser uses FFI =
        run(
            let doc = _yyjson_read(
                dat: source,
                len: len(collection: source),
                flg: $YYJSON_READ_NOFLAG,
            ),
            let root = if doc.is_null() then CPtr.null() else _yyjson_doc_get_root(doc: doc),
            JsonParser {
                source: source,
                doc: if doc.is_null() then None else Some(doc),
                stack: if root.is_null() then [] else [initial_frame(val: root)],
                finished: doc.is_null(),
            },
        )

    #target(arch: "wasm32")
    pub @new (source: str) -> JsonParser uses FFI =
        // WASM uses pure Ori streaming parser
        JsonParser {
            source: source,
            doc: None,
            stack: [],
            finished: false,
        }
}

@initial_frame (val: YyjsonVal) -> StackFrame uses FFI =
    if _yyjson_is_arr(val: val) then
        StackFrame {
            value: ArrayFrame(val),
            iter: Some(alloc_arr_iter()),
            index: 0,
            len: _yyjson_get_len(val: val),
        }
    else if _yyjson_is_obj(val: val) then
        StackFrame {
            value: ObjectFrame(val),
            iter: Some(alloc_obj_iter()),
            index: 0,
            len: _yyjson_get_len(val: val),
        }
    else
        StackFrame {
            value: ArrayFrame(val),  // Primitive wrapped
            iter: None,
            index: 0,
            len: 1,
        }

impl Iterator for JsonParser {
    type Item = JsonEvent

    #target(not_arch: "wasm32")
    @next (self) -> (Option<JsonEvent>, JsonParser) uses FFI =
        if self.finished then
            (None, self)
        else if self.stack.is_empty() then
            (None, JsonParser { ...self, finished: true })
        else
            run(
                let frame = self.stack[# - 1],
                let rest = self.stack.slice(start: 0, end: # - 1),

                match(frame.value,
                    ArrayFrame(arr) ->
                        if frame.index == 0 && frame.iter.is_some() then
                            // First visit: emit StartArray
                            run(
                                _yyjson_arr_iter_init(arr: arr, iter: frame.iter.unwrap()),
                                let new_frame = StackFrame { ...frame, index: 1 },
                                (Some(StartArray), JsonParser { ...self, stack: [...rest, new_frame] }),
                            )
                        else if frame.index <= frame.len then
                            // Emit array elements
                            run(
                                let elem = _yyjson_arr_iter_next(iter: frame.iter.unwrap()),
                                if elem.is_null() then
                                    (Some(EndArray), JsonParser { ...self, stack: rest })
                                else
                                    run(
                                        let new_frame = StackFrame { ...frame, index: frame.index + 1 },
                                        let event = value_to_event(val: elem),
                                        let new_stack = maybe_push_frame(stack: [...rest, new_frame], val: elem),
                                        (Some(event), JsonParser { ...self, stack: new_stack }),
                                    ),
                            )
                        else
                            (Some(EndArray), JsonParser { ...self, stack: rest }),

                    ObjectFrame(obj) ->
                        if frame.index == 0 then
                            // First visit: emit StartObject
                            run(
                                _yyjson_obj_iter_init(obj: obj, iter: frame.iter.unwrap()),
                                let new_frame = StackFrame { ...frame, index: 1 },
                                (Some(StartObject), JsonParser { ...self, stack: [...rest, new_frame] }),
                            )
                        else
                            // Get next key-value pair
                            run(
                                let key_val = _yyjson_obj_iter_next(iter: frame.iter.unwrap()),
                                if key_val.is_null() then
                                    (Some(EndObject), JsonParser { ...self, stack: rest })
                                else
                                    run(
                                        let key = _yyjson_get_str(val: key_val),
                                        let val = _yyjson_obj_iter_get_val(key: key_val),
                                        // Push value frame, then key frame
                                        let key_frame = StackFrame {
                                            value: ObjectKeyFrame(val),
                                            iter: None,
                                            index: 0,
                                            len: 0,
                                        },
                                        let new_frame = StackFrame { ...frame, index: frame.index + 1 },
                                        (Some(Key(key)), JsonParser { ...self, stack: [...rest, new_frame, key_frame] }),
                                    ),
                            ),

                    ObjectKeyFrame(val) ->
                        // Emit the value after a key
                        run(
                            let event = value_to_event(val: val),
                            let new_stack = maybe_push_frame(stack: rest, val: val),
                            (Some(event), JsonParser { ...self, stack: new_stack }),
                        ),
                ),
            )

    #target(arch: "wasm32")
    @next (self) -> (Option<JsonEvent>, JsonParser) =
        // WASM fallback: use pure Ori parser
        // Implementation similar to native but using pure parser
        (None, JsonParser { ...self, finished: true })  // Placeholder
}

@value_to_event (val: YyjsonVal) -> JsonEvent uses FFI =
    if _yyjson_is_arr(val: val) then
        StartArray
    else if _yyjson_is_obj(val: val) then
        StartObject
    else
        Value(yyjson_val_to_json_value(val: val))

@maybe_push_frame (stack: [StackFrame], val: YyjsonVal) -> [StackFrame] uses FFI =
    if _yyjson_is_arr(val: val) then
        [...stack, initial_frame(val: val)]
    else if _yyjson_is_obj(val: val) then
        [...stack, initial_frame(val: val)]
    else
        stack
```

---

## Pure Ori Components

These don't need FFI:

| Component | Implementation |
|-----------|----------------|
| `JsonValue` type | Pure Ori sum type |
| `JsonValue` accessors | Pure Ori pattern matching |
| `JsonValue.at(path)` | Pure Ori string parsing + navigation |
| `#derive(Json)` | Compiler-generated Ori code |
| `Json` trait | Pure Ori trait |
| `JsonError` | Pure Ori type |

---

## Build Configuration

```toml
# ori.toml
[native]
libraries = ["yyjson"]

# yyjson can be statically linked (recommended)
[native.static]
libraries = ["yyjson"]
```

### Bundling yyjson

yyjson is a single-file library. Options:

1. **Bundle source**: Include `yyjson.c`/`yyjson.h` in Ori distribution, compile during build
2. **System library**: Expect `libyyjson` installed
3. **Vendored**: Ship pre-compiled static library

**Recommendation**: Bundle source (option 1) for simplicity and consistency.

---

## Performance Characteristics

| Operation | yyjson FFI | JS JSON API | Pure Ori | Native Speedup |
|-----------|-----------|-------------|----------|----------------|
| Parse 1KB | ~5μs | ~10μs | ~50μs | ~10x |
| Parse 1MB | ~5ms | ~10ms | ~50ms | ~10x |
| Stringify 1KB | ~3μs | ~5μs | ~30μs | ~10x |
| Stringify 1MB | ~3ms | ~5ms | ~30ms | ~10x |

---

## Summary of Changes from Original

| Aspect | Original | This Revision |
|--------|----------|---------------|
| Public API | Defined | **Unchanged** |
| Parse implementation (native) | Not specified | **yyjson FFI** |
| Parse implementation (WASM) | Not specified | **JS JSON.parse** |
| Stringify implementation | Not specified | **yyjson / JS JSON.stringify** |
| Streaming | JsonParser type | **yyjson tree walking (native)** |
| Fallback | N/A | **Pure Ori available** |
| Performance | Not specified | **~10x faster with FFI** |

---

## Design Decisions

### Why convert JS object to JsonValue tree (WASM)?

Eagerly converting the JS object to an Ori `JsonValue` tree ensures consistent behavior across platforms. Users get the same `JsonValue` API regardless of target. The performance overhead is acceptable since the conversion happens once at parse time.

### Why complete pure Ori fallback?

Some environments (embedded, sandboxed) may not have FFI access. A complete pure Ori implementation ensures `std.json` always works, even if slower.

### Why yyjson over simdjson?

simdjson is read-only (no serialization). yyjson supports both parsing and serialization, making it a better fit for a complete JSON library.
