use super::call_arguments::{create_list_from_array_like, MAX_MATERIALIZED_CALL_ARGUMENTS};
use super::*;

// =========================================================================
// JSON
// =========================================================================
pub(crate) fn json_stringify(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let v = args.first().unwrap_or(&Value::Undefined).clone();
    let replacer = args.get(1).cloned().unwrap_or(Value::Undefined);
    let space_arg = args.get(2).cloned().unwrap_or(Value::Undefined);

    let whitelist = if crate::builtins::is_array_or_throw(vm, &replacer)? {
        Some(json_stringify_property_list(vm, &replacer)?)
    } else {
        None
    };
    let replacer_fn = (whitelist.is_none() && crate::builtins::is_callable(&replacer, &vm.heap))
        .then_some(replacer.clone());
    let gap = json_stringify_gap(vm, space_arg)?;

    let mut ctx = StringifyCtx {
        gap,
        whitelist,
        replacer_fn,
        stack: Vec::new(),
    };
    let wrapper = Value::Object(vm.new_object()?);
    vm.define_data_property(&wrapper, PropertyKey::from(""), v)?;
    match serialize_json_property(vm, &wrapper, PropertyKey::from(""), "", &mut ctx, 0)? {
        Some(s) => Ok(Value::String(Arc::from(s.as_str()))),
        None => Ok(Value::Undefined),
    }
}

fn raw_json_text(vm: &Vm, value: &Value) -> Option<Arc<str>> {
    let Value::Object(index) = value else {
        return None;
    };
    vm.heap.with_obj(index.0, |object| {
        let HeapObj::Object(data) = object else {
            return None;
        };
        if data.class_name.as_deref() != Some("RawJSON") {
            return None;
        }
        data.props
            .lock()
            .get(&PropertyKey::from("rawJSON"))
            .and_then(|descriptor| match &descriptor.value {
                Value::String(text) => Some(text.clone()),
                _ => None,
            })
    })
}

fn normalize_raw_json_for_validation(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }
        let run_start = index;
        while bytes.get(index) == Some(&b'\\') {
            index += 1;
        }
        if (index - run_start) % 2 == 1
            && bytes.get(index) == Some(&b'u')
            && index + 4 < bytes.len()
        {
            let digits = &bytes[index + 1..index + 5];
            if digits.iter().all(|byte| byte.is_ascii_hexdigit()) {
                let value = digits.iter().fold(0u16, |value, byte| {
                    value * 16
                        + match byte {
                            b'0'..=b'9' => (byte - b'0') as u16,
                            b'a'..=b'f' => (byte - b'a' + 10) as u16,
                            b'A'..=b'F' => (byte - b'A' + 10) as u16,
                            _ => unreachable!(),
                        }
                });
                if (0xd800..=0xdfff).contains(&value) {
                    bytes[index + 1..index + 5].copy_from_slice(b"0000");
                }
            }
        }
    }
    String::from_utf8(bytes).expect("raw JSON validation preserves UTF-8")
}

fn json_raw_json(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let text = vm.to_string_pub(args.first().unwrap_or(&Value::Undefined))?;
    if text.is_empty()
        || text
            .as_bytes()
            .first()
            .is_some_and(|byte| matches!(byte, b'\t' | b'\n' | b'\r' | b' '))
        || text
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b'\t' | b'\n' | b'\r' | b' '))
    {
        return Err(Error::syntax("Invalid raw JSON text"));
    }
    let validation_text = normalize_raw_json_for_validation(&text);
    let parsed: serde_json::Value = serde_json::from_str(&validation_text)
        .map_err(|error| Error::syntax(format!("Invalid raw JSON text: {error}")))?;
    if matches!(
        parsed,
        serde_json::Value::Array(_) | serde_json::Value::Object(_)
    ) {
        return Err(Error::syntax(
            "Raw JSON text must contain a primitive value",
        ));
    }

    let mut descriptor = PropertyDescriptor::data(Value::String(Arc::from(text)));
    descriptor.writable = false;
    descriptor.enumerable = true;
    descriptor.configurable = false;
    let mut props = IndexMap::new();
    props.insert(PropertyKey::from("rawJSON"), descriptor);
    let object = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(None),
        extensible: AtomicBool::new(false),
        class_name: Some(Arc::from("RawJSON")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(object)?)))
}

fn json_is_raw_json(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    Ok(Value::Bool(
        args.first()
            .is_some_and(|value| raw_json_text(vm, value).is_some()),
    ))
}

fn json_stringify_gap(vm: &mut Vm, mut space: Value) -> error::Result<String> {
    if let Value::Object(index) = &space {
        let primitive = vm.heap.with_obj(index.0, |object| match object {
            HeapObj::Object(data) => data.primitive.lock().clone(),
            _ => None,
        });
        space = match primitive {
            Some(Value::Number(_)) => Value::Number(vm.to_number(&space)?),
            Some(Value::String(_)) => Value::String(Arc::from(vm.to_string_pub(&space)?)),
            _ => space,
        };
    }
    Ok(match space {
        Value::Number(number) => " ".repeat(number.trunc().clamp(0.0, 10.0) as usize),
        Value::String(string) => {
            crate::value::utf16_slice(&string, 0, crate::value::utf16_len(&string).min(10))
        }
        _ => String::new(),
    })
}

