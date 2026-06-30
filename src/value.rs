//! Value model for the RuJa VM.
//!
//! `Value` is a tagged union. Heap objects live in the GC heap as `HeapObj`
//! and are referenced by `GcIdx`. The GC traces reachable objects from roots
//! and reclaims the rest, including reference cycles.

use crate::ast::FunctionExpr;
use indexmap::IndexMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use std::fmt;
use std::sync::Arc;

/// A property key: either a string (possibly numeric-origin) or a Symbol id.
///
/// Stored in object `props` maps so that Symbol-keyed properties (e.g.
/// `Symbol.iterator`) coexist with ordinary string-keyed ones.
#[derive(Clone, Debug)]
pub enum PropertyKey {
    Str(Arc<str>),
    Symbol(u32),
}

impl PropertyKey {
    pub fn from_string(s: String) -> Self {
        PropertyKey::Str(Arc::from(s.as_str()))
    }
    pub fn from_rc(s: Arc<str>) -> Self {
        PropertyKey::Str(s)
    }

    /// If this key is a string key, return its text; otherwise `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropertyKey::Str(s) => Some(s.as_ref()),
            PropertyKey::Symbol(_) => None,
        }
    }

    pub fn is_symbol(&self) -> bool {
        matches!(self, PropertyKey::Symbol(_))
    }
}

impl From<&str> for PropertyKey {
    fn from(s: &str) -> Self {
        PropertyKey::Str(Arc::from(s))
    }
}
impl From<String> for PropertyKey {
    fn from(s: String) -> Self {
        PropertyKey::Str(Arc::from(s.as_str()))
    }
}
impl From<Arc<str>> for PropertyKey {
    fn from(s: Arc<str>) -> Self {
        PropertyKey::Str(s)
    }
}

impl std::hash::Hash for PropertyKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PropertyKey::Str(s) => {
                0u8.hash(state);
                s.hash(state);
            }
            PropertyKey::Symbol(id) => {
                1u8.hash(state);
                id.hash(state);
            }
        }
    }
}

impl PartialEq for PropertyKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PropertyKey::Str(a), PropertyKey::Str(b)) => a == b,
            (PropertyKey::Symbol(a), PropertyKey::Symbol(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for PropertyKey {}

use num_bigint::BigInt;
use num_traits::Zero;

/// A handle into the GC heap.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GcIdx(pub usize);

/// Maximum number of dense (backing-store) elements an array will hold.
/// Indices at or above this threshold are stored as named properties
/// instead of being materialized as `undefined` holes, which prevents a
/// single assignment like `a[0x80000000]` from forcing the engine to
/// allocate billions of slots (a trivial DoS). ES allows arrays to be
/// sparse, so this is spec-compatible.
pub const MAX_DENSE_ARRAY_LEN: usize = 1 << 20; // 1,048,576

/// ES spec "array index": an integer `i` such that `0 <= i < 2^32 - 1`.
/// `2^32 - 1` (0xffffffff) and any value at or beyond it is *not* an array
/// index and must be treated as a named string property. Returns the
/// canonical index when the string is a valid array index, else `None`.
///
/// This rejects non-canonical forms such as `"01"`, `"-1"`, `"1.5"`,
/// `"0xffffffff"` (== 2^32-1, not an array index), `"4294967296"` (>= 2^32),
/// and leading/trailing whitespace.
pub fn parse_array_index(key: &str) -> Option<usize> {
    // A canonical array-index string is a non-empty decimal run of digits
    // with no sign, no leading zero (unless it is exactly "0"), and no
    // surrounding whitespace. `str::parse::<u64>` accepts "  1  " and "+1",
    // so we validate the shape ourselves.
    if key.is_empty() {
        return None;
    }
    let bytes = key.as_bytes();
    if bytes[0].is_ascii_digit() {
        if bytes.len() > 1 && bytes[0] == b'0' {
            // "0" is canonical; "07", "0x1", "00" are not.
            return None;
        }
        if !bytes.iter().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // u64 is enough: any value >= 2^32 is rejected below.
        let n: u64 = key.parse().ok()?;
        // Strictly less than 2^32 - 1. Values equal to 2^32 - 1 are properties.
        if n < (1u64 << 32) - 1 {
            // SAFETY: n < 2^32 - 1 < usize::MAX on all supported platforms.
            return Some(n as usize);
        }
    }
    None
}

/// The value type used throughout the engine.
#[derive(Clone)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    BigInt(BigInt),
    String(Arc<str>),
    Object(GcIdx),
    Symbol(u32),
}

impl Value {
    pub fn undefined() -> Self {
        Value::Undefined
    }
    pub fn null() -> Self {
        Value::Null
    }
    pub fn from_bool(b: bool) -> Self {
        Value::Bool(b)
    }
    pub fn from_num(n: f64) -> Self {
        Value::Number(n)
    }
    pub fn from_string(s: &str) -> Self {
        Value::String(Arc::from(s))
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn is_nullish(&self) -> bool {
        matches!(self, Value::Null | Value::Undefined)
    }
    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Undefined | Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::BigInt(n) => !n.is_zero(),
            Value::String(s) => !s.is_empty(),
            Value::Object(_) | Value::Symbol(_) => true,
        }
    }

    pub fn type_of(&self) -> &'static str {
        match self {
            Value::Undefined => "undefined",
            Value::Null => "object",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::BigInt(_) => "bigint",
            Value::String(_) => "string",
            Value::Object(_) => "object",
            Value::Symbol(_) => "symbol",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::BigInt(a), Value::BigInt(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    /// SameValueZero comparison (used by Map/Set keys, Array.includes): like
    /// `==` except NaN equals NaN and -0 equals +0.
    pub fn same_value_zero(&self, other: &Value) -> bool {
        if let (Value::Number(a), Value::Number(b)) = (self, other) {
            // NaN matches NaN; everything else compares by value (so -0 == +0).
            a.is_nan() && b.is_nan() || a == b
        } else {
            self == other
        }
    }
}

/// Quick string conversion for argument handling (not spec-compliant ToString).
pub fn value_to_debug_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string(),
        Value::Number(n) => num_to_string(*n),
        Value::BigInt(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        _ => format!("{:?}", v),
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s),
            Value::BigInt(n) => write!(f, "{}n", n),
            Value::Object(_) => write!(f, "[object]"),
            Value::Symbol(_) => write!(f, "[symbol]"),
        }
    }
}

