use super::*;

/// Sandbox policy: observable array-like call arguments are materialized only
/// up to this many entries, even though ECMAScript's ToLength limit is larger.
pub(crate) const MAX_MATERIALIZED_CALL_ARGUMENTS: usize = 1 << 20;

fn to_length_capped(vm: &mut Vm, value: &Value, max_materialized: usize) -> error::Result<usize> {
    const MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0;

    let number = vm.to_number(value)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0);
    }

    // ToLength truncates before the engine-specific materialization cap is
    // applied. In particular, max + 0.5 is still an admissible length.
    let length = number.trunc().min(MAX_SAFE_LENGTH);
    if length > max_materialized as f64 {
        return Err(Error::range("argument list too large"));
    }
    Ok(length as usize)
}

/// Materialize CreateListFromArrayLike while rooting each observable result.
/// The returned pin count belongs to the caller until the eventual call or
/// construction has captured its result.
pub(crate) fn create_list_from_array_like(
    vm: &mut Vm,
    value: &Value,
    max_materialized: usize,
) -> error::Result<(Vec<Value>, usize)> {
    if !matches!(value, Value::Object(_)) {
        return Err(Error::type_err("argument list must be an object"));
    }

    let length = vm.get_property(value, "length")?;
    let length_pin_count = vm.pin(&length);
    let len = to_length_capped(vm, &length, max_materialized);
    vm.unpin_many(length_pin_count);
    let len = len?;

    let mut list = Vec::with_capacity(len);
    let mut pin_count = 0;
    for index in 0..len {
        let key = PropertyKey::from_integer_index(index as u64);
        let item = match vm.get_property_by_key(value, &key) {
            Ok(item) => item,
            Err(error) => {
                vm.unpin_many(pin_count);
                return Err(error);
            }
        };
        // Earlier values may no longer be reachable through the source when a
        // later indexed getter re-enters JavaScript and triggers collection.
        pin_count += vm.pin(&item);
        list.push(item);
    }
    Ok((list, pin_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_to_length_truncates_before_applying_resource_limit() {
        let mut vm = Vm::new().expect("failed to initialize VM");
        let max = MAX_MATERIALIZED_CALL_ARGUMENTS;

        assert_eq!(
            to_length_capped(&mut vm, &Value::Number(max as f64 + 0.5), max)
                .expect("fractional boundary should truncate to the cap"),
            max
        );
        assert!(to_length_capped(&mut vm, &Value::Number(max as f64 + 1.0), max).is_err());
        assert!(to_length_capped(&mut vm, &Value::Number(f64::INFINITY), max).is_err());
        assert_eq!(
            to_length_capped(&mut vm, &Value::Number(f64::NAN), max)
                .expect("NaN should coerce to zero length"),
            0
        );
    }
}