fn json_stringify_property_list(vm: &mut Vm, replacer: &Value) -> error::Result<Vec<String>> {
    let length_value = vm.get_property(replacer, "length")?;
    let length = vm.to_number(&length_value)?;
    let length = if length.is_nan() || length <= 0.0 {
        0
    } else {
        length.trunc().min(9_007_199_254_740_991.0) as usize
    };
    let mut property_list = Vec::new();
    for index in 0..length {
        let item = vm.get_property(replacer, &index.to_string())?;
        let name = match &item {
            Value::String(string) => Some(string.to_string()),
            Value::Number(number) => Some(crate::value::num_to_string(*number)),
            Value::Object(index) => {
                let primitive = vm.heap.with_obj(index.0, |object| match object {
                    HeapObj::Object(data) => data.primitive.lock().clone(),
                    _ => None,
                });
                match primitive {
                    Some(Value::String(_) | Value::Number(_)) => {
                        Some(vm.to_string_pub(&item)?.to_string())
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(name) = name {
            if !property_list.contains(&name) {
                property_list.push(name);
            }
        }
    }
    Ok(property_list)
}

struct StringifyCtx {
    gap: String,
    whitelist: Option<Vec<String>>,
    replacer_fn: Option<Value>,
    stack: Vec<usize>,
}

fn serialize_json_property(
    vm: &mut Vm,
    holder: &Value,
    key: PropertyKey,
    indent: &str,
    ctx: &mut StringifyCtx,
    depth: usize,
) -> error::Result<Option<String>> {
    // Guard against deeply-nested user values overflowing the native stack.
    const MAX_STRINGIFY_DEPTH: usize = 256;
    if depth > MAX_STRINGIFY_DEPTH {
        return Ok(None);
    }
    let key_value = match &key {
        PropertyKey::Str(key) => Value::String(key.clone()),
        PropertyKey::Symbol(symbol) => Value::Symbol(*symbol),
    };
    let mut value = vm.get_property_by_key(holder, &key)?;
    let value_pin = vm.pin(&value);
    let result = (|| {
        if matches!(value, Value::Object(_) | Value::BigInt(_)) {
            let to_json = vm.get_property(&value, "toJSON")?;
            if crate::builtins::is_callable(&to_json, &vm.heap) {
                value =
                    vm.call_function(&to_json, std::slice::from_ref(&key_value), Some(value))?;
            }
        }
        if let Some(replacer) = &ctx.replacer_fn {
            value = vm.call_function(replacer, &[key_value, value], Some(holder.clone()))?;
        }
        value = unbox_json_primitive(vm, value)?;
        let transformed_pin = vm.pin(&value);
        let result = serialize_json_value(vm, value, indent, ctx, depth);
        vm.unpin(transformed_pin);
        result
    })();
    vm.unpin(value_pin);
    result
}

fn unbox_json_primitive(vm: &mut Vm, value: Value) -> error::Result<Value> {
    let Value::Object(index) = &value else {
        return Ok(value);
    };
    let primitive = vm.heap.with_obj(index.0, |object| match object {
        HeapObj::Object(data) => data.primitive.lock().clone(),
        _ => None,
    });
    match primitive {
        Some(Value::Number(_)) => Ok(Value::Number(vm.to_number(&value)?)),
        Some(Value::String(_)) => Ok(Value::String(Arc::from(vm.to_string_pub(&value)?))),
        Some(Value::Bool(value)) => Ok(Value::Bool(value)),
        Some(Value::BigInt(value)) => Ok(Value::BigInt(value)),
        _ => Ok(value),
    }
}

fn quote_json_string(value: &str) -> String {
    let units = crate::value::utf16_from_str(value);
    let mut output = String::from("\"");
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        match unit {
            0x08 => output.push_str("\\b"),
            0x09 => output.push_str("\\t"),
            0x0a => output.push_str("\\n"),
            0x0c => output.push_str("\\f"),
            0x0d => output.push_str("\\r"),
            0x22 => output.push_str("\\\""),
            0x5c => output.push_str("\\\\"),
            0x00..=0x1f => output.push_str(&format!("\\u{unit:04x}")),
            0xd800..=0xdbff
                if units
                    .get(index + 1)
                    .is_some_and(|low| (0xdc00..=0xdfff).contains(low)) =>
            {
                output.push_str(&crate::value::utf16_to_string(&units[index..=index + 1]));
                index += 1;
            }
            0xd800..=0xdfff => output.push_str(&format!("\\u{unit:04x}")),
            _ => output.push_str(&crate::value::utf16_to_string(&[unit])),
        }
        index += 1;
    }
    output.push('"');
    output
}

fn serialize_json_value(
    vm: &mut Vm,
    value: Value,
    indent: &str,
    ctx: &mut StringifyCtx,
    depth: usize,
) -> error::Result<Option<String>> {
    if let Some(text) = raw_json_text(vm, &value) {
        return Ok(Some(text.to_string()));
    }
    match value {
        Value::Undefined => Ok(None),
        Value::Null => Ok(Some("null".into())),
        Value::Bool(value) => Ok(Some(value.to_string())),
        Value::Number(value) => Ok(Some(if value.is_nan() || value.is_infinite() {
            "null".to_string()
        } else {
            crate::value::num_to_string(value)
        })),
        Value::BigInt(_) => Err(Error::type_err("Do not know how to serialize a BigInt")),
        Value::String(value) => Ok(Some(quote_json_string(&value))),
        Value::Symbol(_) | Value::PrivateName(_) | Value::Reference(_) => Ok(None),
        Value::Object(idx) => {
            let value = Value::Object(idx);
            if crate::builtins::is_callable(&value, &vm.heap) {
                return Ok(None);
            }
            if ctx.stack.contains(&idx.0) {
                return Err(Error::type_err("Converting circular structure to JSON"));
            }
            ctx.stack.push(idx.0);
            let child_indent = if ctx.gap.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, ctx.gap)
            };
            let serialized = if crate::builtins::is_array_or_throw(vm, &value)? {
                let length_value = vm.get_property(&value, "length")?;
                let length = vm.to_number(&length_value)?;
                let length = if length.is_nan() || length <= 0.0 {
                    0
                } else {
                    length.trunc().min(9_007_199_254_740_991.0) as usize
                };
                let mut parts = Vec::with_capacity(length);
                for index in 0..length {
                    let part = serialize_json_property(
                        vm,
                        &value,
                        PropertyKey::from(index.to_string()),
                        &child_indent,
                        ctx,
                        depth + 1,
                    )?
                    .unwrap_or_else(|| "null".to_string());
                    parts.push(if ctx.gap.is_empty() {
                        part
                    } else {
                        format!("{}{}", child_indent, part)
                    });
                }
                if parts.is_empty() {
                    "[]".into()
                } else if ctx.gap.is_empty() {
                    format!("[{}]", parts.join(","))
                } else {
                    format!("[\n{}\n{}]", parts.join(",\n"), indent)
                }
            } else {
                let keys = if let Some(whitelist) = &ctx.whitelist {
                    whitelist
                        .iter()
                        .map(|key| PropertyKey::from(key.as_str()))
                        .collect::<Vec<_>>()
                } else {
                    own_property_keys_or_throw(vm, &value, true, true, false)?
                };
                let mut pairs = Vec::new();
                for key in keys {
                    let PropertyKey::Str(key_string) = &key else {
                        continue;
                    };
                    if let Some(serialized) = serialize_json_property(
                        vm,
                        &value,
                        key.clone(),
                        &child_indent,
                        ctx,
                        depth + 1,
                    )? {
                        if ctx.gap.is_empty() {
                            pairs.push(format!("{}:{}", quote_json_string(key_string), serialized));
                        } else {
                            pairs.push(format!(
                                "{}{}: {}",
                                child_indent,
                                quote_json_string(key_string),
                                serialized
                            ));
                        }
                    }
                }
                if pairs.is_empty() {
                    "{}".into()
                } else if ctx.gap.is_empty() {
                    format!("{{{}}}", pairs.join(","))
                } else {
                    format!("{{\n{}\n{}}}", pairs.join(",\n"), indent)
                }
            };
            ctx.stack.pop();
            Ok(Some(serialized))
        }
    }
}

pub(crate) fn json_parse(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let input = args.first().cloned().unwrap_or(Value::Undefined);
    let s = vm.to_string_pub(&input)?;
    let reviver = args.get(1).cloned();
    let is_reviver_fn = reviver
        .as_ref()
        .is_some_and(|reviver| crate::builtins::is_callable(reviver, &vm.heap));
    if is_reviver_fn {
        if let Some(rf) = reviver {
            let (parsed, mut source_node) = parse_json_with_source(vm, &s)?;
            let mut source_pins = Vec::new();
            let attached =
                attach_json_source_values(vm, &parsed, &mut source_node, &mut source_pins);
            let result = attached.and_then(|()| {
                let root = Value::Object(vm.new_object()?);
                vm.define_data_property(&root, PropertyKey::from(""), parsed)?;
                let pins = vm.pin_many(&[root.clone(), rf.clone()]);
                let result = internalize_json_property(
                    vm,
                    &rf,
                    &root,
                    PropertyKey::from(""),
                    Some(&source_node),
                    0,
                );
                vm.unpin_many(pins);
                result
            });
            for pin in source_pins {
                vm.unpin(pin);
            }
            return result;
        }
    }
    parse_json_text(vm, &s)
}

pub(crate) fn parse_json_text(vm: &mut Vm, source: &str) -> error::Result<Value> {
    serde_json::from_str::<serde_json::Value>(source)
        .map_err(|error| Error::syntax(format!("Invalid JSON: {error}")))?;
    parse_json_value(vm, &mut source.chars().peekable(), 0)
}

#[derive(Debug)]
enum JsonSourceKind {
    Primitive(Arc<str>),
    Array(Vec<JsonSourceNode>),
    Object(Vec<(Arc<str>, JsonSourceNode)>),
}

#[derive(Debug)]
struct JsonSourceNode {
    original: Value,
    kind: JsonSourceKind,
}

struct JsonSourceParser<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> JsonSourceParser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.offset += 1;
        }
    }

    fn scan_string(&mut self) -> error::Result<&'a str> {
        let start = self.offset;
        if self.source.as_bytes().get(self.offset) != Some(&b'"') {
            return Err(Error::syntax("Invalid JSON string"));
        }
        self.offset += 1;
        while let Some(byte) = self.source.as_bytes().get(self.offset) {
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(&self.source[start..self.offset]);
                }
                b'\\' => {
                    self.offset += 2;
                }
                _ => self.offset += 1,
            }
        }
        Err(Error::syntax("Unterminated JSON string"))
    }

    fn parse_node(&mut self) -> error::Result<JsonSourceNode> {
        self.skip_whitespace();
        let start = self.offset;
        let kind = match self.source.as_bytes().get(self.offset) {
            Some(b'[') => {
                self.offset += 1;
                self.skip_whitespace();
                let mut children = Vec::new();
                if self.source.as_bytes().get(self.offset) != Some(&b']') {
                    loop {
                        children.push(self.parse_node()?);
                        self.skip_whitespace();
                        if self.source.as_bytes().get(self.offset) == Some(&b']') {
                            break;
                        }
                        self.offset += 1;
                    }
                }
                self.offset += 1;
                JsonSourceKind::Array(children)
            }
            Some(b'{') => {
                self.offset += 1;
                self.skip_whitespace();
                let mut children = Vec::new();
                if self.source.as_bytes().get(self.offset) != Some(&b'}') {
                    loop {
                        let key_source = self.scan_string()?;
                        let key: String = serde_json::from_str(key_source)
                            .map_err(|error| Error::syntax(format!("Invalid JSON key: {error}")))?;
                        self.skip_whitespace();
                        self.offset += 1;
                        let child = self.parse_node()?;
                        children.push((Arc::from(key), child));
                        self.skip_whitespace();
                        if self.source.as_bytes().get(self.offset) == Some(&b'}') {
                            break;
                        }
                        self.offset += 1;
                        self.skip_whitespace();
                    }
                }
                self.offset += 1;
                JsonSourceKind::Object(children)
            }
            Some(b'"') => {
                self.scan_string()?;
                JsonSourceKind::Primitive(Arc::from(&self.source[start..self.offset]))
            }
            Some(_) => {
                while self.source.as_bytes().get(self.offset).is_some_and(|byte| {
                    !matches!(byte, b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t')
                }) {
                    self.offset += 1;
                }
                JsonSourceKind::Primitive(Arc::from(&self.source[start..self.offset]))
            }
            None => return Err(Error::syntax("Invalid JSON source record")),
        };
        Ok(JsonSourceNode {
            original: Value::Undefined,
            kind,
        })
    }
}