/// A heap-allocated JS object. All heap-resident data is one of these.
pub enum HeapObj {
    Object(ObjectData),
    Array(ArrayData),
    Function(FunctionData),
    Environment(EnvironmentData),
    Map(MapData),
    Set(SetData),
    WeakMap(WeakMapData),
    WeakSet(WeakSetData),
    Promise(PromiseData),
    Generator(GeneratorData),
    Iterator(IteratorData),
    LazyGenerator(LazyGeneratorData),
}

/// Generic JS object.
pub struct ObjectData {
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
    pub class_name: Option<Arc<str>>,
    /// Private field storage: `#name` -> value. Isolated from normal props
    /// (not enumerable, not accessible via [] or for...in).
    pub private_fields: Mutex<std::collections::HashMap<Arc<str>, Value>>,
    /// Wrapped primitive for boxed primitives created via `new Boolean(x)`,
    /// `new Number(x)`, `new String(x)`, or `Object(x)`. `None` for ordinary
    /// objects. `valueOf()` returns this so `new Number(5) + 1 === 6`.
    pub primitive: Mutex<Option<Value>>,
}

pub struct ArrayData {
    pub items: Mutex<Vec<Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    /// Largest array index currently stored as a named property rather
    /// than in the dense `items` backing store (see `MAX_DENSE_ARRAY_LEN`).
    /// `None` when no such out-of-band index exists, so `length` equals
    /// `items.len()`. Kept in sync only by `set_array_index`.
    pub sparse_max: Mutex<Option<usize>>,
}

pub struct FunctionData {
    pub name: Option<Arc<str>>,
    pub kind: FunctionKind,
    pub closure: GcIdx,
    pub prototype: Mutex<Option<Value>>,
    /// The function's [[Prototype]] (`__proto__`), normally
    /// `Function.prototype`. Kept separate from `prototype` (which is the
    /// object used as [[Prototype]] of instances created via `new`).
    pub proto: Mutex<Option<Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
}

pub enum FunctionKind {
    Native {
        func: crate::vm::NativeFn,
        length: usize,
    },
    Interpreted {
        func: std::sync::Arc<crate::function::FunctionDef>,
    },
    Bound {
        target: GcIdx,
        this_val: Value,
        bound_args: Vec<Value>,
    },
}

pub struct EnvironmentData {
    pub vars: Mutex<IndexMap<Arc<str>, Binding>>,
    pub parent: Mutex<Option<GcIdx>>,
    pub is_function_scope: bool,
    /// `with` statement object environment record: when `Some(obj)`, name
    /// lookups fall back to `obj`'s properties before reaching the parent.
    pub with_object: Mutex<Option<Value>>,
}

