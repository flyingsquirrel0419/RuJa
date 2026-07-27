//! Value model for the RuJa VM.
//!
//! `Value` is a tagged union. Heap objects live in the GC heap as `HeapObj`
//! and are referenced by `GcIdx`. The GC traces reachable objects from roots
//! and reclaims the rest, including reference cycles.

use crate::ast::FunctionExpr;
use indexmap::{IndexMap, IndexSet};
use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};

use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
struct DecimalKey<const N: usize> {
    bytes: [u8; N],
    len: u8,
}

impl<const N: usize> DecimalKey<N> {
    fn new(value: u64) -> Self {
        let mut bytes = [0; N];
        let mut cursor = bytes.len();
        let mut remaining = value;
        loop {
            cursor -= 1;
            bytes[cursor] = b'0' + (remaining % 10) as u8;
            remaining /= 10;
            if remaining == 0 {
                break;
            }
        }
        let len = bytes.len() - cursor;
        bytes.copy_within(cursor.., 0);
        Self {
            bytes,
            len: len as u8,
        }
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("array-index digits are valid UTF-8")
    }
}

type ArrayIndexKey = DecimalKey<10>;
type IntegerIndexKey = DecimalKey<20>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct IntegerIndexStr(IntegerIndexKey);

impl std::ops::Deref for IntegerIndexStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AsRef<str> for IntegerIndexStr {
    fn as_ref(&self) -> &str {
        self
    }
}

#[cfg(target_pointer_width = "64")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlinePropertyKey {
    Index(u32),
    Symbol(u32),
}

#[cfg(target_pointer_width = "32")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InlinePropertyKey {
    Symbol(u32),
}

#[derive(Clone, Debug)]
enum PropertyKeyRepr {
    Str(Arc<str>),
    Inline(InlinePropertyKey),
}

/// Borrowed or stack-formatted view of a string property key.
#[derive(Clone, Copy, Debug)]
enum PropertyKeyStrRepr<'a> {
    Borrowed(&'a str),
    Index(ArrayIndexKey),
}

#[derive(Clone, Copy, Debug)]
pub struct PropertyKeyStr<'a>(PropertyKeyStrRepr<'a>);

impl std::ops::Deref for PropertyKeyStr<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            PropertyKeyStrRepr::Borrowed(value) => value,
            PropertyKeyStrRepr::Index(index) => index.as_str(),
        }
    }
}

impl AsRef<str> for PropertyKeyStr<'_> {
    fn as_ref(&self) -> &str {
        self
    }
}

impl PartialEq<&str> for PropertyKeyStr<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_ref() == *other
    }
}

impl PartialEq<PropertyKeyStr<'_>> for &str {
    fn eq(&self, other: &PropertyKeyStr<'_>) -> bool {
        *self == other.as_ref()
    }
}

impl PartialEq<PropertyKeyStr<'_>> for String {
    fn eq(&self, other: &PropertyKeyStr<'_>) -> bool {
        self.as_str() == other.as_ref()
    }
}

impl Default for PropertyKeyStr<'_> {
    fn default() -> Self {
        PropertyKeyStr(PropertyKeyStrRepr::Borrowed(""))
    }
}

impl fmt::Display for PropertyKeyStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

/// A string, inline canonical array-index, or Symbol property key.
#[derive(Clone, Debug)]
pub struct PropertyKey(PropertyKeyRepr);

const _: () = assert!(std::mem::size_of::<PropertyKey>() == std::mem::size_of::<Arc<str>>());

impl PropertyKey {
    pub fn from_string(s: String) -> Self {
        #[cfg(target_pointer_width = "64")]
        if let Some(index) = parse_array_index(&s) {
            return PropertyKey::from_array_index(index as u32);
        }
        PropertyKey(PropertyKeyRepr::Str(Arc::from(s.as_str())))
    }