fn parse_json_with_source(vm: &mut Vm, source: &str) -> error::Result<(Value, JsonSourceNode)> {
    let value = parse_json_text(vm, source)?;
    let node = JsonSourceParser::new(source).parse_node()?;
    Ok((value, node))
}

fn attach_json_source_values(
    vm: &mut Vm,
    value: &Value,
    node: &mut JsonSourceNode,
    pins: &mut Vec<usize>,
) -> error::Result<()> {
    node.original = value.clone();
    pins.push(vm.pin(value));
    match &mut node.kind {
        JsonSourceKind::Primitive(_) => {}
        JsonSourceKind::Array(children) => {
            for (index, child) in children.iter_mut().enumerate() {
                let value = vm.get_property(value, &index.to_string())?;
                attach_json_source_values(vm, &value, child, pins)?;
            }
        }
        JsonSourceKind::Object(children) => {
            let mut last = std::collections::HashMap::new();
            for (index, (key, _)) in children.iter().enumerate() {
                last.insert(key.clone(), index);
            }
            for (index, (key, child)) in children.iter_mut().enumerate() {
                if last.get(key).copied() != Some(index) {
                    continue;
                }
                let value = vm.get_property(value, key)?;
                attach_json_source_values(vm, &value, child, pins)?;
            }
        }
    }
    Ok(())
}

fn json_same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

fn json_source_child<'a>(
    node: &'a JsonSourceNode,
    key: &PropertyKey,
) -> Option<&'a JsonSourceNode> {
    match (&node.kind, key) {
        (JsonSourceKind::Array(children), PropertyKey::Str(key)) => key
            .parse::<usize>()
            .ok()
            .and_then(|index| children.get(index)),
        (JsonSourceKind::Object(children), PropertyKey::Str(key)) => children
            .iter()
            .rev()
            .find_map(|(candidate, child)| (candidate == key).then_some(child)),
        _ => None,
    }
}

/// Apply InternalizeJSONProperty in place so reviver mutations stay observable.
fn internalize_json_property(
    vm: &mut Vm,
    reviver: &Value,
    holder: &Value,
    name: PropertyKey,
    source_node: Option<&JsonSourceNode>,
    depth: usize,
) -> error::Result<Value> {
    // The parse step already caps nesting, but guard defensively.
    if depth > 256 {
        return Err(Error::syntax(
            "Maximum JSON nesting depth exceeded".to_string(),
        ));
    }
    let value = vm.get_property_by_key(holder, &name)?;
    let source_node = source_node.filter(|node| json_same_value(&node.original, &value));
    let value_pin = vm.pin(&value);
    let result = (|| {
        if matches!(value, Value::Object(_)) {
            if crate::builtins::is_array_or_throw(vm, &value)? {
                let length = vm.get_property(&value, "length")?;
                let length = vm.to_number(&length)?;
                let length = if length.is_nan() || length <= 0.0 {
                    0
                } else {
                    length.trunc().min(9_007_199_254_740_991.0) as usize
                };
                for index in 0..length {
                    let key = PropertyKey::from(index.to_string());
                    let revived = internalize_json_property(
                        vm,
                        reviver,
                        &value,
                        key.clone(),
                        source_node.and_then(|node| json_source_child(node, &key)),
                        depth + 1,
                    )?;
                    if revived.is_undefined() {
                        vm.delete_property_key(&value, &key)?;
                    } else {
                        vm.create_data_property(&value, key, revived)?;
                    }
                }
            } else {
                let keys = own_property_keys_or_throw(vm, &value, true, true, false)?;
                for key in keys {
                    let revived = internalize_json_property(
                        vm,
                        reviver,
                        &value,
                        key.clone(),
                        source_node.and_then(|node| json_source_child(node, &key)),
                        depth + 1,
                    )?;
                    if revived.is_undefined() {
                        vm.delete_property_key(&value, &key)?;
                    } else {
                        vm.create_data_property(&value, key, revived)?;
                    }
                }
            }
        }
        let key = match &name {
            PropertyKey::Str(key) => Value::String(key.clone()),
            PropertyKey::Symbol(symbol) => Value::Symbol(*symbol),
        };
        let context = Value::Object(vm.new_object()?);
        if !matches!(value, Value::Object(_)) {
            if let Some(JsonSourceNode {
                kind: JsonSourceKind::Primitive(source),
                ..
            }) = source_node
            {
                vm.define_data_property(
                    &context,
                    PropertyKey::from("source"),
                    Value::String(source.clone()),
                )?;
            }
        }
        vm.call_function(
            reviver,
            &[key, value.clone(), context],
            Some(holder.clone()),
        )
    })();
    vm.unpin(value_pin);
    result
}
fn parse_json_value(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    depth: usize,
) -> error::Result<Value> {
    // Guard against pathological nesting that would overflow the native
    // stack: `JSON.parse("[".repeat(100000)+...]")` used to abort the host.
    // Node tolerates deep nesting on its larger stack; we cap recursion and
    // surface a SyntaxError instead of crashing.
    const MAX_JSON_DEPTH: usize = 256;
    if depth > MAX_JSON_DEPTH {
        return Err(Error::syntax(
            "Maximum JSON nesting depth exceeded".to_string(),
        ));
    }
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
    match chars.peek() {
        Some(&'{') => {
            chars.next();
            parse_json_obj(vm, chars, depth)
        }
        Some(&'[') => {
            chars.next();
            parse_json_arr(vm, chars, depth)
        }
        Some(&'"') => {
            chars.next();
            parse_json_str(chars)
        }
        Some('t') => {
            chars.take(4).for_each(|_| {});
            Ok(Value::Bool(true))
        }
        Some('f') => {
            chars.take(5).for_each(|_| {});
            Ok(Value::Bool(false))
        }
        Some('n') => {
            chars.take(4).for_each(|_| {});
            Ok(Value::Null)
        }
        Some(c) if *c == '-' || c.is_ascii_digit() => parse_json_num(chars),
        _ => Err(Error::syntax("Invalid JSON".to_string())),
    }
}
fn parse_json_obj(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    depth: usize,
) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    loop {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'}') {
            chars.next();
            break;
        }
        // consume the opening quote of the key string
        if chars.peek() == Some(&'"') {
            chars.next();
        }
        let key = match parse_json_str(chars)? {
            Value::String(s) => s.to_string(),
            _ => String::new(),
        };
        while chars.peek() != Some(&':') {
            match chars.peek() {
                None => return Err(Error::syntax("Invalid JSON: expected ':'".to_string())),
                Some(&_) => {
                    chars.next();
                }
            }
        }
        chars.next();
        let val = parse_json_value(vm, chars, depth + 1)?;
        // JSON-parsed properties are enumerable (data_prop is non-enumerable for builtins).
        let mut desc = data_prop(val);
        desc.enumerable = true;
        props.insert(PropertyKey::from(key.as_str()), desc);
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'}') {
            chars.next();
            break;
        }
    }
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: None,
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}
fn parse_json_arr(
    vm: &mut Vm,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    depth: usize,
) -> error::Result<Value> {
    let mut items = Vec::new();
    loop {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&']') {
            chars.next();
            break;
        }
        items.push(parse_json_value(vm, chars, depth + 1)?);
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&']') {
            chars.next();
            break;
        }
    }
    let obj = HeapObj::Array(ArrayData::new(items, Some(vm.array_proto.clone())));
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}
fn parse_json_str(chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut s = String::new();
    while let Some(c) = chars.next() {
        if c == '"' {
            break;
        }
        if c == '\\' {
            match chars.next() {
                Some('b') => s.push('\u{0008}'),
                Some('f') => s.push('\u{000c}'),
                Some('n') => s.push('\n'),
                Some('r') => s.push('\r'),
                Some('t') => s.push('\t'),
                Some('"') => s.push('"'),
                Some('/') => s.push('/'),
                Some('\\') => s.push('\\'),
                Some('u') => {
                    let mut value = 0u32;
                    for _ in 0..4 {
                        let digit = chars
                            .next()
                            .and_then(|digit| digit.to_digit(16))
                            .ok_or_else(|| Error::syntax("Invalid JSON Unicode escape"))?;
                        value = value * 16 + digit;
                    }
                    if (0xd800..=0xdbff).contains(&value) {
                        if chars.next() != Some('\\') || chars.next() != Some('u') {
                            return Err(Error::syntax("Unsupported lone surrogate in JSON string"));
                        }
                        let mut low = 0u32;
                        for _ in 0..4 {
                            let digit = chars
                                .next()
                                .and_then(|digit| digit.to_digit(16))
                                .ok_or_else(|| Error::syntax("Invalid JSON Unicode escape"))?;
                            low = low * 16 + digit;
                        }
                        if !(0xdc00..=0xdfff).contains(&low) {
                            return Err(Error::syntax("Unsupported lone surrogate in JSON string"));
                        }
                        value = 0x10000 + ((value - 0xd800) << 10) + (low - 0xdc00);
                    }
                    let decoded = char::from_u32(value).ok_or_else(|| {
                        Error::syntax("Unsupported lone surrogate in JSON string")
                    })?;
                    s.push(decoded);
                }
                Some(_) => return Err(Error::syntax("Invalid JSON escape")),
                None => break,
            }
        } else {
            s.push(c);
        }
    }
    Ok(Value::String(Arc::from(s.as_str())))
}
fn parse_json_num(chars: &mut std::iter::Peekable<std::str::Chars>) -> error::Result<Value> {
    let mut s = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
            s.push(c);
            chars.next();
        } else {
            break;
        }
    }
    Ok(Value::Number(s.parse().unwrap_or(f64::NAN)))
}
fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

