//! Value model for the RuJa VM.
//!
//! `Value` is a tagged union. Heap objects live in the GC heap as `HeapObj`
//! and are referenced by `GcIdx`. The GC traces reachable objects from roots
//! and reclaims the rest, including reference cycles.

use crate::ast::FunctionExpr;
use indexmap::{IndexMap, IndexSet};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};

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

impl std::fmt::Debug for GcIdx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GcIdx({})", self.0)
    }
}

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

/// ES CanonicalNumericIndexString. `"-0"` is canonical even though
/// `String(-0)` is `"0"`.
pub fn canonical_numeric_index_string(key: &str) -> Option<f64> {
    if key == "-0" {
        return Some(-0.0);
    }
    let n = match key {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => match key.parse::<f64>() {
            Ok(n) => n,
            Err(_) => return None,
        },
    };
    if num_to_string(n) == key {
        Some(n)
    } else {
        None
    }
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
    /// Internal-only class private name identity. These values are stored in
    /// class lexical environments and are never exposed to user code.
    PrivateName(PrivateNameKey),
    /// An ES Reference record (spec 6.2.4). Used for LHS evaluation so that
    /// `GetValue`/`PutValue` can preserve the original binding even if it is
    /// deleted/recreated between the two steps (e.g. `with` + compound-assignment
    /// where a getter deletes the property). `base` is unresolved, an
    /// environment record (GcIdx), or a property base value; `name` is the
    /// referenced name; `strict` controls whether unresolved puts throw
    /// ReferenceError.
    Reference(Box<ReferenceRecord>),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PrivateNameKey {
    pub id: u64,
    pub description: Arc<str>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum PrivateSlotKey {
    Private(PrivateNameKey),
    Internal(Arc<str>),
}

/// A spec Reference record (6.2.4 The Reference Specification Type).
#[derive(Clone, Debug)]
pub struct ReferenceRecord {
    /// The base value: an environment record (GcIdx) for identifier references,
    /// or a Value (Object/with) for property references.
    pub base: ReferenceBase,
    /// The referenced name (property key or lexically resolved private name).
    pub name: ReferencedName,
    /// Whether the reference is strict (unresolved puts throw ReferenceError).
    pub strict: bool,
}

/// The [[ReferencedName]] component of a Reference Record.
#[derive(Clone, Debug)]
pub enum ReferencedName {
    Property(PropertyKey),
    Private(PrivateNameKey),
}

impl ReferencedName {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ReferencedName::Property(key) => key.as_str(),
            ReferencedName::Private(_) => None,
        }
    }
}

impl From<PropertyKey> for ReferencedName {
    fn from(key: PropertyKey) -> Self {
        ReferencedName::Property(key)
    }
}