    pub fn from_rc(s: Arc<str>) -> Self {
        #[cfg(target_pointer_width = "64")]
        if let Some(index) = parse_array_index(&s) {
            return PropertyKey::from_array_index(index as u32);
        }
        PropertyKey(PropertyKeyRepr::Str(s))
    }

    pub fn from_array_index(index: u32) -> Self {
        if index == u32::MAX {
            return PropertyKey(PropertyKeyRepr::Str(Arc::from("4294967295")));
        }
        #[cfg(target_pointer_width = "64")]
        {
            PropertyKey(PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)))
        }
        #[cfg(target_pointer_width = "32")]
        {
            PropertyKey(PropertyKeyRepr::Str(Arc::from(
                ArrayIndexKey::new(index as u64).as_str(),
            )))
        }
    }

    pub(crate) fn from_integer_index(index: u64) -> Self {
        if index < u32::MAX as u64 {
            PropertyKey::from_array_index(index as u32)
        } else {
            PropertyKey(PropertyKeyRepr::Str(Arc::from(
                IntegerIndexKey::new(index).as_str(),
            )))
        }
    }

    pub(crate) fn integer_index_str(index: u64) -> IntegerIndexStr {
        IntegerIndexStr(IntegerIndexKey::new(index))
    }

    pub fn symbol(id: u32) -> Self {
        PropertyKey(PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(id)))
    }

    /// Return a borrowed or stack-formatted view for a string key.
    pub fn as_str(&self) -> Option<PropertyKeyStr<'_>> {
        match &self.0 {
            PropertyKeyRepr::Str(value) => {
                Some(PropertyKeyStr(PropertyKeyStrRepr::Borrowed(value)))
            }
            #[cfg(target_pointer_width = "64")]
            PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)) => Some(PropertyKeyStr(
                PropertyKeyStrRepr::Index(ArrayIndexKey::new(*index as u64)),
            )),
            PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(_)) => None,
        }
    }

    pub fn array_index(&self) -> Option<u32> {
        match &self.0 {
            #[cfg(target_pointer_width = "64")]
            PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)) => Some(*index),
            PropertyKeyRepr::Str(value) => parse_array_index(value).map(|index| index as u32),
            PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(_)) => None,
        }
    }

    pub fn symbol_id(&self) -> Option<u32> {
        match self.0 {
            PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(id)) => Some(id),
            _ => None,
        }
    }

    pub fn is_str(&self, expected: &str) -> bool {
        self.as_str()
            .is_some_and(|value| value.as_ref() == expected)
    }

    pub(crate) fn string_arc(&self) -> Option<Arc<str>> {
        match &self.0 {
            PropertyKeyRepr::Str(value) => Some(value.clone()),
            #[cfg(target_pointer_width = "64")]
            PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)) => {
                Some(Arc::from(ArrayIndexKey::new(*index as u64).as_str()))
            }
            PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(_)) => None,
        }
    }

    pub(crate) fn into_string_arc(self) -> Option<Arc<str>> {
        match self.0 {
            PropertyKeyRepr::Str(value) => Some(value),
            #[cfg(target_pointer_width = "64")]
            PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)) => {
                Some(Arc::from(ArrayIndexKey::new(index as u64).as_str()))
            }
            PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(_)) => None,
        }
    }

    pub fn is_symbol(&self) -> bool {
        self.symbol_id().is_some()
    }
}

impl From<&str> for PropertyKey {
    fn from(s: &str) -> Self {
        #[cfg(target_pointer_width = "64")]
        if let Some(index) = parse_array_index(s) {
            return PropertyKey::from_array_index(index as u32);
        }
        PropertyKey(PropertyKeyRepr::Str(Arc::from(s)))
    }
}

impl From<String> for PropertyKey {
    fn from(s: String) -> Self {
        PropertyKey::from_string(s)
    }
}

impl From<Arc<str>> for PropertyKey {
    fn from(s: Arc<str>) -> Self {
        PropertyKey::from_rc(s)
    }
}