const MS_PER_SECOND: f64 = 1_000.0;
const MS_PER_MINUTE: f64 = 60.0 * MS_PER_SECOND;
const MS_PER_HOUR: f64 = 60.0 * MS_PER_MINUTE;
const MS_PER_DAY: f64 = 24.0 * MS_PER_HOUR;
const MAX_TIME_VALUE: f64 = 8.64e15;
const MAKE_DAY_YEAR_LIMIT: f64 = 1_000_000_000.0;
const MAKE_DAY_MONTH_LIMIT: f64 = 12_000_000_000.0;
const MAKE_DAY_DATE_LIMIT: f64 = 1_000_000_000_000.0;
const DATE_WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DATE_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn date_time_clip(ts: f64) -> f64 {
    if ts.is_nan() || ts.is_infinite() || ts.abs() > MAX_TIME_VALUE {
        f64::NAN
    } else if ts == 0.0 {
        0.0
    } else {
        let clipped = ts.trunc();
        if clipped == 0.0 {
            0.0
        } else {
            clipped
        }
    }
}

fn date_days_from_civil(year: i64, month_one_based: i64, day: i64) -> i64 {
    let year = year - i64::from(month_one_based <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month_one_based + if month_one_based > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn date_days_from_civil_i128(year: i128, month_one_based: i128, day: i128) -> Option<i128> {
    let year = year.checked_sub(i128::from(month_one_based <= 2))?;
    let era = if year >= 0 {
        year
    } else {
        year.checked_sub(399)?
    } / 400;
    let yoe = year.checked_sub(era.checked_mul(400)?)?;
    let month = month_one_based.checked_add(if month_one_based > 2 { -3 } else { 9 })?;
    let doy = 153_i128
        .checked_mul(month)?
        .checked_add(2)?
        .checked_div(5)?
        .checked_add(day)?
        .checked_sub(1)?;
    let doe = yoe
        .checked_mul(365)?
        .checked_add(yoe / 4)?
        .checked_sub(yoe / 100)?
        .checked_add(doy)?;
    era.checked_mul(146097)?
        .checked_add(doe)?
        .checked_sub(719468)
}

fn date_civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn date_limited_integer(value: f64, limit: f64) -> Option<i128> {
    let value = value.trunc();
    if value.abs() > limit {
        None
    } else {
        Some(value as i128)
    }
}

fn date_make_day(year: f64, month: f64, date: f64) -> f64 {
    if !year.is_finite() || !month.is_finite() || !date.is_finite() {
        return f64::NAN;
    }
    let Some(year) = date_limited_integer(year, MAKE_DAY_YEAR_LIMIT) else {
        return f64::NAN;
    };
    let Some(month) = date_limited_integer(month, MAKE_DAY_MONTH_LIMIT) else {
        return f64::NAN;
    };
    let Some(date) = date_limited_integer(date, MAKE_DAY_DATE_LIMIT) else {
        return f64::NAN;
    };
    let Some(year) = year.checked_add(month.div_euclid(12)) else {
        return f64::NAN;
    };
    if (year as f64).abs() > MAKE_DAY_YEAR_LIMIT {
        return f64::NAN;
    }
    let month_zero_based = month.rem_euclid(12);
    let Some(day) = date_days_from_civil_i128(year, month_zero_based + 1, 1)
        .and_then(|day| day.checked_add(date.checked_sub(1)?))
    else {
        return f64::NAN;
    };
    day as f64
}

fn date_make_day_with_year_offset(year: f64, month: f64, date: f64) -> f64 {
    let year = if year.is_finite() {
        let int_year = year.trunc();
        if (0.0..=99.0).contains(&int_year) {
            int_year + 1900.0
        } else {
            year
        }
    } else {
        year
    };
    date_make_day(year, month, date)
}

fn date_make_time(hour: f64, min: f64, sec: f64, ms: f64) -> f64 {
    if !hour.is_finite() || !min.is_finite() || !sec.is_finite() || !ms.is_finite() {
        return f64::NAN;
    }
    hour.trunc() * MS_PER_HOUR
        + min.trunc() * MS_PER_MINUTE
        + sec.trunc() * MS_PER_SECOND
        + ms.trunc()
}

fn date_make_date(day: f64, time: f64) -> f64 {
    day * MS_PER_DAY + time
}

fn date_day(ts: f64) -> i64 {
    (ts / MS_PER_DAY).floor() as i64
}

fn date_time_within_day(ts: f64) -> f64 {
    ts.rem_euclid(MS_PER_DAY)
}

fn date_time_components(ts: f64) -> (f64, f64, f64, f64, f64) {
    let day = date_day(ts) as f64;
    let time = date_time_within_day(ts);
    let hour = (time / MS_PER_HOUR).floor();
    let minute = ((time % MS_PER_HOUR) / MS_PER_MINUTE).floor();
    let second = ((time % MS_PER_MINUTE) / MS_PER_SECOND).floor();
    let ms = time % MS_PER_SECOND;
    (day, hour, minute, second, ms)
}

fn date_components(ts: f64) -> (f64, f64, f64, f64, f64) {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let time = date_time_within_day(ts);
    (
        year as f64,
        (month_one_based - 1) as f64,
        date as f64,
        day as f64,
        time,
    )
}

fn date_year_string(year: i64) -> String {
    if year < 0 {
        format!("-{:04}", year.saturating_abs())
    } else {
        format!("{year:04}")
    }
}

fn date_iso_year_string(year: i64) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else if year < 0 {
        format!("-{:06}", year.saturating_abs())
    } else {
        format!("+{year:06}")
    }
}

fn date_time_string(ts: f64) -> String {
    let (_, hour, minute, second, _) = date_time_components(ts);
    format!(
        "{:02}:{:02}:{:02}",
        hour as i64, minute as i64, second as i64
    )
}

fn date_date_string(ts: f64) -> String {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let weekday = (day + 4).rem_euclid(7) as usize;
    let month = (month_one_based - 1) as usize;
    format!(
        "{} {} {:02} {}",
        DATE_WEEKDAYS[weekday],
        DATE_MONTHS[month],
        date,
        date_year_string(year)
    )
}

fn date_utc_string(ts: f64) -> String {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let weekday = (day + 4).rem_euclid(7) as usize;
    let month = (month_one_based - 1) as usize;
    format!(
        "{}, {:02} {} {} {} GMT",
        DATE_WEEKDAYS[weekday],
        date,
        DATE_MONTHS[month],
        date_year_string(year),
        date_time_string(ts)
    )
}

fn date_date_time_string(ts: f64) -> String {
    format!("{} {} GMT+0000", date_date_string(ts), date_time_string(ts))
}

fn date_iso_string(ts: f64) -> String {
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let (_, hour, minute, second, ms) = date_time_components(ts);
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        date_iso_year_string(year),
        month_one_based,
        date,
        hour as i64,
        minute as i64,
        second as i64,
        ms as i64
    )
}