pub struct Binding {
    pub value: Mutex<Value>,
    pub kind: BindingKind,
    pub initialized: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingKind {
    Var,
    Let,
    Const,
}

pub struct MapData {
    pub entries: Mutex<Vec<(Value, Value)>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

pub struct SetData {
    pub items: Mutex<Vec<Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

/// A WeakMap holds (object-key -> value) pairs where the key is held
/// *weakly*: if the key is unreachable from anywhere except this WeakMap,
/// the entry is dropped during GC. Values are held strongly (per spec the
/// value is only reachable while the key is). Keys must be objects.
pub struct WeakMapData {
    /// (key heap idx, value) pairs. The key idx is not marked as a GC root,
    /// so an unreachable key causes the entry to be swept.
    pub entries: Mutex<Vec<(usize, Value)>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

/// A WeakSet holds object members weakly: an unreachable member is dropped
/// during GC. Members must be objects.
pub struct WeakSetData {
    pub items: Mutex<Vec<usize>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

pub struct PromiseData {
    pub state: Mutex<PromiseStatus>,
    pub result: Mutex<Value>,
    pub handlers: Mutex<Vec<PromiseHandler>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PromiseStatus {
    Pending,
    Fulfilled,
    Rejected,
}

pub struct PromiseHandler {
    pub on_fulfilled: Value,
    pub on_rejected: Value,
    pub derived: Option<GcIdx>,
}

pub struct GeneratorData {
    pub function: FunctionExpr,
    pub closure: GcIdx,
    pub state: Mutex<Vec<Value>>,
    pub ip: AtomicUsize,
    pub done: AtomicBool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

/// A lazy (pull-based) generator: its function body is executed incrementally
/// across `next()` calls, suspending at each `yield`.
pub struct LazyGeneratorData {
    /// The compiled function definition (holds the bytecode chunk).
    pub fdef: Arc<crate::function::FunctionDef>,
    /// Closure environment captured at creation time.
    pub closure: GcIdx,
    /// Current environment (advanced by PushScope/PopScope); saved/restored
    /// across yields so block scopes resume correctly.
    pub env: Mutex<GcIdx>,
    /// `this` value for the generator function call.
    pub this_val: Mutex<Value>,
    /// Arguments bound to the generator function's parameters.
    pub args: Mutex<Vec<Value>>,
    /// Current instruction pointer; 0 before the first `next()`.
    pub ip: AtomicUsize,
    /// Saved operand stack depth at suspension (for incremental runs we keep a
    /// per-generator value stack).
    pub stack: Mutex<Vec<Value>>,
    /// Local variables slot table.
    pub locals: Mutex<Vec<Value>>,
    /// Saved try/catch handler stack (so catches resume across yields).
    pub catch_stack: Mutex<Vec<(usize, u32)>>,
    /// True once the body has begun executing.
    pub started: AtomicBool,
    /// True once the body has run to completion (return / fall-off end).
    pub done: AtomicBool,
    /// The value sent into the generator via `next(v)` (consumed by `yield`).
    pub resume_value: Mutex<Value>,
    /// True for `async function*`: `next()` wraps results in a Promise.
    pub is_async: bool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

/// Internal iterator state used by `for...of` / `for...in` and the spread operator.
pub struct IteratorData {
    /// Remaining values to yield, in order (eager mode).
    pub items: Mutex<Vec<Value>>,
    /// Current position into `items` (eager mode).
    pub index: AtomicUsize,
    /// Lazy mode: a JS iterator object whose `next()` method is called on each
    /// pull. When `Some`, `items`/`index` are ignored. `done` is set once the
    /// JS `next()` reports `done: true`.
    pub lazy_iter: Mutex<Option<Value>>,
    /// Lazy mode: a generator object to pull via `resume_generator` on each
    /// `next()`. Mutually exclusive with `lazy_iter`. Preserves the
    /// generator's return value (used by `yield*`).
    pub generator: Mutex<Option<Value>>,
    pub done: AtomicBool,
}

#[derive(Clone)]
pub struct PropertyDescriptor {
    pub value: Value,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
    pub get: Option<Value>,
    pub set: Option<Value>,
    pub is_accessor: bool,
}

impl Default for PropertyDescriptor {
    fn default() -> Self {
        PropertyDescriptor {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: true,
            get: None,
            set: None,
            is_accessor: false,
        }
    }
}

impl PropertyDescriptor {
    pub fn data(value: Value) -> Self {
        PropertyDescriptor {
            value,
            ..Default::default()
        }
    }
}

impl HeapObj {
    /// Is this object callable?
    pub fn is_function(&self) -> bool {
        matches!(self, HeapObj::Function(_))
    }