/// The base of a Reference: either an environment record (for identifier
/// references) or a value (Object, primitive wrapper, etc.) for property
/// references. `Value` is boxed to break the recursive type size.
#[derive(Clone, Debug)]
pub enum ReferenceBase {
    /// No binding/property was found when the reference was created.
    Unresolvable,
    /// An environment record (heap index of an `EnvironmentData`).
    Environment(GcIdx),
    /// A property reached through an object environment record (`with` or
    /// global object identifier resolution), not a normal member expression.
    ObjectEnvironment(Box<Value>),
    /// A value base (Object, primitive wrapper, etc.) for property references.
    Value(Box<Value>),
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
            Value::Object(_) | Value::Symbol(_) | Value::PrivateName(_) => true,
            // References should be resolved via GetValue before reaching here.
            Value::Reference(_) => true,
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
            Value::PrivateName(_) => "object",
            Value::Reference(_) => "object",
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
            (Value::PrivateName(a), Value::PrivateName(b)) => a == b,
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

/// Wrapper around `Value` that implements `Hash` + `Eq` using SameValueZero
/// semantics (NaN == NaN, -0 == +0). Used as the key type for `IndexMap` in
/// Map/Set so lookups are O(1) instead of O(n) linear scans.
#[derive(Clone)]
pub struct MapKey(pub Value);

impl MapKey {
    pub fn new(value: Value) -> Self {
        if let Value::Number(n) = value {
            if n == 0.0 {
                return MapKey(Value::Number(0.0));
            }
            MapKey(Value::Number(n))
        } else {
            MapKey(value)
        }
    }
}

impl std::hash::Hash for MapKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.0 {
            Value::Undefined => 0u8.hash(state),
            Value::Null => 1u8.hash(state),
            Value::Bool(b) => {
                2u8.hash(state);
                b.hash(state);
            }
            Value::Number(n) => {
                3u8.hash(state);
                if n.is_nan() || *n == 0.0 {
                    0u64.hash(state);
                } else {
                    n.to_bits().hash(state);
                }
            }
            Value::BigInt(n) => {
                4u8.hash(state);
                n.hash(state);
            }
            Value::String(s) => {
                5u8.hash(state);
                s.hash(state);
            }
            Value::Object(idx) => {
                6u8.hash(state);
                idx.0.hash(state);
            }
            Value::Symbol(id) => {
                7u8.hash(state);
                id.hash(state);
            }
            Value::PrivateName(key) => {
                8u8.hash(state);
                key.hash(state);
            }
            Value::Reference(_) => {
                9u8.hash(state);
            }
        }
    }
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_value_zero(&other.0)
    }
}

impl Eq for MapKey {}

/// Quick string conversion for argument handling (not spec-compliant ToString).
pub fn value_to_debug_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string(),
        Value::Number(n) => num_to_string(*n),
        Value::BigInt(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::PrivateName(key) => format!("[private #{}]", key.description),
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
            Value::PrivateName(key) => write!(f, "[private #{}]", key.description),
            Value::Reference(r) => write!(f, "[reference {:?}]", r),
        }
    }
}