fn date_parse_digits_i64(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

fn date_month_from_name(name: &str) -> Option<i64> {
    DATE_MONTHS
        .iter()
        .position(|month| *month == name)
        .map(|idx| idx as i64)
}

fn date_parse_hms(text: &str) -> Option<(i64, i64, i64)> {
    let mut parts = text.split(':');
    let hour = date_parse_digits_i64(parts.next()?)?;
    let minute = date_parse_digits_i64(parts.next()?)?;
    let second = date_parse_digits_i64(parts.next()?)?;
    if parts.next().is_some()
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }
    Some((hour, minute, second))
}

fn date_make_utc_time(
    year: i64,
    month_zero_based: i64,
    date: i64,
    hour: i64,
    minute: i64,
    second: i64,
    ms: i64,
) -> f64 {
    date_time_clip(date_make_date(
        date_make_day(year as f64, month_zero_based as f64, date as f64),
        date_make_time(hour as f64, minute as f64, second as f64, ms as f64),
    ))
}

fn date_parse_timezone_offset(text: &str) -> Option<i64> {
    if text == "Z" || text.is_empty() {
        return Some(0);
    }
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = &text[1..];
    let (hour, minute) = if rest.len() == 5 && rest.as_bytes().get(2) == Some(&b':') {
        (
            date_parse_digits_i64(&rest[..2])?,
            date_parse_digits_i64(&rest[3..])?,
        )
    } else if rest.len() == 4 {
        (
            date_parse_digits_i64(&rest[..2])?,
            date_parse_digits_i64(&rest[2..])?,
        )
    } else {
        return None;
    };
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return None;
    }
    Some(sign * (hour * 60 + minute))
}

fn date_parse_iso_string(source: &str) -> Option<f64> {
    let s = source.trim();
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    let (sign, year_start, year_len) = match bytes[0] {
        b'+' => (1_i64, 1, 6),
        b'-' => (-1_i64, 1, 6),
        b'0'..=b'9' => (1_i64, 0, 4),
        _ => return None,
    };
    if s.len() < year_start + year_len {
        return None;
    }
    let year_digits = date_parse_digits_i64(&s[year_start..year_start + year_len])?;
    if sign < 0 && year_digits == 0 {
        return None;
    }
    let year = sign * year_digits;
    let mut rest = &s[year_start + year_len..];
    if rest.is_empty() {
        return Some(date_make_utc_time(year, 0, 1, 0, 0, 0, 0));
    }
    if !rest.starts_with('-') || rest.len() < 3 {
        return None;
    }
    let month = date_parse_digits_i64(&rest[1..3])?;
    if !(1..=12).contains(&month) {
        return None;
    }
    rest = &rest[3..];
    if rest.is_empty() {
        return Some(date_make_utc_time(year, month - 1, 1, 0, 0, 0, 0));
    }
    if !rest.starts_with('-') || rest.len() < 3 {
        return None;
    }
    let date = date_parse_digits_i64(&rest[1..3])?;
    if !(1..=31).contains(&date) {
        return None;
    }
    rest = &rest[3..];
    if rest.is_empty() {
        return Some(date_make_utc_time(year, month - 1, date, 0, 0, 0, 0));
    }
    if !rest.starts_with('T') || rest.len() < 6 {
        return None;
    }
    let hour = date_parse_digits_i64(&rest[1..3])?;
    let minute = date_parse_digits_i64(&rest[4..6])?;
    if rest.as_bytes().get(3) != Some(&b':')
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
    {
        return None;
    }
    rest = &rest[6..];
    let mut second = 0;
    let mut ms = 0;
    if rest.starts_with(':') {
        if rest.len() < 3 {
            return None;
        }
        second = date_parse_digits_i64(&rest[1..3])?;
        if !(0..=59).contains(&second) {
            return None;
        }
        rest = &rest[3..];
    }
    if rest.starts_with('.') {
        let digits = rest[1..].bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let raw = &rest[1..1 + digits.min(3)];
        ms = date_parse_digits_i64(raw)? * 10_i64.pow((3 - raw.len()) as u32);
        rest = &rest[1 + digits..];
    }
    let offset_minutes = date_parse_timezone_offset(rest)?;
    let time = date_make_utc_time(year, month - 1, date, hour, minute, second, ms);
    Some(date_time_clip(time - offset_minutes as f64 * MS_PER_MINUTE))
}

fn date_parse_legacy_string(source: &str) -> Option<f64> {
    let parts: Vec<&str> = source.split_whitespace().collect();
    if parts.len() >= 6 && parts[0].ends_with(',') {
        let date = date_parse_digits_i64(parts[1])?;
        let month = date_month_from_name(parts[2])?;
        let year = parts[3].parse::<i64>().ok()?;
        let (hour, minute, second) = date_parse_hms(parts[4])?;
        if parts[5] != "GMT" {
            return None;
        }
        return Some(date_make_utc_time(
            year, month, date, hour, minute, second, 0,
        ));
    }
    if parts.len() >= 6 {
        let month = date_month_from_name(parts[1])?;
        let date = date_parse_digits_i64(parts[2])?;
        let year = parts[3].parse::<i64>().ok()?;
        let (hour, minute, second) = date_parse_hms(parts[4])?;
        let offset = parts[5]
            .strip_prefix("GMT")
            .and_then(date_parse_timezone_offset)?;
        let time = date_make_utc_time(year, month, date, hour, minute, second, 0);
        return Some(date_time_clip(time - offset as f64 * MS_PER_MINUTE));
    }
    None
}

fn date_parse_string(source: &str) -> f64 {
    date_parse_iso_string(source)
        .or_else(|| date_parse_legacy_string(source))
        .unwrap_or(f64::NAN)
}

fn active_native_name(vm: &mut Vm) -> Option<Arc<str>> {
    let callee = vm.current_native_callee().cloned()?;
    let Value::Object(idx) = callee else {
        return None;
    };
    vm.heap.with_obj(idx.0, |o| {
        if let HeapObj::Function(f) = o {
            f.name.clone()
        } else {
            None
        }
    })
}

const DATE_VALUE_SLOT: &str = "[[DateValue]]";

fn date_value_slot_key() -> crate::value::PrivateSlotKey {
    crate::value::PrivateSlotKey::Internal(Arc::from(DATE_VALUE_SLOT))
}

fn new_date_object(vm: &mut Vm, prototype: Value, time_value: f64) -> error::Result<Value> {
    let pin_count = vm.pin(&prototype);
    let result = vm
        .alloc(HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Date")),
            private_fields: Mutex::new(std::collections::HashMap::from([(
                date_value_slot_key(),
                crate::value::PrivateSlot::Value(Value::Number(time_value)),
            )])),
            primitive: Mutex::new(None),
        }))
        .map(Value::Object);
    vm.unpin_many(pin_count);
    result
}

pub(crate) fn date_constructor(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    if vm.current_native_new_target().is_none() {
        return Ok(Value::String(Arc::from(
            date_date_time_string(now_ms()).as_str(),
        )));
    }

    let ts = if args.is_empty() {
        now_ms()
    } else if args.len() == 1 {
        let value = args.first().unwrap_or(&Value::Undefined);
        if let Ok((_, ts)) = date_this_time_value(vm, Some(value.clone())) {
            ts
        } else {
            let value = vm.to_primitive(value)?;
            match value {
                Value::String(source) => date_parse_string(&source),
                _ => vm.to_number(&value)?,
            }
        }
    } else {
        let year = vm.to_number(args.first().unwrap_or(&Value::Undefined))?;
        let month = vm.to_number(args.get(1).unwrap_or(&Value::Undefined))?;
        let date = match args.get(2) {
            Some(value) => vm.to_number(value)?,
            None => 1.0,
        };
        let hour = match args.get(3) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        let minute = match args.get(4) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        let second = match args.get(5) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        let ms = match args.get(6) {
            Some(value) => vm.to_number(value)?,
            None => 0.0,
        };
        date_make_date(
            date_make_day_with_year_offset(year, month, date),
            date_make_time(hour, minute, second, ms),
        )
    };
    // ES TimeValue: values outside +/-8.64e15 ms become Invalid Date (NaN).
    let ts = date_time_clip(ts);
    let fallback = vm.current_realm_date_prototype();
    let prototype = native_constructor_prototype_with_default(vm, "Date", fallback)?;
    new_date_object(vm, prototype, ts)
}

fn date_this_time_value(vm: &Vm, this: Option<Value>) -> error::Result<(GcIdx, f64)> {
    let Some(Value::Object(idx)) = this else {
        return Err(Error::type_err("Date method called on non-Date receiver"));
    };
    let ts = vm
        .heap
        .with_obj(idx.0, |obj| {
            let HeapObj::Object(data) = obj else {
                return None;
            };
            data.private_fields
                .lock()
                .get(&date_value_slot_key())
                .and_then(|slot| match slot {
                    crate::value::PrivateSlot::Value(Value::Number(value)) => Some(*value),
                    _ => None,
                })
        })
        .ok_or_else(|| Error::type_err("Date method called on non-Date receiver"))?;
    Ok((idx, ts))
}