    /// Common props accessor for any object kind.
    pub fn props(&self) -> &Mutex<IndexMap<PropertyKey, PropertyDescriptor>> {
        match self {
            HeapObj::Object(o) => &o.props,
            HeapObj::Array(a) => &a.props,
            HeapObj::Function(f) => &f.props,
            HeapObj::Map(m) => &m.props,
            HeapObj::Set(s) => &s.props,
            HeapObj::WeakMap(w) => &w.props,
            HeapObj::WeakSet(ws) => &ws.props,
            HeapObj::Promise(p) => &p.props,
            HeapObj::Generator(g) => &g.props,
            HeapObj::LazyGenerator(g) => &g.props,
            HeapObj::Iterator(_) => panic!("iterator has no props"),
            HeapObj::Environment(_) => panic!("env has no props"),
        }
    }

    /// Common proto accessor.
    pub fn proto(&self) -> &Mutex<Option<Value>> {
        match self {
            HeapObj::Object(o) => &o.proto,
            HeapObj::Array(a) => &a.proto,
            HeapObj::Function(f) => &f.proto,
            HeapObj::Map(m) => &m.proto,
            HeapObj::Set(s) => &s.proto,
            HeapObj::WeakMap(w) => &w.proto,
            HeapObj::WeakSet(ws) => &ws.proto,
            HeapObj::Promise(p) => &p.proto,
            HeapObj::Generator(g) => &g.proto,
            HeapObj::LazyGenerator(g) => &g.proto,
            HeapObj::Environment(_) => panic!("env has no proto"),
            HeapObj::Iterator(_) => panic!("iterator has no proto"),
        }
    }