impl std::hash::Hash for PropertyKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.0 {
            PropertyKeyRepr::Str(value) => {
                0u8.hash(state);
                value.hash(state);
            }
            #[cfg(target_pointer_width = "64")]
            PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)) => {
                0u8.hash(state);
                ArrayIndexKey::new(*index as u64).as_str().hash(state);
            }
            PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(id)) => {
                1u8.hash(state);
                id.hash(state);
            }
        }
    }
}

impl PartialEq for PropertyKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (PropertyKeyRepr::Str(a), PropertyKeyRepr::Str(b)) => a == b,
            #[cfg(target_pointer_width = "64")]
            (
                PropertyKeyRepr::Inline(InlinePropertyKey::Index(a)),
                PropertyKeyRepr::Inline(InlinePropertyKey::Index(b)),
            ) => a == b,
            (
                PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(a)),
                PropertyKeyRepr::Inline(InlinePropertyKey::Symbol(b)),
            ) => a == b,
            #[cfg(target_pointer_width = "64")]
            (PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)), PropertyKeyRepr::Str(s))
            | (PropertyKeyRepr::Str(s), PropertyKeyRepr::Inline(InlinePropertyKey::Index(index))) => {
                parse_array_index(s) == Some(*index as usize)
            }
            _ => false,
        }
    }
}

impl Eq for PropertyKey {}

#[cfg(test)]
mod property_key_tests {
    use super::{InlinePropertyKey, PropertyKey, PropertyKeyRepr};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::sync::Arc;