pub(crate) fn date_get_time(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    Ok(Value::Number(ts))
}

pub(crate) fn date_get_component(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let ts = match date_get_time(vm, &[], this)? {
        Value::Number(n) if n.is_finite() => n,
        _ => return Ok(Value::Number(f64::NAN)),
    };
    let name = active_native_name(vm).unwrap_or_else(|| Arc::from(""));
    let day = date_day(ts);
    let (year, month_one_based, date) = date_civil_from_days(day);
    let time = date_time_within_day(ts);
    let value = match name.as_ref() {
        "getFullYear" | "getUTCFullYear" => year as f64,
        "getMonth" | "getUTCMonth" => (month_one_based - 1) as f64,
        "getDate" | "getUTCDate" => date as f64,
        "getDay" | "getUTCDay" => (day + 4).rem_euclid(7) as f64,
        "getHours" | "getUTCHours" => (time / MS_PER_HOUR).floor(),
        "getMinutes" | "getUTCMinutes" => ((time % MS_PER_HOUR) / MS_PER_MINUTE).floor(),
        "getSeconds" | "getUTCSeconds" => ((time % MS_PER_MINUTE) / MS_PER_SECOND).floor(),
        "getMilliseconds" | "getUTCMilliseconds" => time % MS_PER_SECOND,
        _ => f64::NAN,
    };
    Ok(Value::Number(value))
}

pub(crate) fn date_set_component(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (idx, ts) = date_this_time_value(vm, this)?;
    let name = active_native_name(vm).unwrap_or_else(|| Arc::from(""));
    let value = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => f64::NAN,
    };
    let value = match name.as_ref() {
        "setTime" => date_time_clip(value),
        "setMilliseconds" | "setUTCMilliseconds" => {
            date_set_time_component(vm, args, ts, value, 1)?
        }
        "setSeconds" | "setUTCSeconds" => date_set_time_component(vm, args, ts, value, 2)?,
        "setMinutes" | "setUTCMinutes" => date_set_time_component(vm, args, ts, value, 3)?,
        "setHours" | "setUTCHours" => date_set_time_component(vm, args, ts, value, 4)?,
        "setDate" | "setUTCDate" => date_set_date_component(vm, args, ts, value, 1)?,
        "setMonth" | "setUTCMonth" => date_set_date_component(vm, args, ts, value, 2)?,
        "setFullYear" | "setUTCFullYear" => date_set_date_component(vm, args, ts, value, 3)?,
        _ => value,
    };
    if value.is_nan()
        && !matches!(name.as_ref(), "setTime" | "setFullYear" | "setUTCFullYear")
        && ts.is_nan()
    {
        return Ok(Value::Number(f64::NAN));
    }
    vm.heap.with_obj(idx.0, |o| {
        if let HeapObj::Object(o) = o {
            o.private_fields.lock().insert(
                date_value_slot_key(),
                crate::value::PrivateSlot::Value(Value::Number(value)),
            );
        }
    });
    Ok(Value::Number(value))
}

fn date_set_time_component(
    vm: &mut Vm,
    args: &[Value],
    ts: f64,
    first: f64,
    arity: usize,
) -> error::Result<f64> {
    let (day, old_hour, old_minute, old_second, old_ms) = date_time_components(ts);
    let (hour, minute, second, ms) = match arity {
        1 => (old_hour, old_minute, old_second, first),
        2 => {
            let ms = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_ms,
            };
            (old_hour, old_minute, first, ms)
        }
        3 => {
            let second = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_second,
            };
            let ms = match args.get(2) {
                Some(v) => vm.to_number(v)?,
                None => old_ms,
            };
            (old_hour, first, second, ms)
        }
        4 => {
            let minute = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_minute,
            };
            let second = match args.get(2) {
                Some(v) => vm.to_number(v)?,
                None => old_second,
            };
            let ms = match args.get(3) {
                Some(v) => vm.to_number(v)?,
                None => old_ms,
            };
            (first, minute, second, ms)
        }
        _ => unreachable!(),
    };
    if ts.is_nan() {
        return Ok(f64::NAN);
    }
    Ok(date_time_clip(date_make_date(
        day,
        date_make_time(hour, minute, second, ms),
    )))
}

fn date_set_date_component(
    vm: &mut Vm,
    args: &[Value],
    ts: f64,
    first: f64,
    arity: usize,
) -> error::Result<f64> {
    let base = if arity == 3 && ts.is_nan() { 0.0 } else { ts };
    let (old_year, old_month, old_date, _, time) = date_components(base);
    let (year, month, date) = match arity {
        1 => (old_year, old_month, first),
        2 => {
            let date = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_date,
            };
            (old_year, first, date)
        }
        3 => {
            let month = match args.get(1) {
                Some(v) => vm.to_number(v)?,
                None => old_month,
            };
            let date = match args.get(2) {
                Some(v) => vm.to_number(v)?,
                None => old_date,
            };
            (first, month, date)
        }
        _ => unreachable!(),
    };
    if ts.is_nan() && arity != 3 {
        return Ok(f64::NAN);
    }
    Ok(date_time_clip(date_make_date(
        date_make_day(year, month, date),
        time,
    )))
}

pub(crate) fn date_to_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if ts.is_nan() {
        return Ok(Value::String(Arc::from("Invalid Date")));
    }
    let name = active_native_name(vm).unwrap_or_else(|| Arc::from(""));
    let result = match name.as_ref() {
        "toDateString" | "toLocaleDateString" => date_date_string(ts),
        "toTimeString" | "toLocaleTimeString" => {
            format!("{} GMT+0000", date_time_string(ts))
        }
        "toUTCString" => date_utc_string(ts),
        "toString" | "toLocaleString" => date_date_time_string(ts),
        _ => date_date_time_string(ts),
    };
    Ok(Value::String(Arc::from(result.as_str())))
}

pub(crate) fn date_to_primitive(
    vm: &mut Vm,
    args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let object = this.unwrap_or(Value::Undefined);
    if !matches!(object, Value::Object(_)) {
        return Err(Error::type_err(
            "Date.prototype[Symbol.toPrimitive] requires an object receiver",
        ));
    }
    let methods = match args.first().unwrap_or(&Value::Undefined) {
        Value::String(hint) if hint.as_ref() == "default" || hint.as_ref() == "string" => {
            ["toString", "valueOf"]
        }
        Value::String(hint) if hint.as_ref() == "number" => ["valueOf", "toString"],
        _ => return Err(Error::type_err("Invalid Date toPrimitive hint")),
    };
    let object_pin = vm.pin(&object);
    let result = (|| {
        for name in methods {
            let method = vm.get_property(&object, name)?;
            if crate::builtins::is_callable(&method, &vm.heap) {
                let value = vm.call_function(&method, &[], Some(object.clone()))?;
                if !matches!(value, Value::Object(_)) {
                    return Ok(value);
                }
            }
        }
        Err(Error::type_err("Cannot convert object to primitive value"))
    })();
    vm.unpin(object_pin);
    result
}

pub(crate) fn date_to_iso_string(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if !ts.is_finite() {
        return Err(Error::range("Invalid time value"));
    }
    Ok(Value::String(Arc::from(date_iso_string(ts).as_str())))
}

pub(crate) fn date_to_json(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let value = this.unwrap_or(Value::Undefined);
    if value.is_nullish() {
        return Err(Error::type_err(
            "Cannot convert undefined or null to object",
        ));
    }
    let object = vm.to_object(&value)?;
    let primitive = vm.to_primitive_number(&object)?;
    if let Value::Number(n) = primitive {
        if !n.is_finite() {
            return Ok(Value::Null);
        }
    }
    let to_iso = vm.get_property(&object, "toISOString")?;
    if !is_callable(&to_iso, &vm.heap) {
        return Err(Error::type_err("toISOString is not callable"));
    }
    vm.call_function(&to_iso, &[], Some(object))
}

pub(crate) fn date_to_temporal_instant(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if !ts.is_finite() {
        return Err(Error::range("Invalid time value"));
    }

    let epoch_nanoseconds = BigInt::from(ts as i64) * BigInt::from(1_000_000_i64);
    let mut props = IndexMap::new();
    props.insert(
        PropertyKey::from("epochNanoseconds"),
        data_prop(Value::BigInt(epoch_nanoseconds)),
    );
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("Temporal.Instant")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(vm.alloc(obj)?))
}