/// A TypedArray view over an ArrayBuffer. `buffer` is kept only for legacy
/// owned-storage objects from older snapshots; new allocations use
/// `viewed_array_buffer` plus byte offset/length slots.
pub struct TypedArrayData {
    pub buffer: Mutex<Vec<u8>>,
    pub viewed_array_buffer: Option<Value>,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub kind: TypedArrayKind,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct ArrayBufferData {
    pub bytes: Mutex<Vec<u8>>,
    pub detached: AtomicBool,
    pub immutable: AtomicBool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

pub struct DataViewData {
    pub buffer: Value,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TypedArrayKind {
    Uint8,
    Uint8Clamped,
    Int8,
    Uint16,
    Int16,
    Uint32,
    Int32,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

impl TypedArrayKind {
    pub fn element_size(&self) -> usize {
        match self {
            TypedArrayKind::Uint8 | TypedArrayKind::Uint8Clamped | TypedArrayKind::Int8 => 1,
            TypedArrayKind::Uint16 | TypedArrayKind::Int16 => 2,
            TypedArrayKind::Uint32 | TypedArrayKind::Int32 => 4,
            TypedArrayKind::Float32 => 4,
            TypedArrayKind::Float64 => 8,
            TypedArrayKind::BigInt64 | TypedArrayKind::BigUint64 => 8,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            TypedArrayKind::Uint8 => "Uint8Array",
            TypedArrayKind::Uint8Clamped => "Uint8ClampedArray",
            TypedArrayKind::Int8 => "Int8Array",
            TypedArrayKind::Uint16 => "Uint16Array",
            TypedArrayKind::Int16 => "Int16Array",
            TypedArrayKind::Uint32 => "Uint32Array",
            TypedArrayKind::Int32 => "Int32Array",
            TypedArrayKind::Float32 => "Float32Array",
            TypedArrayKind::Float64 => "Float64Array",
            TypedArrayKind::BigInt64 => "BigInt64Array",
            TypedArrayKind::BigUint64 => "BigUint64Array",
        }
    }
}

/// A Proxy object: intercepts property operations via handler traps.
pub struct ProxyData {
    pub target: Value,
    pub handler: Value,
    pub revoked: parking_lot::Mutex<bool>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

/// A heap-allocated JS object. All heap-resident data is one of these.
pub enum HeapObj {
    Object(ObjectData),
    Array(ArrayData),
    Function(FunctionData),
    Environment(EnvironmentData),
    Map(MapData),
    Set(SetData),
    CollectionIterator(CollectionIteratorData),
    RegExpStringIterator(RegExpStringIteratorData),
    WeakMap(WeakMapData),
    WeakSet(WeakSetData),
    Promise(PromiseData),
    Generator(GeneratorData),
    Iterator(IteratorData),
    LazyGenerator(LazyGeneratorData),
    Proxy(ProxyData),
    TypedArray(TypedArrayData),
    ArrayBuffer(ArrayBufferData),
    DataView(DataViewData),
}

/// Generic JS object.
pub struct ObjectData {
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
    pub class_name: Option<Arc<str>>,
    /// Engine-internal hidden slots used by built-ins. ECMAScript class
    /// private elements live on the owning `GcCell` so every object exotic
    /// can carry them without exposing them through normal properties.
    pub private_fields: Mutex<std::collections::HashMap<PrivateSlotKey, PrivateSlot>>,
    /// Wrapped primitive for boxed primitives created via `new Boolean(x)`,
    /// `new Number(x)`, `new String(x)`, or `Object(x)`. `None` for ordinary
    /// objects. `valueOf()` returns this so `new Number(5) + 1 === 6`.
    pub primitive: Mutex<Option<Value>>,
}

#[derive(Clone)]
pub enum PrivateSlot {
    Value(Value),
    Method(Value),
    Accessor {
        get: Option<Value>,
        set: Option<Value>,
    },
}

pub struct ArrayData {
    pub items: Mutex<Vec<Value>>,
    /// Dense index presence bits. `items[i] == Undefined` can mean either an
    /// explicit `undefined` value or an array hole.
    pub present: Mutex<Vec<bool>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
    /// Largest array index currently stored as a named property rather
    /// than in the dense `items` backing store (see `MAX_DENSE_ARRAY_LEN`).
    /// `None` when no such out-of-band index exists, so `length` equals
    /// `items.len()`. Kept in sync only by `set_array_index`.
    pub sparse_max: Mutex<Option<usize>>,
    /// Sloppy-mode mapped arguments object support. When present, integer
    /// indices alias the corresponding parameter binding in `env`.
    pub arguments_map: Mutex<Option<ArgumentsMap>>,
    /// RuJa currently represents arguments objects with ArrayData for reuse,
    /// but arguments are not Array exotic objects: writing past the indexed
    /// arguments must not grow `length`.
    pub is_arguments: AtomicBool,
}

pub struct ArgumentsMap {
    pub env: GcIdx,
    pub names: Vec<Option<Arc<str>>>,
}

impl ArrayData {
    pub fn new(items: Vec<Value>, proto: Option<Value>) -> Self {
        let present = vec![true; items.len()];
        Self::with_present(items, present, proto)
    }

    pub fn new_holes(len: usize, proto: Option<Value>) -> Self {
        Self::with_present(vec![Value::Undefined; len], vec![false; len], proto)
    }

    fn with_present(items: Vec<Value>, present: Vec<bool>, proto: Option<Value>) -> Self {
        ArrayData {
            items: Mutex::new(items),
            present: Mutex::new(present),
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(proto),
            extensible: AtomicBool::new(true),
            sparse_max: Mutex::new(None),
            arguments_map: Mutex::new(None),
            is_arguments: AtomicBool::new(false),
        }
    }

    pub fn is_dense_present(&self, index: usize) -> bool {
        self.present.lock().get(index).copied().unwrap_or(false)
    }
}

pub struct FunctionData {
    pub name: Option<Arc<str>>,
    pub kind: FunctionKind,
    pub closure: GcIdx,
    /// Lexically captured `new.target` for arrow functions. Non-arrow
    /// functions keep this as `undefined`; construct calls still use the
    /// per-frame `new_target` set by the VM.
    pub lexical_new_target: Value,
    /// True for class constructors: calling them without `new` is a TypeError.
    pub is_class_ctor: std::sync::atomic::AtomicBool,
    pub prototype: Mutex<Option<Value>>,
    /// The function's [[Prototype]] (`__proto__`), normally
    /// `Function.prototype`. Kept separate from `prototype` (which is the
    /// object used as [[Prototype]] of instances created via `new`).
    pub proto: Mutex<Option<Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub extensible: AtomicBool,
    /// Engine-internal hidden slots; class private elements are cell-owned.
    pub private_fields: Mutex<std::collections::HashMap<PrivateSlotKey, PrivateSlot>>,
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
    pub deletable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingKind {
    Var,
    Param,
    Let,
    /// Immutable named function-expression binding. Assignments are early-bound
    /// to this environment binding; strict code throws, sloppy code ignores.
    FunctionName,
    Const,
}

pub struct MapData {
    pub entries: Mutex<IndexMap<MapKey, Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

pub struct SetData {
    pub items: Mutex<IndexSet<MapKey>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CollectionIteratorKind {
    ArrayValues,
    MapEntries,
    MapKeys,
    MapValues,
    SetEntries,
    SetValues,
}

pub struct CollectionIteratorData {
    pub source: Value,
    pub kind: CollectionIteratorKind,
    pub index: AtomicUsize,
    pub done: AtomicBool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

pub struct RegExpStringIteratorData {
    pub matcher: Value,
    pub string: Arc<str>,
    pub global: bool,
    pub full_unicode: bool,
    pub done: AtomicBool,
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

#[derive(Clone)]
pub struct PromiseReactionCapability {
    pub promise: Value,
    pub resolve: Value,
    pub reject: Value,
}

pub struct PromiseHandler {
    pub on_fulfilled: Value,
    pub on_rejected: Value,
    pub derived: Option<PromiseReactionCapability>,
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
    pub catch_stack: Mutex<Vec<(usize, u32, GcIdx)>>,
    /// Saved try/finally guard stack (so generator return/throw resumes can
    /// run active finally blocks after yielding inside protected regions).
    pub finally_stack: Mutex<Vec<(usize, u32)>>,
    /// Monotonic guard sequence restored with catch/finally stacks.
    pub guard_seq: AtomicU32,
    /// Pending completion saved when a generator yields from inside a finally.
    pub finally_completion_tag: AtomicU8,
    pub finally_completion_val: Mutex<Value>,
    /// True once the body has begun executing.
    pub started: AtomicBool,
    /// True once the body has run to completion (return / fall-off end).
    pub done: AtomicBool,
    /// True when suspension occurred inside `yield*` and the next completion
    /// must be forwarded to the delegated iterator.
    pub delegating: AtomicBool,
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
    /// Cached `next` method for `lazy_iter`, captured by GetIterator.
    pub lazy_next: Mutex<Option<Value>>,
    /// Lazy mode: a generator object to pull via `resume_generator` on each
    /// `next()`. Mutually exclusive with `lazy_iter`. Preserves the
    /// generator's return value (used by `yield*`).
    pub generator: Mutex<Option<Value>>,
    /// Lazy ArrayIterator mode. Keeps the iterated array-like object alive and
    /// reads `length` plus the current integer property on each pull, so array
    /// growth, contraction, accessors, and arguments object mapping are visible.
    pub array_like: Mutex<Option<Value>>,
    /// Lazy `for...in` mode. `items` stores the initial key list, while this
    /// keeps the enumerated object alive and lets each key be revalidated
    /// immediately before it is yielded.
    pub for_in_source: Mutex<Option<Value>>,
    /// Object where each `for...in` key was discovered. This is aligned with
    /// `items` for for-in iterators and empty for all other iterator kinds.
    pub for_in_key_sources: Mutex<Vec<Value>>,
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

    /// A temporary empty object used as a placeholder when `Heap::with_obj`
    /// is reentered on the same index. Reentrant reads already diverge from
    /// ES; this keeps them from panicking ("use after free") instead of
    /// returning the real (temporarily-borrowed) value. The outer `with_obj`
    /// frame restores the real object, so the placeholder is never persisted.
    pub fn placeholder() -> HeapObj {
        HeapObj::Object(ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(None),
            extensible: AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        })
    }

    /// Common props accessor for any object kind.
    pub fn props(&self) -> &Mutex<IndexMap<PropertyKey, PropertyDescriptor>> {
        match self {
            HeapObj::Object(o) => &o.props,
            HeapObj::Array(a) => &a.props,
            HeapObj::Function(f) => &f.props,
            HeapObj::Map(m) => &m.props,
            HeapObj::Set(s) => &s.props,
            HeapObj::CollectionIterator(i) => &i.props,
            HeapObj::RegExpStringIterator(i) => &i.props,
            HeapObj::WeakMap(w) => &w.props,
            HeapObj::WeakSet(ws) => &ws.props,
            HeapObj::Promise(p) => &p.props,
            HeapObj::Generator(g) => &g.props,
            HeapObj::LazyGenerator(g) => &g.props,
            HeapObj::Proxy(p) => &p.props,
            HeapObj::TypedArray(t) => &t.props,
            HeapObj::ArrayBuffer(a) => &a.props,
            HeapObj::DataView(d) => &d.props,
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
            HeapObj::CollectionIterator(i) => &i.proto,
            HeapObj::RegExpStringIterator(i) => &i.proto,
            HeapObj::WeakMap(w) => &w.proto,
            HeapObj::WeakSet(ws) => &ws.proto,
            HeapObj::Promise(p) => &p.proto,
            HeapObj::Generator(g) => &g.proto,
            HeapObj::LazyGenerator(g) => &g.proto,
            HeapObj::Proxy(p) => &p.proto,
            HeapObj::TypedArray(t) => &t.proto,
            HeapObj::ArrayBuffer(a) => &a.proto,
            HeapObj::DataView(d) => &d.proto,
            HeapObj::Environment(_) => panic!("env has no proto"),
            HeapObj::Iterator(_) => panic!("iterator has no proto"),
        }
    }

    /// Class name for `Object.prototype.toString`.
    pub fn class_name(&self) -> &str {
        match self {
            HeapObj::Object(o) => {
                if let Some(name) = o.class_name.as_ref() {
                    return name.as_ref();
                }
                match o.primitive.lock().as_ref() {
                    Some(Value::String(_)) => "String",
                    Some(Value::Number(_)) => "Number",
                    Some(Value::Bool(_)) => "Boolean",
                    Some(Value::BigInt(_)) => "BigInt",
                    Some(Value::Symbol(_)) => "Symbol",
                    _ => "Object",
                }
            }
            HeapObj::Array(a) => {
                if a.is_arguments.load(Ordering::Relaxed) {
                    "Arguments"
                } else {
                    "Array"
                }
            }
            HeapObj::Function(_) => "Function",
            HeapObj::Map(_) => "Map",
            HeapObj::Set(_) => "Set",
            HeapObj::CollectionIterator(i) => match i.kind {
                CollectionIteratorKind::ArrayValues => "Array Iterator",
                CollectionIteratorKind::MapEntries
                | CollectionIteratorKind::MapKeys
                | CollectionIteratorKind::MapValues => "Map Iterator",
                CollectionIteratorKind::SetEntries | CollectionIteratorKind::SetValues => {
                    "Set Iterator"
                }
            },
            HeapObj::RegExpStringIterator(_) => "RegExp String Iterator",
            HeapObj::WeakMap(_) => "WeakMap",
            HeapObj::WeakSet(_) => "WeakSet",
            HeapObj::Promise(_) => "Promise",
            HeapObj::Generator(_) => "Generator",
            HeapObj::LazyGenerator(_) => "Generator",
            HeapObj::Iterator(_) => "Iterator",
            HeapObj::Environment(_) => "Environment",
            HeapObj::Proxy(_) => "Object",
            HeapObj::TypedArray(t) => t.kind.name(),
            HeapObj::ArrayBuffer(_) => "ArrayBuffer",
            HeapObj::DataView(_) => "DataView",
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self, HeapObj::Array(_))
    }
    pub fn is_extensible(&self) -> bool {
        match self {
            HeapObj::Object(o) => o.extensible.load(Ordering::Relaxed),
            HeapObj::Array(a) => a.extensible.load(Ordering::Relaxed),
            HeapObj::Function(f) => f.extensible.load(Ordering::Relaxed),
            HeapObj::TypedArray(t) => t.extensible.load(Ordering::Relaxed),
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
// for code-unit-level operations. Lone surrogates are preserved internally as
// private-use sentinels and mapped back to their original code units here.
// =========================================================================

const SURROGATE_SENTINEL_BASE: u32 = 0xF0000;

fn surrogate_to_sentinel(unit: u16) -> char {
    debug_assert!((0xD800..=0xDFFF).contains(&unit));
    char::from_u32(SURROGATE_SENTINEL_BASE + (unit as u32 - 0xD800)).unwrap()
}

fn sentinel_to_surrogate(ch: char) -> Option<u16> {
    let cp = ch as u32;
    if (SURROGATE_SENTINEL_BASE..=SURROGATE_SENTINEL_BASE + 0x7FF).contains(&cp) {
        Some(0xD800 + (cp - SURROGATE_SENTINEL_BASE) as u16)
    } else {
        None
    }
}

/// Return the single JS UTF-16 code unit represented by an internal char.
/// Supplementary scalar chars return None because they encode to a pair.
pub(crate) fn utf16_single_unit_from_internal_char(ch: char) -> Option<u16> {
    sentinel_to_surrogate(ch).or_else(|| {
        let cp = ch as u32;
        (cp <= 0xFFFF).then_some(cp as u16)
    })
}

/// Encode an internal Rust string into JS UTF-16 code units. Supplementary
/// characters become surrogate pairs, and RuJa's private lone-surrogate
/// sentinels become their original code units.
pub fn utf16_from_str(s: &str) -> Vec<u16> {
    let mut units = Vec::new();
    for ch in s.chars() {
        if let Some(unit) = sentinel_to_surrogate(ch) {
            units.push(unit);
        } else {
            let mut buf = [0; 2];
            units.extend_from_slice(ch.encode_utf16(&mut buf));
        }
    }
    units
}

/// Decode a sequence of UTF-16 code units back into a Rust `String`. Lone
/// surrogates are preserved using private-use sentinels because Rust `String`
/// cannot directly represent them. Valid pairs whose scalar value collides with
/// that sentinel range are kept as two sentinel-backed code units so later
/// UTF-16 round trips preserve the original JS string length.
pub fn utf16_to_string(units: &[u16]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < units.len() {
        let unit = units[i];
        if (0xD800..=0xDBFF).contains(&unit) && i + 1 < units.len() {
            let low = units[i + 1];
            if (0xDC00..=0xDFFF).contains(&low) {
                let cp = 0x10000 + (((unit as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                if (SURROGATE_SENTINEL_BASE..=SURROGATE_SENTINEL_BASE + 0x7FF).contains(&cp) {
                    out.push(surrogate_to_sentinel(unit));
                    out.push(surrogate_to_sentinel(low));
                    i += 2;
                    continue;
                } else if let Some(ch) = char::from_u32(cp) {
                    out.push(ch);
                    i += 2;
                    continue;
                }
            }
        }
        if (0xD800..=0xDFFF).contains(&unit) {
            out.push(surrogate_to_sentinel(unit));
        } else if let Some(ch) = char::from_u32(unit as u32) {
            out.push(ch);
        }
        i += 1;
    }
    out
}

/// Build a Rust `String` from a series of UTF-16 code-unit numeric arguments
/// (as used by `String.fromCharCode`). Lone surrogates are preserved through
/// private-use sentinels. Pairs whose scalar would collide with that sentinel
/// range are also kept as two sentinel-backed code units so later UTF-16
/// operations can still distinguish them from a lone surrogate.
pub fn utf16_from_codes(codes: &[u16]) -> String {
    utf16_to_string(codes)
}

/// Return the JS length (UTF-16 code-unit count) of a Rust string.
/// Fast path: if the string is pure ASCII (the common case in JS),
/// the length equals the byte length — no UTF-16 encoding needed.
pub fn utf16_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        utf16_from_str(s).len()
    }
}

/// Convert a UTF-16 code-unit index into a UTF-8 byte index when the position
/// is also a Rust string boundary. Returns `None` for positions inside a
/// supplementary character or past the end.
pub fn utf16_index_to_byte(s: &str, index: usize) -> Option<usize> {
    if s.is_ascii() {
        return (index <= s.len()).then_some(index);
    }

    let mut utf16_pos = 0;
    for (byte_pos, ch) in s.char_indices() {
        if utf16_pos == index {
            return Some(byte_pos);
        }
        utf16_pos += if sentinel_to_surrogate(ch).is_some() {
            1
        } else {
            ch.len_utf16()
        };
        if utf16_pos > index {
            return None;
        }
    }
    (utf16_pos == index).then_some(s.len())
}

/// Get the code unit at UTF-16 index `i`, or None if out of range.
/// Fast path: ASCII strings can index directly by byte.
pub fn utf16_get(s: &str, i: usize) -> Option<u16> {
    if s.is_ascii() {
        s.as_bytes().get(i).map(|b| *b as u16)
    } else {
        utf16_from_str(s).get(i).copied()
    }
}

/// Slice a Rust string by UTF-16 code-unit indices [start, end).
/// Fast path: ASCII strings can slice directly by byte index.
pub fn utf16_slice(s: &str, start: usize, end: usize) -> String {
    if s.is_ascii() {
        let len = s.len();
        let start = start.min(len);
        let end = end.clamp(start, len);
        s[start..end].to_string()
    } else {
        let units = utf16_from_str(s);
        let start = start.min(units.len());
        let end = end.clamp(start, units.len());
        utf16_to_string(&units[start..end])
    }
}

/// Split a JS string into the values produced by StringIterator. Valid
/// surrogate pairs are yielded as one supplementary character; lone surrogates
/// are yielded as one code unit.
pub fn utf16_code_point_strings(s: &str) -> Vec<String> {
    let units = utf16_from_str(s);
    let mut out = Vec::new();
    let mut i = 0;
    while i < units.len() {
        let unit = units[i];
        if (0xD800..=0xDBFF).contains(&unit) && i + 1 < units.len() {
            let low = units[i + 1];
            if (0xDC00..=0xDFFF).contains(&low) {
                out.push(utf16_to_string(&[unit, low]));
                i += 2;
                continue;
            }
        }
        out.push(utf16_to_string(&[unit]));
        i += 1;
    }
    out
}

/// Find the UTF-16 code-unit index of `needle` in `s` starting at or after
/// code-unit index `start`. Returns the code-unit index or None.
pub fn utf16_index_of(s: &str, needle: &str, start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(utf16_len(s)));
    }
    let hay = utf16_from_str(s);
    let nee = utf16_from_str(needle);
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
    let hay = utf16_from_str(s);
    let nee = utf16_from_str(needle);
    if nee.is_empty() {
        return Some(end.min(hay.len()));
    }
    if nee.len() > hay.len() {
        return None;
    }
    let max_start = hay.len().saturating_sub(nee.len()).min(end);
    for i in (0..=max_start).rev() {
        if hay[i..i + nee.len()] == nee[..] {
            return Some(i);
        }
    }
    None
}