    /// Class name for `Object.prototype.toString`.
    pub fn class_name(&self) -> &str {
        match self {
            HeapObj::Object(o) => o
                .class_name
                .as_ref()
                .map(|s| s.as_ref())
                .unwrap_or("Object"),
            HeapObj::Array(_) => "Array",
            HeapObj::Function(_) => "Function",
            HeapObj::Map(_) => "Map",
            HeapObj::Set(_) => "Set",
            HeapObj::WeakMap(_) => "WeakMap",
            HeapObj::WeakSet(_) => "WeakSet",
            HeapObj::Promise(_) => "Promise",
            HeapObj::Generator(_) => "Generator",
            HeapObj::LazyGenerator(_) => "Generator",
            HeapObj::Iterator(_) => "Iterator",
            HeapObj::Environment(_) => "Environment",
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, HeapObj::Array(_))
    }
    pub fn is_extensible(&self) -> bool {
        match self {
            HeapObj::Object(o) => o.extensible.load(Ordering::Relaxed),
            _ => true,
        }
    }
}

/// Render an f64 the way JS `String(n)` would.
pub fn num_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n == f64::INFINITY {
        return "Infinity".to_string();
    }
    if n == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }
    if n == 0.0 {
        // ES ToString: both +0 and -0 stringify to "0".
        return "0".to_string();
    }
    // ECMAScript uses exponential notation outside [1e-6, 1e21).
    let abs = n.abs();
    if !(1e-6..1e21).contains(&abs) {
        return format_exponential(n, abs);
    }
    if n.fract() == 0.0 && n.abs() < 1e21 && n.abs() <= i64::MAX as f64 {
        return format!("{}", n as i64);
    }
    let s = format!("{}", n);
    if s.ends_with(".0") {
        s[..s.len() - 2].to_string()
    } else {
        s
    }
}

/// Format a number in ECMAScript exponential notation (e.g. `1e+21`, `1e-7`).
/// Format a number in ECMAScript exponential notation (e.g. `1e+21`, `5e-17`).
///
/// Uses Rust's `{:e}` formatting, which already emits a correctly-rounded
/// shortest mantissa (avoiding the floating-point division error that the
/// previous `n / 10f64.powi(exp)` approach introduced, e.g. `5e-17` ->
/// `4.999999999999999e-17`). The only adjustment needed for ECMAScript is to
/// always emit an explicit exponent sign (`e+21` not `e21`), strip trailing
/// zeros from the mantissa, and strip leading zeros from the exponent digits.
fn format_exponential(n: f64, _abs: f64) -> String {
    let s = format!("{:e}", n);
    let epos = match s.find('e') {
        Some(p) => p,
        None => return s, // should not happen for finite non-zero inputs
    };
    let (mant, rest) = s.split_at(epos);
    let exp_str = &rest[1..]; // skip the 'e'
                              // Normalize the mantissa: drop any trailing zeros and a dangling `.` so
                              // that "5.000000" -> "5" and "1.500000" -> "1.5" (Rust's `{:e}` already
                              // emits the shortest form, but this keeps us correct regardless of how
                              // the formatter rounds a given value).
    let mant = mant.trim_end_matches('0').trim_end_matches('.');
    // Normalize the exponent: strip any leading zeros from the digits part
    // (e.g. "e-07" -> "e-7") and keep the sign explicit.
    let (sign, digits) = if let Some(d) = exp_str.strip_prefix('-') {
        ("-", d)
    } else if let Some(d) = exp_str.strip_prefix('+') {
        // ES exponent notation always emits an explicit sign.
        ("+", d)
    } else {
        ("+", exp_str)
    };
    let digits = digits.trim_start_matches('0');
    // A mantissa of "" (e.g. input rendered "0...e..") or digits of "" must
    // not produce an empty token.
    let mant = if mant.is_empty() { "0" } else { mant };
    let digits = if digits.is_empty() { "0" } else { digits };
    format!("{}e{}{}", mant, sign, digits)
}

// =========================================================================
// UTF-16 helpers
//
// JS strings are sequences of UTF-16 code units. Rust `&str`/`String` are
// UTF-8 and cannot represent lone (unpaired) surrogates. We model JS string
// length/indexing/charCodeAt on UTF-16 code units by converting to `Vec<u16>`
// for code-unit-level operations. Lone surrogates round-trip through the
// `u16` vector (they just can't be losslessly re-encoded to UTF-8).
// =========================================================================

/// Encode a Rust string into a Vec of UTF-16 code units. Supplementary
/// characters become surrogate pairs; the result mirrors `String.prototype.length`.
pub fn utf16_from_str(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Decode a sequence of UTF-16 code units back into a Rust `String`. Lone
/// surrogates are replaced with U+FFFD (matches JS `ToString` behavior where
/// the string already contains them internally; for our purposes this is
/// only called on well-formed sequences).
pub fn utf16_to_string(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// Build a Rust `String` from a series of UTF-16 code-unit numeric arguments
/// (as used by `String.fromCharCode`). Unlike `char::from_u32`, this handles
/// lone surrogates by emitting them directly into the u16 vector, then
/// decoding. Lone surrogates become U+FFFD in the resulting String (so the
/// length seen by the engine is correct even though the lone surrogate is
/// not round-trippable through UTF-8).
pub fn utf16_from_codes(codes: &[u16]) -> String {
    String::from_utf16_lossy(codes)
}

/// Return the JS length (UTF-16 code-unit count) of a Rust string.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Get the code unit at UTF-16 index `i`, or None if out of range.
pub fn utf16_get(s: &str, i: usize) -> Option<u16> {
    s.encode_utf16().nth(i)
}

/// Slice a Rust string by UTF-16 code-unit indices [start, end).
pub fn utf16_slice(s: &str, start: usize, end: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    let start = start.min(units.len());
    let end = end.clamp(start, units.len());
    String::from_utf16_lossy(&units[start..end])
}

/// Find the UTF-16 code-unit index of `needle` in `s` starting at or after
/// code-unit index `start`. Returns the code-unit index or None.
pub fn utf16_index_of(s: &str, needle: &str, start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(utf16_len(s)));
    }
    let hay: Vec<u16> = s.encode_utf16().collect();
    let nee: Vec<u16> = needle.encode_utf16().collect();
    let start = start.min(hay.len());
    if nee.len() > hay.len() - start {
        return None;
    }
    'outer: for i in start..=(hay.len() - nee.len()) {
        for j in 0..nee.len() {
            if hay[i + j] != nee[j] {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

/// Last index of `needle` at or before code-unit index `end`.
pub fn utf16_last_index_of(s: &str, needle: &str, end: usize) -> Option<usize> {
    let hay: Vec<u16> = s.encode_utf16().collect();
    let nee: Vec<u16> = needle.encode_utf16().collect();
    if nee.is_empty() {
        return Some(end.min(hay.len()));
    }
    let max_start = hay.len().saturating_sub(nee.len()).min(end);
    for i in (0..=max_start).rev() {
        if hay[i..i + nee.len()] == nee[..] {
            return Some(i);
        }
    }
    None
}