pub(crate) fn date_get_timezone_offset(
    vm: &mut Vm,
    _args: &[Value],
    this: Option<Value>,
) -> error::Result<Value> {
    let (_, ts) = date_this_time_value(vm, this)?;
    if ts.is_nan() {
        return Ok(Value::Number(f64::NAN));
    }
    Ok(Value::Number(0.0))
}

pub(crate) fn date_now(
    _vm: &mut Vm,
    _args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    Ok(Value::Number(now_ms()))
}

pub(crate) fn date_parse(
    vm: &mut Vm,
    args: &[Value],
    _this: Option<Value>,
) -> error::Result<Value> {
    let source = match args.first() {
        Some(v) => vm.to_string(v)?,
        None => return Ok(Value::Number(f64::NAN)),
    };
    Ok(Value::Number(date_parse_string(&source)))
}

pub(crate) fn date_utc(vm: &mut Vm, args: &[Value], _this: Option<Value>) -> error::Result<Value> {
    let year = match args.first() {
        Some(v) => vm.to_number(v)?,
        None => f64::NAN,
    };
    let month = match args.get(1) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let date = match args.get(2) {
        Some(v) => vm.to_number(v)?,
        None => 1.0,
    };
    let hour = match args.get(3) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let minute = match args.get(4) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let second = match args.get(5) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let ms = match args.get(6) {
        Some(v) => vm.to_number(v)?,
        None => 0.0,
    };
    let time = date_make_date(
        date_make_day_with_year_offset(year, month, date),
        date_make_time(hour, minute, second, ms),
    );
    Ok(Value::Number(date_time_clip(time)))
}

fn reflect_property_key(vm: &mut Vm, args: &[Value]) -> error::Result<Value> {
    vm.to_property_key_value(args.get(1).unwrap_or(&Value::Undefined))
}

pub(crate) fn reflect_get(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.get target must be an object"));
    }
    let key = reflect_property_key(vm, args)?;
    let receiver = args.get(2).cloned().unwrap_or_else(|| target.clone());
    match &key {
        Value::String(s) => vm.get_property_rx(&target, s, receiver),
        Value::Symbol(id) => {
            vm.get_property_key_rx(&target, &crate::value::PropertyKey::Symbol(*id), receiver)
        }
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    }
}
pub(crate) fn reflect_set(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.set target must be an object"));
    }
    let key = reflect_property_key(vm, args)?;
    let value = args.get(2).cloned().unwrap_or(Value::Undefined);
    let receiver = args.get(3).cloned().unwrap_or_else(|| target.clone());
    let result = match &key {
        Value::String(s) => vm.try_set_property_with_receiver(&target, s, value, &receiver),
        Value::Symbol(id) => vm.try_set_property_key_with_receiver(
            &target,
            &PropertyKey::Symbol(*id),
            value,
            &receiver,
        ),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    result.map(Value::Bool)
}
pub(crate) fn reflect_has(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.has target must be an object"));
    }
    let key = reflect_property_key(vm, args)?;
    let pkey = match key {
        Value::String(s) => PropertyKey::from(s.as_ref()),
        Value::Symbol(id) => PropertyKey::Symbol(id),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    let has = vm.has_property_key(&target, &pkey)?;
    Ok(Value::Bool(has))
}
pub(crate) fn reflect_delete_property(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.deleteProperty target must be an object",
        ));
    }
    let key = reflect_property_key(vm, args)?;
    let pkey = match key {
        Value::String(s) => PropertyKey::from_rc(s),
        Value::Symbol(id) => PropertyKey::Symbol(id),
        _ => unreachable!("ToPropertyKey returns only String or Symbol"),
    };
    vm.delete_property_key(&target, &pkey).map(Value::Bool)
}
fn reflect_define_property(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.defineProperty target must be an object",
        ));
    }
    object_define_property_result(vm, args, false).map(Value::Bool)
}
fn reflect_get_own_property_descriptor(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.getOwnPropertyDescriptor target must be an object",
        ));
    }
    object_get_own_property_descriptor(vm, args, None)
}
pub(crate) fn reflect_own_keys(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err("Reflect.ownKeys target must be an object"));
    }
    let property_keys = own_property_keys_or_throw(vm, &target, false, true, true)?;
    let mut keys = Vec::new();
    for key in property_keys {
        reserve_own_key_consumer_values(
            vm,
            &mut keys,
            1,
            #[cfg(test)]
            crate::vm::OwnKeyConsumerReservationSite::Result,
        )?;
        keys.push(property_key_to_value(&key));
    }
    make_value_array_in_current_realm(vm, keys)
}
fn reflect_get_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    reflect_get_prototype_of_strict(vm, args)
}
fn reflect_set_prototype_of(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    reflect_set_prototype_of_result(vm, args).map(Value::Bool)
}
fn reflect_is_extensible(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.isExtensible target must be an object",
        ));
    }
    vm.is_extensible(&target).map(Value::Bool)
}
fn reflect_prevent_extensions(
    vm: &mut Vm,
    args: &[Value],
    _: Option<Value>,
) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    if !matches!(target, Value::Object(_)) {
        return Err(Error::type_err(
            "Reflect.preventExtensions target must be an object",
        ));
    }
    vm.prevent_extensions(&target).map(Value::Bool)
}
fn reflect_apply(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);
    let args_arr = args.get(2).cloned().unwrap_or(Value::Undefined);
    if !is_callable(&target, &vm.heap) {
        return Err(Error::type_err("target is not callable"));
    }
    let (call_args, call_args_pin_count) =
        create_list_from_array_like(vm, &args_arr, MAX_MATERIALIZED_CALL_ARGUMENTS)?;
    let result = vm.call_function(&target, &call_args, Some(this_arg));
    vm.unpin_many(call_args_pin_count);
    result
}

fn reflect_construct(vm: &mut Vm, args: &[Value], _: Option<Value>) -> error::Result<Value> {
    let target = args.first().cloned().unwrap_or(Value::Undefined);
    let args_arr = args.get(1).cloned().unwrap_or(Value::Undefined);
    if !vm.is_constructor_value(&target) {
        return Err(Error::type_err("target is not a constructor"));
    }
    let new_target = if let Some(value) = args.get(2) {
        if !vm.is_constructor_value(value) {
            return Err(Error::type_err("newTarget is not a constructor"));
        }
        value.clone()
    } else {
        target.clone()
    };
    let (call_args, call_args_pin_count) =
        create_list_from_array_like(vm, &args_arr, MAX_MATERIALIZED_CALL_ARGUMENTS)?;
    let result = vm.construct_with_new_target(&target, &call_args, &new_target);
    vm.unpin_many(call_args_pin_count);
    result
}

pub(crate) fn build_reflect(vm: &mut Vm) -> error::Result<Value> {
    let env = vm.global;
    let object_proto = vm.object_proto.clone();
    build_reflect_in_env(vm, env, object_proto)
}