    #[cfg(target_pointer_width = "64")]
    fn hash(key: &PropertyKey) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn canonical_array_indices_preserve_text_on_32_bit_targets() {
        for (text, value) in [("0", 0), ("1", 1), ("4294967294", u32::MAX - 1)] {
            let key = PropertyKey::from_array_index(value);
            assert_eq!(key.as_str().unwrap(), text);
            assert_eq!(key.array_index(), Some(value));
        }

        let original: Arc<str> = Arc::from("123");
        let key = PropertyKey::from_rc(original.clone());
        assert!(Arc::ptr_eq(
            &original,
            &key.string_arc().expect("numeric key remains a string")
        ));
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn canonical_array_indices_are_inline_and_exact() {
        for (text, value) in [("0", 0), ("1", 1), ("4294967294", u32::MAX - 1)] {
            let key = PropertyKey::from(text);
            assert!(matches!(
                key,
                PropertyKey(PropertyKeyRepr::Inline(InlinePropertyKey::Index(index)))
                    if index == value
            ));
            assert_eq!(key.as_str().unwrap(), text);
        }

        for text in ["00", "01", "-0", "4294967295", "1.5"] {
            assert!(matches!(
                PropertyKey::from(text),
                PropertyKey(PropertyKeyRepr::Str(_))
            ));
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn inline_and_arc_string_keys_share_equality_and_hash() {
        let inline = PropertyKey::from_array_index(1234);
        let arc = PropertyKey(PropertyKeyRepr::Str(Arc::from("1234")));
        assert_eq!(inline, arc);
        assert_eq!(hash(&inline), hash(&arc));

        let mut keys = std::collections::HashMap::new();
        keys.insert(inline, "value");
        assert_eq!(keys.get(&arc), Some(&"value"));
    }

    #[test]
    fn inline_index_preserves_property_key_storage_size() {
        assert_eq!(
            std::mem::size_of::<PropertyKey>(),
            std::mem::size_of::<Arc<str>>()
        );
    }

    #[test]
    fn integer_index_keys_preserve_array_index_and_named_boundaries() {
        for (index, text, array_index) in [
            (4_294_967_294, "4294967294", Some(4_294_967_294)),
            (4_294_967_295, "4294967295", None),
            (4_294_967_296, "4294967296", None),
            (9_007_199_254_740_990, "9007199254740990", None),
            (u64::MAX, "18446744073709551615", None),
        ] {
            let key = PropertyKey::from_integer_index(index);
            assert_eq!(key.as_str().as_deref(), Some(text));
            assert_eq!(PropertyKey::integer_index_str(index).as_ref(), text);
            assert_eq!(key.array_index(), array_index);
            assert_eq!(key, PropertyKey::from(text));
        }
    }
}

use num_bigint::{BigInt, BigUint};
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
    BigInt(Arc<BigInt>),
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
    /// Explicit [[ThisValue]] for a super Reference. Ordinary property and
    /// environment References derive their call receiver from [[Base]].
    pub this_value: Option<Box<Value>>,
}

/// The [[ReferencedName]] component of a Reference Record.
#[derive(Clone, Debug)]
pub enum ReferencedName {
    Property(PropertyKey),
    /// A computed property name whose ToPropertyKey operation is deferred
    /// until GetValue/PutValue. Assignment and delete can observe that delay.
    UncoercedProperty(Box<Value>),
    Private(PrivateNameKey),
}

impl ReferencedName {
    pub fn as_str(&self) -> Option<PropertyKeyStr<'_>> {
        match self {
            ReferencedName::Property(key) => key.as_str(),
            ReferencedName::UncoercedProperty(_) | ReferencedName::Private(_) => None,
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
    /// Visit every VM-heap index reachable through an internal value. Heap
    /// tracing, temporary-root sizing, and root publication share this walk so
    /// Reference layout changes cannot make their root sets drift apart.
    pub(crate) fn visit_gc_roots(&self, visit: &mut impl FnMut(usize)) {
        match self {
            Value::Object(index) => visit(index.0),
            Value::Reference(reference) => reference.visit_gc_roots(visit),
            _ => {}
        }
    }
}

impl ReferenceRecord {
    pub(crate) fn visit_gc_roots(&self, visit: &mut impl FnMut(usize)) {
        match &self.base {
            ReferenceBase::Unresolvable => {}
            ReferenceBase::Environment(environment) => visit(environment.0),
            ReferenceBase::ObjectEnvironment(base) | ReferenceBase::Value(base) => {
                base.visit_gc_roots(visit)
            }
        }
        if let Some(this_value) = &self.this_value {
            this_value.visit_gc_roots(visit);
        }
        if let ReferencedName::UncoercedProperty(name) = &self.name {
            name.visit_gc_roots(visit);
        }
    }
}

impl Value {
    /// Wrap an immutable BigInt so cloning a `Value` remains constant-time.
    pub fn bigint(value: BigInt) -> Self {
        Self::BigInt(Arc::new(value))
    }

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

impl From<BigInt> for Value {
    fn from(value: BigInt) -> Self {
        Self::bigint(value)
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
    pub length_tracking: bool,
    pub kind: TypedArrayKind,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct ArrayBufferData {
    pub bytes: Arc<Mutex<Vec<u8>>>,
    pub waiters: Arc<
        Mutex<std::collections::HashMap<usize, std::collections::VecDeque<Arc<AtomicsWaiter>>>>,
    >,
    pub detached: AtomicBool,
    pub immutable: AtomicBool,
    pub shared: bool,
    pub max_byte_length: Option<usize>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct AtomicsWaiter {
    pub notified: Mutex<bool>,
    pub wake: Condvar,
}

pub struct DataViewData {
    pub buffer: Value,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub length_tracking: bool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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
    // [[Call]] and [[Construct]] presence is fixed by ProxyCreate and survives revocation.
    pub callable: bool,
    pub constructable: bool,
    pub revoked: parking_lot::Mutex<bool>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
}

/// ECMAScript Module Namespace Exotic Object.
pub type ModuleBinding = (GcIdx, Arc<str>);

pub struct ModuleNamespaceData {
    /// Sorted exported names mapped to their resolved live environment binding.
    pub exports: Mutex<IndexMap<Arc<str>, ModuleBinding>>,
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
    IteratorHelper(IteratorHelperData),
    RegExpStringIterator(RegExpStringIteratorData),
    WeakMap(WeakMapData),
    WeakSet(WeakSetData),
    WeakRef(WeakRefData),
    FinalizationRegistry(FinalizationRegistryData),
    Promise(PromiseData),
    Generator(GeneratorData),
    Iterator(IteratorData),
    LazyGenerator(LazyGeneratorData),
    Proxy(ProxyData),
    ModuleNamespace(ModuleNamespaceData),
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
    /// `items.len()`. Kept in sync by the shared property-storage publisher.
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

    pub(crate) fn try_new(
        items: Vec<Value>,
        proto: Option<Value>,
    ) -> Result<Self, std::collections::TryReserveError> {
        let mut present = Vec::new();
        present.try_reserve_exact(items.len())?;
        present.resize(items.len(), true);
        Ok(Self::with_present(items, present, proto))
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
    /// The function's [[HomeObject]], set for concise methods and accessors
    /// when they are installed on an object or class element.
    pub home_object: Mutex<Option<Value>>,
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
        /// `None` means the builtin has no [[Construct]] internal method.
        /// A mode is present only for native constructors and also selects
        /// who owns receiver allocation and prototype observation.
        construct_mode: Option<NativeConstructMode>,
    },
    Interpreted {
        func: std::sync::Arc<crate::function::FunctionDef>,
    },
    Bound {
        target: GcIdx,
        this_val: Value,
        bound_args: Vec<Value>,
        constructable: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeConstructMode {
    /// The native body allocates the result, but `.prototype` is observed first.
    InternalEagerPrototype,
    /// The native body controls whether and when `.prototype` is observed.
    InternalDeferredPrototype,
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
    /// Target environment and binding name for an immutable live import.
    pub indirect: Option<(GcIdx, Arc<str>)>,
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
    Import,
}

pub struct MapData {
    pub entries: Mutex<IndexMap<MapKey, Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct SetData {
    pub items: Mutex<IndexSet<MapKey>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CollectionIteratorKind {
    WrappedIterator,
    StringValues,
    ArrayEntries,
    ArrayKeys,
    ArrayValues,
    MapEntries,
    MapKeys,
    MapValues,
    SetEntries,
    SetValues,
}

pub struct CollectionIteratorData {
    pub source: Mutex<Value>,
    pub next_method: Mutex<Option<Value>>,
    pub kind: CollectionIteratorKind,
    pub index: Mutex<u64>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IteratorHelperKind {
    Map,
    Filter,
    FlatMap,
    Take,
    Drop,
    Concat,
    Zip,
    ZipKeyed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IteratorZipMode {
    Shortest,
    Longest,
    Strict,
}

#[derive(Clone)]
pub struct IteratorHelperInner {
    pub iterator: Value,
    pub next_method: Value,
}

pub struct IteratorConcatIterable {
    pub iterable: Value,
    pub open_method: Value,
}

pub struct IteratorHelperData {
    pub resume_realm: GcIdx,
    pub iterator: Value,
    pub next_method: Value,
    pub callback: Option<Value>,
    pub kind: IteratorHelperKind,
    pub counter: Mutex<BigUint>,
    pub inner_iterator: Mutex<Option<IteratorHelperInner>>,
    pub concat_iterables: Box<[IteratorConcatIterable]>,
    pub concat_index: AtomicUsize,
    pub zip_iterators: Mutex<Box<[Option<IteratorHelperInner>]>>,
    pub zip_open_count: AtomicUsize,
    pub zip_padding: Box<[Value]>,
    pub zip_keys: Box<[PropertyKey]>,
    pub zip_mode: IteratorZipMode,
    /// Exact mathematical remaining count; `None` represents +Infinity.
    pub remaining: Mutex<Option<BigUint>>,
    /// 0 = suspended-start, 1 = executing, 2 = completed, 3 = suspended-yield.
    pub state: AtomicU8,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct RegExpStringIteratorData {
    pub matcher: Value,
    pub string: Arc<str>,
    pub global: bool,
    pub full_unicode: bool,
    pub done: AtomicBool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
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
    pub extensible: AtomicBool,
}

/// A WeakSet holds object members weakly: an unreachable member is dropped
/// during GC. Members must be objects.
pub struct WeakSetData {
    pub items: Mutex<Vec<usize>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

/// A WeakRef target is deliberately omitted from normal GC tracing. Object
/// targets are cleared during sweep; Symbol targets remain available because
/// RuJa's Symbol table is not currently garbage-collected.
pub struct WeakRefData {
    pub target: Mutex<Option<Value>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct FinalizationRegistryCell {
    pub target: Option<Value>,
    pub held_value: Value,
    pub unregister_token: Option<Value>,
}

pub struct FinalizationRegistryData {
    pub cleanup_callback: Value,
    pub cells: Mutex<Vec<FinalizationRegistryCell>>,
    pub cleanup_scheduled: AtomicBool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

pub struct PromiseData {
    pub state: Mutex<PromiseStatus>,
    pub result: Mutex<Value>,
    pub handlers: Mutex<Vec<PromiseHandler>>,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
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
    pub continuation: Option<PromiseContinuation>,
}

pub enum ArrayFromAsyncAwaitKind {
    IteratorNext,
    MappedValue,
    ArrayLikeValue,
    ArrayLikeMappedValue,
    IteratorClose { original_reason: Value },
}

pub struct ArrayFromAsyncContinuation {
    pub capability: PromiseReactionCapability,
    pub realm: GcIdx,
    pub target: Value,
    pub source: Value,
    pub iterator: Value,
    pub next_method: Value,
    pub sync_iterator: bool,
    pub mapper: Value,
    pub this_arg: Value,
    pub index: usize,
    pub length: usize,
    pub await_kind: ArrayFromAsyncAwaitKind,
}

pub enum PromiseContinuation {
    DynamicImport {
        target: std::path::PathBuf,
        capability: PromiseReactionCapability,
        realm: GcIdx,
    },
    AsyncGenerator {
        generator: GcIdx,
        kind: AsyncGeneratorAwaitKind,
    },
    AsyncFromSyncIterator {
        capability: PromiseReactionCapability,
        done: bool,
        iterator: Option<Value>,
        close_on_rejection: bool,
        realm: GcIdx,
    },
    ArrayFromAsync(Box<ArrayFromAsyncContinuation>),
    AsyncFunction(Box<AsyncFunctionContinuation>),
}

#[derive(Clone, Copy)]
pub enum AsyncGeneratorAwaitKind {
    Resume,
    ResumeDelegate,
    ResolveYield,
    ResumeReturn,
    ResumeReturnDelegated,
    ResolveReturn,
}

#[derive(Clone)]
pub enum AsyncGeneratorRequestKind {
    Next(Value),
    Return(Value),
    Throw(Value),
}

#[derive(Clone)]
pub struct AsyncGeneratorRequest {
    pub kind: AsyncGeneratorRequestKind,
    pub capability: PromiseReactionCapability,
}

pub struct AsyncFunctionContinuation {
    pub capability: PromiseReactionCapability,
    pub chunk: Arc<crate::bytecode::Chunk>,
    pub ip: usize,
    pub stack: Vec<Value>,
    pub locals: Vec<Value>,
    pub callee: Value,
    pub env: GcIdx,
    pub catch_stack: Vec<(usize, u32, GcIdx, usize)>,
    pub guard_seq: u32,
    pub this_val: Value,
    pub new_target: Value,
    pub finally_stack: Vec<(usize, u32)>,
    pub finally_completion_tag: u8,
    pub finally_completion_val: Value,
    pub eval_global_bindings: bool,
    pub eval_deletable_bindings: bool,
    pub in_parameter_initializers: bool,
    pub direct_eval_new_target_allowed: bool,
    pub is_derived_ctor: bool,
    pub module_evaluation: bool,
}

pub struct GeneratorData {
    pub function: FunctionExpr,
    pub closure: GcIdx,
    pub state: Mutex<Vec<Value>>,
    pub ip: AtomicUsize,
    pub done: AtomicBool,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
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
    pub catch_stack: Mutex<Vec<(usize, u32, GcIdx, usize)>>,
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
    /// Async generators serialize next/return/throw requests in call order.
    pub async_queue: Mutex<std::collections::VecDeque<AsyncGeneratorRequest>>,
    /// True while the queue front is executing or waiting on an Await job.
    pub async_processing: AtomicBool,
    /// True after AsyncGeneratorYield resolves the current request and the
    /// body is waiting for the next request at the suspended yield point.
    pub async_suspended_yield: AtomicBool,
    /// Which internal `yield*` continuation is waiting on an async iterator
    /// method. The bytecode frame restores this phase after the Promise job.
    pub async_delegate_await_kind: AtomicU8,
    pub props: Mutex<IndexMap<PropertyKey, PropertyDescriptor>>,
    pub proto: Mutex<Option<Value>>,
    pub extensible: AtomicBool,
}

/// State for the lazy `for...in` iterator described by CreateForInIterator.
pub struct ForInIteratorState {
    pub object: Option<Value>,
    pub object_was_visited: bool,
    pub visited_keys: IndexSet<Arc<str>>,
    pub remaining_keys: Vec<Arc<str>>,
    pub remaining_index: usize,
    /// Directed prototype edges persist across `next()` calls so a Proxy cycle
    /// cannot reset its replay budget by yielding one key per pull.
    pub followed_edges: std::collections::HashSet<(usize, usize)>,
    pub rooted_nodes: std::collections::HashSet<usize>,
    /// Values corresponding to `rooted_nodes`; traced by the GC to prevent
    /// heap-slot reuse from changing persistent edge identities.
    pub traversal_roots: Vec<Value>,
    pub proxy_seen: bool,
    pub cycle_replays: usize,
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
    /// Lazy `for...in` mode. Own keys, descriptors, and prototypes are queried
    /// only as iteration advances so Proxy traps run at their observable phase.
    pub for_in: Mutex<Option<ForInIteratorState>>,
    /// True when this iterator is the internal adapter returned by
    /// GetIterator(value, async) after falling back to the sync protocol.
    pub async_from_sync: AtomicBool,
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
            HeapObj::IteratorHelper(i) => &i.props,
            HeapObj::RegExpStringIterator(i) => &i.props,
            HeapObj::WeakMap(w) => &w.props,
            HeapObj::WeakSet(ws) => &ws.props,
            HeapObj::WeakRef(wr) => &wr.props,
            HeapObj::FinalizationRegistry(registry) => &registry.props,
            HeapObj::Promise(p) => &p.props,
            HeapObj::Generator(g) => &g.props,
            HeapObj::LazyGenerator(g) => &g.props,
            HeapObj::Proxy(p) => &p.props,
            HeapObj::ModuleNamespace(n) => &n.props,
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
            HeapObj::IteratorHelper(i) => &i.proto,
            HeapObj::RegExpStringIterator(i) => &i.proto,
            HeapObj::WeakMap(w) => &w.proto,
            HeapObj::WeakSet(ws) => &ws.proto,
            HeapObj::WeakRef(wr) => &wr.proto,
            HeapObj::FinalizationRegistry(registry) => &registry.proto,
            HeapObj::Promise(p) => &p.proto,
            HeapObj::Generator(g) => &g.proto,
            HeapObj::LazyGenerator(g) => &g.proto,
            HeapObj::Proxy(p) => &p.proto,
            HeapObj::ModuleNamespace(n) => &n.proto,
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
                CollectionIteratorKind::WrappedIterator => "Iterator",
                CollectionIteratorKind::StringValues => "String Iterator",
                CollectionIteratorKind::ArrayEntries
                | CollectionIteratorKind::ArrayKeys
                | CollectionIteratorKind::ArrayValues => "Array Iterator",
                CollectionIteratorKind::MapEntries
                | CollectionIteratorKind::MapKeys
                | CollectionIteratorKind::MapValues => "Map Iterator",
                CollectionIteratorKind::SetEntries | CollectionIteratorKind::SetValues => {
                    "Set Iterator"
                }
            },
            HeapObj::IteratorHelper(_) => "Iterator Helper",
            HeapObj::RegExpStringIterator(_) => "RegExp String Iterator",
            HeapObj::WeakMap(_) => "WeakMap",
            HeapObj::WeakSet(_) => "WeakSet",
            HeapObj::WeakRef(_) => "WeakRef",
            HeapObj::FinalizationRegistry(_) => "FinalizationRegistry",
            HeapObj::Promise(_) => "Promise",
            HeapObj::Generator(_) => "Generator",
            HeapObj::LazyGenerator(_) => "Generator",
            HeapObj::Iterator(_) => "Iterator",
            HeapObj::Environment(_) => "Environment",
            HeapObj::Proxy(_) => "Object",
            HeapObj::ModuleNamespace(_) => "Module",
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
            HeapObj::Map(m) => m.extensible.load(Ordering::Relaxed),
            HeapObj::Set(s) => s.extensible.load(Ordering::Relaxed),
            HeapObj::TypedArray(t) => t.extensible.load(Ordering::Relaxed),
            HeapObj::CollectionIterator(iterator) => iterator.extensible.load(Ordering::Relaxed),
            HeapObj::IteratorHelper(iterator) => iterator.extensible.load(Ordering::Relaxed),
            HeapObj::RegExpStringIterator(iterator) => iterator.extensible.load(Ordering::Relaxed),
            HeapObj::WeakMap(map) => map.extensible.load(Ordering::Relaxed),
            HeapObj::WeakSet(set) => set.extensible.load(Ordering::Relaxed),
            HeapObj::WeakRef(wr) => wr.extensible.load(Ordering::Relaxed),
            HeapObj::FinalizationRegistry(registry) => registry.extensible.load(Ordering::Relaxed),
            HeapObj::Promise(promise) => promise.extensible.load(Ordering::Relaxed),
            HeapObj::Generator(generator) => generator.extensible.load(Ordering::Relaxed),
            HeapObj::LazyGenerator(generator) => generator.extensible.load(Ordering::Relaxed),
            HeapObj::ArrayBuffer(buffer) => buffer.extensible.load(Ordering::Relaxed),
            HeapObj::DataView(view) => view.extensible.load(Ordering::Relaxed),
            HeapObj::ModuleNamespace(_) => false,
            _ => true,
        }
    }

    pub fn prevent_extensions(&self) {
        let extensible = match self {
            HeapObj::Object(object) => &object.extensible,
            HeapObj::Array(array) => &array.extensible,
            HeapObj::Function(function) => &function.extensible,
            HeapObj::Map(map) => &map.extensible,
            HeapObj::Set(set) => &set.extensible,
            HeapObj::CollectionIterator(iterator) => &iterator.extensible,
            HeapObj::IteratorHelper(iterator) => &iterator.extensible,
            HeapObj::RegExpStringIterator(iterator) => &iterator.extensible,
            HeapObj::WeakMap(map) => &map.extensible,
            HeapObj::WeakSet(set) => &set.extensible,
            HeapObj::WeakRef(weak_ref) => &weak_ref.extensible,
            HeapObj::FinalizationRegistry(registry) => &registry.extensible,
            HeapObj::Promise(promise) => &promise.extensible,
            HeapObj::Generator(generator) => &generator.extensible,
            HeapObj::LazyGenerator(generator) => &generator.extensible,
            HeapObj::TypedArray(array) => &array.extensible,
            HeapObj::ArrayBuffer(buffer) => &buffer.extensible,
            HeapObj::DataView(view) => &view.extensible,
            _ => return,
        };
        extensible.store(false, Ordering::Relaxed);
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