pub(crate) fn build_reflect_in_env(
    vm: &mut Vm,
    env: GcIdx,
    object_proto: Value,
) -> error::Result<Value> {
    let mut method_pins = 0;
    let result = (|| {
        let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
        let entries: &[(&str, NativeFn, usize)] = &[
            ("get", reflect_get as NativeFn, 2),
            ("set", reflect_set as NativeFn, 3),
            ("has", reflect_has as NativeFn, 2),
            ("deleteProperty", reflect_delete_property as NativeFn, 2),
            ("defineProperty", reflect_define_property as NativeFn, 3),
            (
                "getOwnPropertyDescriptor",
                reflect_get_own_property_descriptor as NativeFn,
                2,
            ),
            ("ownKeys", reflect_own_keys as NativeFn, 1),
            ("getPrototypeOf", reflect_get_prototype_of as NativeFn, 1),
            ("setPrototypeOf", reflect_set_prototype_of as NativeFn, 2),
            ("isExtensible", reflect_is_extensible as NativeFn, 1),
            (
                "preventExtensions",
                reflect_prevent_extensions as NativeFn,
                1,
            ),
            ("apply", reflect_apply as NativeFn, 3),
            ("construct", reflect_construct as NativeFn, 2),
        ];
        for (name, f, len) in entries {
            let idx = vm.new_native_function_in_env_with_gc_retry(name, *f, *len, env)?;
            let method = Value::Object(idx);
            method_pins += vm.pin(&method);
            props.insert(PropertyKey::from(*name), data_prop(method));
        }
        let mut tag = data_prop(Value::String(Arc::from("Reflect")));
        tag.writable = false;
        tag.enumerable = false;
        tag.configurable = true;
        props.insert(
            PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
            tag,
        );
        let obj = HeapObj::Object(ObjectData {
            props: Mutex::new(props),
            proto: Mutex::new(Some(object_proto)),
            extensible: AtomicBool::new(true),
            class_name: Some(Arc::from("Reflect")),
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        Ok(Value::Object(vm.alloc(obj)?))
    })();
    vm.unpin_many(method_pins);
    result
}

pub(crate) fn build_json(vm: &mut Vm) -> error::Result<Value> {
    let mut props: IndexMap<PropertyKey, PropertyDescriptor> = IndexMap::new();
    let pi = vm.new_native_function("parse", json_parse, 2)?;
    let si = vm.new_native_function("stringify", json_stringify, 3)?;
    let raw = vm.new_native_function("rawJSON", json_raw_json, 1)?;
    let is_raw = vm.new_native_function("isRawJSON", json_is_raw_json, 1)?;
    props.insert(PropertyKey::from("parse"), data_prop(Value::Object(pi)));
    props.insert(PropertyKey::from("stringify"), data_prop(Value::Object(si)));
    props.insert(PropertyKey::from("rawJSON"), data_prop(Value::Object(raw)));
    props.insert(
        PropertyKey::from("isRawJSON"),
        data_prop(Value::Object(is_raw)),
    );
    let mut tag = data_prop(Value::String(Arc::from("JSON")));
    tag.writable = false;
    tag.enumerable = false;
    tag.configurable = true;
    props.insert(
        PropertyKey::Symbol(vm.well_known_symbols.to_string_tag),
        tag,
    );
    let obj = HeapObj::Object(ObjectData {
        props: Mutex::new(props),
        proto: Mutex::new(Some(vm.object_proto.clone())),
        extensible: AtomicBool::new(true),
        class_name: Some(Arc::from("JSON")),
        private_fields: Mutex::new(std::collections::HashMap::new()),
        primitive: Mutex::new(None),
    });
    Ok(Value::Object(GcIdx(vm.heap.allocate(obj)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect_builder_roots_methods_across_cap_triggered_gc() {
        const METHOD_NAMES: [&str; 13] = [
            "get",
            "set",
            "has",
            "deleteProperty",
            "ownKeys",
            "getPrototypeOf",
            "setPrototypeOf",
            "isExtensible",
            "preventExtensions",
            "getOwnPropertyDescriptor",
            "defineProperty",
            "apply",
            "construct",
        ];

        let mut failed_vm = Vm::new().expect("failed to initialize capped VM");
        failed_vm.gc();
        let failed_baseline_live = failed_vm.heap.live_count();
        let failed_baseline_pins = failed_vm.gc_pins.len();
        failed_vm.set_max_heap_objects(Some(failed_baseline_live + METHOD_NAMES.len()));
        let failed_env = failed_vm.global;
        let failed_object_proto = failed_vm.object_proto.clone();
        assert!(
            build_reflect_in_env(&mut failed_vm, failed_env, failed_object_proto).is_err(),
            "Reflect object should not exceed an exact methods-only cap"
        );
        assert_eq!(failed_vm.gc_pins.len(), failed_baseline_pins);
        failed_vm.set_max_heap_objects(None);
        failed_vm.gc();
        assert_eq!(failed_vm.heap.live_count(), failed_baseline_live);

        for garbage_count in [14, 7, 1] {
            let mut vm = Vm::new().expect("failed to initialize VM");
            vm.gc();
            let baseline_live = vm.heap.live_count();
            let baseline_pins = vm.gc_pins.len();

            for _ in 0..garbage_count {
                vm.new_object()
                    .expect("unreachable garbage fixture should allocate");
            }
            vm.set_max_heap_objects(Some(baseline_live + METHOD_NAMES.len() + 1));
            let env = vm.global;
            let object_proto = vm.object_proto.clone();
            let reflect = build_reflect_in_env(&mut vm, env, object_proto)
                .expect("Reflect should fit after collecting the garbage fixture");
            vm.set_max_heap_objects(None);

            let reflect_pin = vm.pin(&reflect);
            vm.gc();
            let mut method_indices = std::collections::HashSet::new();
            for name in METHOD_NAMES {
                let method = vm
                    .get_property(&reflect, name)
                    .unwrap_or_else(|_| panic!("Reflect.{name} should remain readable"));
                let Value::Object(method_idx) = method else {
                    panic!("Reflect.{name} should remain an object");
                };
                vm.heap.with_obj(method_idx.0, |object| {
                    assert!(
                        matches!(object, HeapObj::Function(_)),
                        "Reflect.{name} should remain callable"
                    );
                });
                assert!(
                    method_indices.insert(method_idx.0),
                    "Reflect.{name} should retain a distinct function"
                );
                assert_eq!(
                    vm.get_property(&Value::Object(method_idx), "name")
                        .unwrap_or_else(|_| panic!("Reflect.{name}.name should remain readable")),
                    Value::String(Arc::from(name))
                );
            }
            vm.unpin_many(reflect_pin);
            assert_eq!(vm.gc_pins.len(), baseline_pins);
        }
    }

    #[test]
    fn reflect_argument_list_pins_balance_on_success_and_errors() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        vm.run(
            r#"
            var itemError = {};
            var abruptItems = {
              length: 2,
              get 0() { return {}; },
              get 1() { throw itemError; }
            };
            var lengthError = {};
            var abruptLength = {
              get length() {
                return { valueOf: function() { throw lengthError; } };
              }
            };
            var completeItems = {
              length: 2,
              get 0() { return { label: "first" }; },
              get 1() { return { label: "second" }; }
            };
            var targetError = {};
            function returningCallTarget(first) { return first; }
            function ReturningConstructTarget(first) { this.first = first; }
            function throwingCallTarget() { throw targetError; }
            function ThrowingConstructTarget() { throw targetError; }
            "#,
        )
        .expect("failed to create Reflect argument-list fixtures");

        let baseline = vm.gc_pins.len();
        for name in ["abruptItems", "abruptLength"] {
            let value = vm.run(name).expect("failed to read abrupt fixture");
            assert!(
                create_list_from_array_like(&mut vm, &value, MAX_MATERIALIZED_CALL_ARGUMENTS)
                    .is_err()
            );
            assert_eq!(vm.gc_pins.len(), baseline, "pin leak after {name}");
        }

        let value = vm
            .run("completeItems")
            .expect("failed to read complete fixture");
        let (items, pin_count) =
            create_list_from_array_like(&mut vm, &value, MAX_MATERIALIZED_CALL_ARGUMENTS)
                .expect("complete list materialization should succeed");
        assert_eq!(items.len(), 2);
        assert_eq!(vm.gc_pins.len(), baseline + pin_count);
        vm.gc();
        assert_ne!(items[0], items[1]);
        assert_eq!(
            vm.get_property(&items[0], "label")
                .expect("first item should survive collection"),
            Value::String(Arc::from("first"))
        );
        assert_eq!(
            vm.get_property(&items[1], "label")
                .expect("second item should survive collection"),
            Value::String(Arc::from("second"))
        );
        vm.unpin_many(pin_count);
        assert_eq!(vm.gc_pins.len(), baseline);

        let call_target = vm
            .run("returningCallTarget")
            .expect("failed to read returning call target");
        let call_result = reflect_apply(
            &mut vm,
            &[call_target, Value::Undefined, value.clone()],
            None,
        )
        .expect("successful Reflect.apply should return its target result");
        assert_eq!(vm.gc_pins.len(), baseline);
        assert_eq!(
            vm.get_property(&call_result, "label")
                .expect("returned argument should remain valid"),
            Value::String(Arc::from("first"))
        );

        let construct_target = vm
            .run("ReturningConstructTarget")
            .expect("failed to read returning construct target");
        let construct_result = reflect_construct(&mut vm, &[construct_target, value.clone()], None)
            .expect("successful Reflect.construct should return an instance");
        assert_eq!(vm.gc_pins.len(), baseline);
        let constructed_first = vm
            .get_property(&construct_result, "first")
            .expect("constructed argument property should be readable");
        assert_eq!(
            vm.get_property(&constructed_first, "label")
                .expect("constructed argument should remain valid"),
            Value::String(Arc::from("first"))
        );

        let call_target = vm
            .run("throwingCallTarget")
            .expect("failed to read throwing call target");
        assert!(reflect_apply(
            &mut vm,
            &[call_target, Value::Undefined, value.clone()],
            None,
        )
        .is_err());
        assert_eq!(vm.gc_pins.len(), baseline);

        let construct_target = vm
            .run("ThrowingConstructTarget")
            .expect("failed to read throwing construct target");
        assert!(reflect_construct(&mut vm, &[construct_target, value], None).is_err());
        assert_eq!(vm.gc_pins.len(), baseline);
    }
}
