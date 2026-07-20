//! Promise microtask, generator, and async runtime helpers split
//! from vm/mod.rs for readability.

use super::property::PropertyTraversal;
use super::*;
use crate::error::{self, Error};
use crate::value::{
    AsyncFunctionContinuation, GcIdx, PromiseReactionCapability, PromiseStatus, Value,
};
use crate::value::{FunctionKind, HeapObj, NativeConstructMode, PropertyKey};
use indexmap::IndexMap;
use parking_lot::Mutex;
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Clone, Copy)]
enum AsyncFromSyncMethod {
    Next,
    Return,
    Throw,
}

enum ConstructorTraversalStep {
    Bound {
        function: GcIdx,
        target: GcIdx,
        bound_len: usize,
        constructable: bool,
    },
    Proxy {
        target: Value,
        handler: Value,
        constructable: bool,
        revoked: bool,
    },
    Function {
        function: GcIdx,
        closure: GcIdx,
        constructable: bool,
    },
    Other,
}

impl Vm {
    pub(crate) fn evaluate_module_chunk_async(
        &mut self,
        chunk: Arc<crate::bytecode::Chunk>,
        env: GcIdx,
    ) -> error::Result<(GcIdx, Option<Value>)> {
        let capability = self.new_intrinsic_promise_capability_in_env(env)?;
        let Value::Object(promise) = capability.promise.clone() else {
            return Err(Error::internal(
                "Module evaluation capability has no Promise",
            ));
        };
        let capability_pins = self.pin_many(&[
            Value::Object(promise),
            capability.resolve.clone(),
            capability.reject.clone(),
        ]);
        let stack_base = self.stack.len();
        let mut frame = CallFrame::new(
            chunk.clone(),
            chunk.body_start_ip,
            stack_base,
            vec![Value::Undefined; 256],
            env,
            Value::Undefined,
        );
        frame.async_mode = true;
        frame.module_evaluation = true;
        self.frames.push(frame);
        let target_depth = self.frames.len() - 1;
        let result = self.interpret_to_depth(target_depth);
        let suspended = self
            .frames
            .get(target_depth)
            .is_some_and(|frame| frame.async_awaiting);
        let (settled, completion) = if suspended {
            (
                self.begin_async_function_await(target_depth, capability),
                None,
            )
        } else {
            let mut completion = None;
            let settled = match result {
                // Module evaluation exposes only completion, never the value
                // of its final expression statement. Assimilating that value
                // can deadlock when it is a Promise that imports this module.
                Ok(value) => {
                    completion = Some(value);
                    self.resolve_promise_capability_value(&capability, Value::Undefined)
                }
                Err(error) if !error.catchable() => {
                    self.frames.truncate(target_depth);
                    self.stack.truncate(stack_base);
                    self.unpin_many(capability_pins);
                    return Err(error);
                }
                Err(error) => self.reject_promise_capability_error(&capability, &error),
            };
            (settled, completion)
        };
        if self.frames.len() > target_depth {
            self.frames.truncate(target_depth);
        }
        self.stack.truncate(stack_base);
        self.unpin_many(capability_pins);
        settled?;
        Ok((promise, completion))
    }

    fn new_intrinsic_promise_capability(&mut self) -> error::Result<PromiseReactionCapability> {
        let constructor = self.current_realm_promise_constructor();
        let capability = crate::builtins::new_promise_capability(self, constructor)?;
        Ok(PromiseReactionCapability {
            promise: capability.promise,
            resolve: capability.resolve,
            reject: capability.reject,
        })
    }

    fn new_intrinsic_promise_capability_in_env(
        &mut self,
        env: GcIdx,
    ) -> error::Result<PromiseReactionCapability> {
        let constructor = self.promise_constructor_for_env(env);
        let capability = crate::builtins::new_promise_capability_in_env(self, constructor, env)?;
        Ok(PromiseReactionCapability {
            promise: capability.promise,
            resolve: capability.resolve,
            reject: capability.reject,
        })
    }

    fn resolve_promise_capability_value(
        &mut self,
        capability: &PromiseReactionCapability,
        value: Value,
    ) -> error::Result<()> {
        self.settle_promise_capability(capability, value, false)
    }

    fn reject_promise_capability_error(
        &mut self,
        capability: &PromiseReactionCapability,
        error: &Arc<Error>,
    ) -> error::Result<()> {
        let env = self.current_realm_global_env();
        self.reject_promise_capability_error_in_env(capability, error, env)
    }

    fn reject_promise_capability_error_in_env(
        &mut self,
        capability: &PromiseReactionCapability,
        error: &Arc<Error>,
        env: GcIdx,
    ) -> error::Result<()> {
        let capability_pins = self.pin_many(&[
            capability.promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
        ]);
        let result = match self.promise_rejection_reason_in_realm(error, env) {
            Ok(reason) => {
                let reason_pin = self.pin(&reason);
                let result = self
                    .call_function(&capability.reject, &[reason], Some(Value::Undefined))
                    .map(|_| ());
                self.unpin(reason_pin);
                result
            }
            Err(error) => Err(error),
        };
        self.unpin_many(capability_pins);
        result
    }

    fn promise_resolve_intrinsic(&mut self, value: Value) -> error::Result<GcIdx> {
        let env = self.current_realm_global_env();
        self.promise_resolve_intrinsic_in_env(value, env)
    }

    pub(crate) fn promise_resolve_intrinsic_in_env(
        &mut self,
        value: Value,
        env: GcIdx,
    ) -> error::Result<GcIdx> {
        let value_pin = self.pin(&value);
        let result = (|| -> error::Result<GcIdx> {
            let native_promise = match &value {
                Value::Object(idx)
                    if self
                        .heap
                        .with_obj(idx.0, |obj| matches!(obj, HeapObj::Promise(_))) =>
                {
                    Some(*idx)
                }
                _ => None,
            };

            if let Some(promise) = native_promise {
                let constructor = self.get_property(&value, "constructor")?;
                if constructor == self.promise_constructor_for_env(env) {
                    return Ok(promise);
                }
            }

            let capability = self.new_intrinsic_promise_capability_in_env(env)?;
            self.resolve_promise_capability_value(&capability, value)?;
            match capability.promise {
                Value::Object(idx) => Ok(idx),
                _ => Err(Error::internal("Promise capability returned non-object")),
            }
        })();
        self.unpin(value_pin);
        result
    }

    pub(crate) fn promise_resolve_for_await(&mut self, value: Value) -> error::Result<GcIdx> {
        let env = self.current_realm_global_env();
        self.promise_resolve_for_await_in_env(value, env)
    }

    pub(crate) fn promise_resolve_for_await_in_env(
        &mut self,
        value: Value,
        env: GcIdx,
    ) -> error::Result<GcIdx> {
        match self.promise_resolve_intrinsic_in_env(value, env) {
            Ok(promise) => Ok(promise),
            Err(error) => self.rejected_promise_for_await_error_in_env(&error, env),
        }
    }

    pub(crate) fn rejected_promise_for_await_error_in_env(
        &mut self,
        error: &Arc<Error>,
        env: GcIdx,
    ) -> error::Result<GcIdx> {
        // Classify host aborts before capability allocation so a resource
        // failure cannot replace them. A thrown JS object is otherwise held
        // only by Arc<Error>, which the GC cannot trace.
        let reason = self.promise_rejection_reason_in_realm(error, env)?;
        let reason_pin = self.pin(&reason);
        let capability = match self.new_intrinsic_promise_capability_in_env(env) {
            Ok(capability) => capability,
            Err(error) => {
                self.unpin(reason_pin);
                return Err(error);
            }
        };
        let rejected = self.settle_promise_capability(&capability, reason, true);
        self.unpin(reason_pin);
        rejected?;
        match capability.promise {
            Value::Object(idx) => Ok(idx),
            _ => Err(Error::internal("Promise capability returned non-object")),
        }
    }

    fn push_async_function_frame(
        &mut self,
        callee: Value,
        fdef: Arc<crate::function::FunctionDef>,
        env: GcIdx,
        this_val: Value,
        args: &[Value],
        new_target: Value,
    ) -> usize {
        let mut locals = vec![Value::Undefined; fdef.num_locals.max(256)];
        for (index, argument) in args.iter().enumerate().take(fdef.params.len()) {
            let slot = fdef.param_slots.get(index).copied().unwrap_or(index);
            if slot < locals.len() {
                locals[slot] = argument.clone();
            }
        }
        let mut frame = CallFrame::new(
            fdef.chunk.clone(),
            0,
            self.stack.len(),
            locals,
            env,
            this_val,
        );
        frame.callee = callee;
        frame.in_parameter_initializers = fdef.has_parameter_expressions;
        frame.new_target = new_target;
        frame.direct_eval_new_target_allowed = !fdef.is_arrow;
        frame.async_mode = true;
        self.frames.push(frame);
        self.frames.len() - 1
    }

    fn capture_async_function_continuation(
        &mut self,
        target_depth: usize,
        capability: PromiseReactionCapability,
    ) -> error::Result<AsyncFunctionContinuation> {
        if self.frames.len() != target_depth + 1 {
            return Err(Error::internal(
                "async function suspension left nested frames active",
            ));
        }
        let mut frame = self
            .frames
            .pop()
            .ok_or_else(|| Error::internal("async function frame missing"))?;
        if !frame.async_awaiting {
            return Err(Error::internal(
                "async function continuation captured without Await",
            ));
        }
        let stack = self.stack.split_off(frame.stack_base);
        Ok(AsyncFunctionContinuation {
            capability,
            chunk: frame.chunk,
            ip: frame.ip,
            stack,
            locals: frame.locals,
            callee: frame.callee,
            env: frame.env,
            catch_stack: frame.catch_stack,
            guard_seq: frame.guard_seq.load(Ordering::Relaxed),
            this_val: frame.this_val,
            new_target: frame.new_target,
            finally_stack: frame.finally_stack,
            finally_completion_tag: frame.finally_completion_tag.load(Ordering::Relaxed),
            finally_completion_val: frame.finally_completion_val.into_inner(),
            eval_global_bindings: frame.eval_global_bindings,
            eval_deletable_bindings: frame.eval_deletable_bindings,
            in_parameter_initializers: frame.in_parameter_initializers,
            direct_eval_new_target_allowed: frame.direct_eval_new_target_allowed,
            is_derived_ctor: frame.is_derived_ctor,
            module_evaluation: frame.module_evaluation,
        })
    }

    fn begin_async_function_await(
        &mut self,
        target_depth: usize,
        capability: PromiseReactionCapability,
    ) -> error::Result<()> {
        let awaited = self
            .frames
            .get(target_depth)
            .and_then(|frame| frame.async_await_value.clone())
            .ok_or_else(|| Error::internal("async Await value missing"))?;
        // PromiseResolve runs while the execution context is still active.
        let promise = self.promise_resolve_for_await(awaited)?;
        let continuation = self.capture_async_function_continuation(target_depth, capability)?;
        let state = self.heap.with_obj(promise.0, |object| {
            if let HeapObj::Promise(data) = object {
                *data.state.lock()
            } else {
                PromiseStatus::Fulfilled
            }
        });
        let handler = crate::value::PromiseHandler {
            on_fulfilled: Value::Undefined,
            on_rejected: Value::Undefined,
            derived: None,
            continuation: Some(crate::value::PromiseContinuation::AsyncFunction(Box::new(
                continuation,
            ))),
        };
        if state == PromiseStatus::Pending {
            self.heap.with_obj(promise.0, |object| {
                if let HeapObj::Promise(data) = object {
                    data.handlers.lock().push(handler);
                }
            });
        } else {
            self.microtask_queue.push_back(Microtask::Then {
                promise,
                on_fulfilled: Value::Undefined,
                on_rejected: Value::Undefined,
                derived: None,
                continuation: handler.continuation,
                realm: None,
            });
        }
        Ok(())
    }

    fn execute_async_function(
        &mut self,
        callee: Value,
        fdef: Arc<crate::function::FunctionDef>,
        env: GcIdx,
        this_val: Value,
        args: &[Value],
        new_target: Value,
    ) -> error::Result<Value> {
        let capability = self.new_intrinsic_promise_capability_in_env(env)?;
        let promise = capability.promise.clone();
        let capability_pins = self.pin_many(&[
            capability.promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
        ]);
        let stack_base = self.stack.len();
        let target_depth =
            self.push_async_function_frame(callee, fdef, env, this_val, args, new_target);
        let result = self.interpret_to_depth(target_depth);
        let suspended = self
            .frames
            .get(target_depth)
            .is_some_and(|frame| frame.async_awaiting);
        let settled = if suspended {
            self.begin_async_function_await(target_depth, capability)
        } else {
            match result {
                Ok(value) => self.resolve_promise_capability_value(&capability, value),
                Err(error) if !error.catchable() => {
                    if self.frames.len() > target_depth {
                        self.frames.truncate(target_depth);
                    }
                    self.unpin_many(capability_pins);
                    return Err(error);
                }
                Err(error) => {
                    let rejected = self.reject_promise_capability_error(&capability, &error);
                    if self.frames.len() > target_depth {
                        self.frames.truncate(target_depth);
                    }
                    rejected
                }
            }
        };
        if suspended && settled.is_err() {
            self.frames.truncate(target_depth);
            self.stack.truncate(stack_base);
        }
        self.unpin_many(capability_pins);
        settled?;
        Ok(promise)
    }

    pub(crate) fn run_async_function_reaction(
        &mut self,
        continuation: AsyncFunctionContinuation,
        promise: GcIdx,
    ) -> error::Result<()> {
        let source_pin = self.pin(&Value::Object(promise));
        let (state, result) = self.heap.with_obj(promise.0, |object| {
            if let HeapObj::Promise(data) = object {
                (*data.state.lock(), data.result.lock().clone())
            } else {
                (PromiseStatus::Fulfilled, Value::Undefined)
            }
        });
        let AsyncFunctionContinuation {
            capability,
            chunk,
            ip,
            stack,
            locals,
            callee,
            env,
            catch_stack,
            guard_seq,
            this_val,
            new_target,
            finally_stack,
            finally_completion_tag,
            finally_completion_val,
            eval_global_bindings,
            eval_deletable_bindings,
            in_parameter_initializers,
            direct_eval_new_target_allowed,
            is_derived_ctor,
            module_evaluation,
        } = continuation;
        let module_path = chunk.source_path.clone();
        let capability_pins = self.pin_many(&[
            capability.promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
            result.clone(),
        ]);
        let caller_stack = std::mem::replace(&mut self.stack, stack);
        let mut frame = CallFrame::new(chunk, ip, 0, locals, env, this_val);
        frame.callee = callee;
        frame.catch_stack = catch_stack;
        frame.guard_seq.store(guard_seq, Ordering::Relaxed);
        frame.new_target = new_target;
        frame.finally_stack = finally_stack;
        frame
            .finally_completion_tag
            .store(finally_completion_tag, Ordering::Relaxed);
        *frame.finally_completion_val.lock() = finally_completion_val;
        frame.eval_global_bindings = eval_global_bindings;
        frame.eval_deletable_bindings = eval_deletable_bindings;
        frame.in_parameter_initializers = in_parameter_initializers;
        frame.direct_eval_new_target_allowed = direct_eval_new_target_allowed;
        frame.is_derived_ctor = is_derived_ctor;
        frame.module_evaluation = module_evaluation;
        frame.async_mode = true;
        if state == PromiseStatus::Rejected {
            *frame.force_throw.lock() = Some(result);
        } else {
            self.stack.push(result);
        }
        self.frames.push(frame);
        let target_depth = self.frames.len() - 1;
        let run_result = self.interpret_to_depth(target_depth);
        let suspended = self
            .frames
            .get(target_depth)
            .is_some_and(|frame| frame.async_awaiting);
        let settled = if suspended {
            self.begin_async_function_await(target_depth, capability)
        } else {
            match run_result {
                Ok(value) if module_evaluation => {
                    if let Some(path) = module_path.as_deref() {
                        self.set_module_completion(path, value);
                    }
                    self.resolve_promise_capability_value(&capability, Value::Undefined)
                }
                Ok(value) => self.resolve_promise_capability_value(&capability, value),
                Err(error) if !error.catchable() => {
                    if module_evaluation {
                        if let Some(path) = module_path.as_deref() {
                            self.mark_module_evaluation_aborted(path, error.clone());
                        }
                    }
                    if self.frames.len() > target_depth {
                        self.frames.truncate(target_depth);
                    }
                    self.stack = caller_stack;
                    self.unpin_many(capability_pins);
                    self.unpin(source_pin);
                    return Err(error);
                }
                Err(error) => {
                    let rejected = self.reject_promise_capability_error(&capability, &error);
                    if self.frames.len() > target_depth {
                        self.frames.truncate(target_depth);
                    }
                    rejected
                }
            }
        };
        if suspended {
            if let Err(error) = &settled {
                if module_evaluation && !error.catchable() {
                    if let Some(path) = module_path.as_deref() {
                        self.mark_module_evaluation_aborted(path, error.clone());
                    }
                }
                self.frames.truncate(target_depth);
            }
        }
        self.stack = caller_stack;
        self.unpin_many(capability_pins);
        self.unpin(source_pin);
        settled
    }

    pub(crate) fn run_async_from_sync_iterator_reaction(
        &mut self,
        capability: PromiseReactionCapability,
        done: bool,
        iterator: Option<Value>,
        close_on_rejection: bool,
        realm: GcIdx,
        promise: GcIdx,
    ) -> error::Result<()> {
        let (state, result) = self.heap.with_obj(promise.0, |object| {
            if let HeapObj::Promise(data) = object {
                (*data.state.lock(), data.result.lock().clone())
            } else {
                (PromiseStatus::Fulfilled, Value::Undefined)
            }
        });
        let pins = self.pin_many(&[
            Value::Object(promise),
            capability.promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
            result.clone(),
            iterator.clone().unwrap_or(Value::Undefined),
        ]);
        let settled = if state == PromiseStatus::Rejected {
            if close_on_rejection {
                if let Some(iterator) = &iterator {
                    if let Err(error) = self.iterator_close(iterator) {
                        if !error.catchable() {
                            self.unpin_many(pins);
                            return Err(error);
                        }
                    }
                }
            }
            self.settle_promise_capability(&capability, result, true)
        } else {
            match crate::builtins::regexp::gen_result_in_env(self, result, done, false, realm) {
                Ok(iterator_result) => {
                    self.settle_promise_capability(&capability, iterator_result, false)
                }
                Err(error) => self.reject_promise_capability_error(&capability, &error),
            }
        };
        self.unpin_many(pins);
        settled
    }

    pub(crate) fn run_thenable_job(
        &mut self,
        thenable: Value,
        then: Value,
        resolve: Value,
        reject: Value,
        realm: GcIdx,
    ) -> error::Result<()> {
        let pins = self.pin_many(&[
            thenable.clone(),
            then.clone(),
            resolve.clone(),
            reject.clone(),
        ]);
        let call_result = self.call_function(&then, &[resolve, reject.clone()], Some(thenable));
        let result = if let Err(error) = call_result {
            match self.promise_rejection_reason_in_realm(&error, realm) {
                Ok(reason) => {
                    let reason_pin = self.pin(&reason);
                    let result = self
                        .call_function(&reject, &[reason], Some(Value::Undefined))
                        .map(|_| ());
                    self.unpin(reason_pin);
                    result
                }
                Err(error) => Err(error),
            }
        } else {
            Ok(())
        };
        self.unpin_many(pins);
        result
    }

    pub(crate) fn run_then(
        &mut self,
        promise: GcIdx,
        on_fulfilled: Value,
        on_rejected: Value,
        derived: Option<PromiseReactionCapability>,
        realm: Option<GcIdx>,
    ) -> error::Result<()> {
        let (state, result) = self.heap.with_obj(promise.0, |o| {
            if let HeapObj::Promise(p) = o {
                (*p.state.lock(), p.result.lock().clone())
            } else {
                (PromiseStatus::Fulfilled, Value::Undefined)
            }
        });
        let handler = if state == PromiseStatus::Rejected {
            on_rejected
        } else {
            on_fulfilled
        };
        if !crate::builtins::is_callable(&handler, &self.heap) {
            // pass-through: settle the derived promise with the same outcome
            if let Some(capability) = &derived {
                if state == PromiseStatus::Rejected {
                    self.settle_promise_reaction_capability(capability, result, true)?;
                } else {
                    self.settle_promise_reaction_capability(capability, result, false)?;
                }
            }
            return Ok(());
        }
        // Pin the source promise, handler, result, and derived promise as GC roots while the
        // handler call runs: call_function may allocate enough to trigger a GC,
        // which would otherwise collect these values held only in Rust locals.
        let mut roots = vec![Value::Object(promise), handler.clone(), result.clone()];
        if let Some(capability) = &derived {
            roots.push(capability.promise.clone());
            roots.push(capability.resolve.clone());
            roots.push(capability.reject.clone());
        }
        let pinned = self.pin_many(&roots);
        let handler_realm = realm.unwrap_or_else(|| self.current_realm_global_env());
        // call the handler with the result
        let call_ret = self.call_function(&handler, std::slice::from_ref(&result), None);
        let outcome = match call_ret {
            Ok(ret) => {
                if let Some(capability) = &derived {
                    // PromiseReactionJob always calls the capability's resolve
                    // function. That path performs self-resolution checks and
                    // observes an overridden `then` even for native Promises.
                    self.settle_promise_reaction_capability(capability, ret, false)
                } else {
                    Ok(())
                }
            }
            Err(error) if !error.catchable() => Err(error),
            Err(error) => {
                if let Some(capability) = &derived {
                    match self.promise_rejection_reason_in_realm(&error, handler_realm) {
                        Ok(reason) => {
                            self.settle_promise_reaction_capability(capability, reason, true)
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    Ok(())
                }
            }
        };
        // Error materialization and capability settlement can allocate, so the
        // reaction roots stay pinned through the whole outcome conversion.
        self.unpin_many(pinned);
        outcome
    }

    fn settle_promise_reaction_capability(
        &mut self,
        capability: &PromiseReactionCapability,
        value: Value,
        rejected: bool,
    ) -> error::Result<()> {
        // Never replay an arbitrary species-provided capability function.
        // Intrinsic resolving functions preserve their own post-call stage.
        self.settle_promise_capability(capability, value, rejected)
    }

    pub(super) fn settle_promise_capability(
        &mut self,
        capability: &PromiseReactionCapability,
        value: Value,
        rejected: bool,
    ) -> error::Result<()> {
        let function = if rejected {
            capability.reject.clone()
        } else {
            capability.resolve.clone()
        };
        let pins = self.pin_many(&[
            capability.promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
            function.clone(),
            value.clone(),
        ]);
        let result = self.call_function(
            &function,
            std::slice::from_ref(&value),
            Some(Value::Undefined),
        );
        self.unpin_many(pins);
        result.map(|_| ())
    }

    pub fn new_object(&mut self) -> error::Result<GcIdx> {
        self.new_object_in_env(self.global)
    }

    pub(crate) fn new_object_in_env(&mut self, env: GcIdx) -> error::Result<GcIdx> {
        let prototype = self.object_prototype_for_env(env);
        let obj = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(prototype)),
            extensible: std::sync::atomic::AtomicBool::new(true),
            class_name: None,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        self.alloc(obj)
    }

    pub(crate) fn new_object_in_current_realm(&mut self) -> error::Result<GcIdx> {
        self.new_object_in_env(self.current_realm_global_env())
    }

    /// Allocate a heap object, returning a catchable `RangeError` if the
    /// heap limit is exceeded. All heap allocations must go through this
    /// method so the limit is enforced uniformly.
    pub(crate) fn alloc(&mut self, obj: HeapObj) -> error::Result<GcIdx> {
        Ok(self.try_alloc(obj)?)
    }

    /// Preserve the typed heap-limit failure for callers that have a bounded
    /// preallocated fallback, while sharing the ordinary rooted GC retry.
    pub(crate) fn try_alloc(
        &mut self,
        obj: HeapObj,
    ) -> Result<GcIdx, crate::gc::HeapLimitExceeded> {
        // If a heap limit is set, try collecting first to free up space.
        let max = self.max_heap_objects;
        if max > 0 && self.heap.live_count() >= max {
            self.heap.collect(&self.collect_roots());
            self.ic.clear();
            self.schedule_finalization_cleanup_jobs();
        }
        Ok(GcIdx(self.heap.allocate(obj)?))
    }

    /// Set the wrapped primitive on an object (for `new Number(5)`,
    /// `new Boolean(true)`, `new String("x")`, `Object(1n)`). `valueOf()` and
    /// `ToPrimitive` consult this so `new Number(5) + 1 === 6`.
    pub fn set_primitive(&mut self, obj: &Value, prim: Value) {
        if let Value::Object(idx) = obj {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Object(od) = o {
                    *od.primitive.lock() = Some(prim);
                }
            });
        }
    }

    /// Allocate a function with native impl.
    /// Register a Rust function as a global JS function. The function is
    /// callable from JS by `name` with the given `length` (arity).
    ///
    /// ```no_run
    /// use ruja::{Vm, Value};
    ///
    /// let mut vm = Vm::new().expect("init");
    /// vm.register_fn("double", |vm, args, _| {
    ///     let n = vm.to_number(&args[0])?;
    ///     Ok(Value::Number(n * 2.0))
    /// }, 1).unwrap();
    /// vm.run("double(21);").unwrap(); // -> 42
    /// ```
    pub fn register_fn(&mut self, name: &str, func: NativeFn, length: usize) -> error::Result<()> {
        let idx = self.new_native_function(name, func, length)?;
        crate::builtins::define_global(self, name, Value::Object(idx));
        Ok(())
    }

    pub fn new_native_function(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
    ) -> error::Result<GcIdx> {
        self.new_native_function_in_env(name, func, length, self.global)
    }

    pub(crate) fn new_native_function_in_env(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
        closure: GcIdx,
    ) -> error::Result<GcIdx> {
        self.new_native_function_with_construct_mode_in_env(name, func, length, closure, None)
    }

    /// Runtime intrinsic installers can use this after pinning every earlier
    /// provisional function in the same allocation batch.
    pub(crate) fn new_native_function_in_env_with_gc_retry(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
        closure: GcIdx,
    ) -> error::Result<GcIdx> {
        let function = self.native_function_object(name, func, length, closure, None);
        self.alloc(function)
    }

    pub(crate) fn new_native_constructor(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
        construct_mode: NativeConstructMode,
    ) -> error::Result<GcIdx> {
        self.new_native_constructor_in_env(name, func, length, self.global, construct_mode)
    }

    pub(crate) fn new_native_constructor_in_env(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
        closure: GcIdx,
        construct_mode: NativeConstructMode,
    ) -> error::Result<GcIdx> {
        self.new_native_function_with_construct_mode_in_env(
            name,
            func,
            length,
            closure,
            Some(construct_mode),
        )
    }

    fn new_native_function_with_construct_mode_in_env(
        &mut self,
        name: &str,
        func: NativeFn,
        length: usize,
        closure: GcIdx,
        construct_mode: Option<NativeConstructMode>,
    ) -> error::Result<GcIdx> {
        let function = self.native_function_object(name, func, length, closure, construct_mode);
        Ok(GcIdx(self.heap.allocate(function)?))
    }

    fn native_function_object(
        &self,
        name: &str,
        func: NativeFn,
        length: usize,
        closure: GcIdx,
        construct_mode: Option<NativeConstructMode>,
    ) -> HeapObj {
        let realm = crate::environment::global_env_root(&self.heap, closure);
        let function_proto = self
            .realm_function_prototypes
            .get(&realm.0)
            .cloned()
            .unwrap_or_else(|| self.function_proto.clone());
        let mut props = IndexMap::new();
        let mut len_desc = crate::value::PropertyDescriptor::data(Value::Number(length as f64));
        len_desc.writable = false;
        len_desc.enumerable = false;
        len_desc.configurable = true;
        props.insert(crate::value::PropertyKey::from("length"), len_desc);
        let mut name_desc = crate::value::PropertyDescriptor::data(Value::String(Arc::from(name)));
        name_desc.writable = false;
        name_desc.enumerable = false;
        name_desc.configurable = true;
        props.insert(crate::value::PropertyKey::from("name"), name_desc);

        let fdef = crate::value::FunctionData {
            name: Some(Arc::from(name)),
            kind: crate::value::FunctionKind::Native {
                func,
                length,
                construct_mode,
            },
            closure,
            lexical_new_target: Value::Undefined,
            home_object: Mutex::new(None),
            is_class_ctor: std::sync::atomic::AtomicBool::new(false),
            // Constructor installers attach an instance prototype separately;
            // [[Construct]] itself is represented by `construct_mode`.
            // Every native function's own [[Prototype]] (`__proto__`) is
            // `Function.prototype` once it has been allocated.
            prototype: Mutex::new(None),
            proto: Mutex::new(match function_proto {
                Value::Object(_) => Some(function_proto),
                _ => None,
            }),
            props: Mutex::new(props),
            extensible: std::sync::atomic::AtomicBool::new(true),
            private_fields: Mutex::new(std::collections::HashMap::new()),
        };
        HeapObj::Function(fdef)
    }

    pub(crate) fn native_callee_closure(&self) -> Option<GcIdx> {
        match &self.execution_contexts.last()?.kind {
            ExecutionContextKind::Native { .. } => Some(self.execution_contexts.last()?.realm_env),
            ExecutionContextKind::Interpreted { .. } => None,
        }
    }

    fn constructor_traversal_step(&self, value: &Value) -> ConstructorTraversalStep {
        let Value::Object(index) = value else {
            return ConstructorTraversalStep::Other;
        };
        self.heap.with_obj(index.0, |object| match object {
            HeapObj::Function(function) => match &function.kind {
                FunctionKind::Bound {
                    target,
                    bound_args,
                    constructable,
                    ..
                } => ConstructorTraversalStep::Bound {
                    function: *index,
                    target: *target,
                    bound_len: bound_args.len(),
                    constructable: *constructable,
                },
                FunctionKind::Interpreted { func } => ConstructorTraversalStep::Function {
                    function: *index,
                    closure: function.closure,
                    constructable: !func.is_arrow
                        && !func.is_method
                        && !func.is_async
                        && !func.is_generator,
                },
                FunctionKind::Native { construct_mode, .. } => ConstructorTraversalStep::Function {
                    function: *index,
                    closure: function.closure,
                    constructable: construct_mode.is_some(),
                },
            },
            HeapObj::Proxy(proxy) => ConstructorTraversalStep::Proxy {
                target: proxy.target.clone(),
                handler: proxy.handler.clone(),
                constructable: proxy.constructable,
                revoked: *proxy.revoked.lock(),
            },
            _ => ConstructorTraversalStep::Other,
        })
    }

    pub(crate) fn constructor_realm(&mut self, constructor: &Value) -> error::Result<GcIdx> {
        let mut current = constructor.clone();
        loop {
            match self.constructor_traversal_step(&current) {
                ConstructorTraversalStep::Bound { target, .. } => {
                    self.consume_fuel()?;
                    current = Value::Object(target);
                }
                ConstructorTraversalStep::Proxy {
                    target, revoked, ..
                } => {
                    if revoked {
                        return Err(Error::type_err("Cannot get Realm of a revoked Proxy"));
                    }
                    self.consume_fuel()?;
                    current = target;
                }
                ConstructorTraversalStep::Function { closure, .. } => {
                    return Ok(crate::environment::global_env_root(&self.heap, closure));
                }
                ConstructorTraversalStep::Other => {
                    return Err(Error::type_err("constructor has no Realm"));
                }
            }
        }
    }

    pub(crate) fn constructor_realm_or_fallback(
        &mut self,
        constructor: &Value,
        fallback: GcIdx,
    ) -> error::Result<GcIdx> {
        match self.constructor_realm(constructor) {
            Ok(realm) => Ok(realm),
            Err(error) if !error.catchable() => Err(error),
            Err(_) => Ok(fallback),
        }
    }

    pub(crate) fn promise_reaction_job_realm(
        &mut self,
        handler: &Value,
    ) -> error::Result<Option<GcIdx>> {
        let fallback = self.current_realm_global_env();
        self.promise_reaction_job_realm_with_fallback(handler, fallback)
    }

    pub(crate) fn promise_reaction_job_realm_with_fallback(
        &mut self,
        handler: &Value,
        fallback: GcIdx,
    ) -> error::Result<Option<GcIdx>> {
        if !crate::builtins::is_callable(handler, &self.heap) {
            return Ok(None);
        }
        self.constructor_realm_or_fallback(handler, fallback)
            .map(Some)
    }

    pub(crate) fn constructor_realm_default_prototype(
        &mut self,
        constructor: &Value,
        intrinsic: &str,
        fallback: Value,
    ) -> error::Result<Value> {
        let realm = self.constructor_realm(constructor)?;
        self.realm_default_prototype(realm, intrinsic, fallback)
    }

    pub(crate) fn realm_default_prototype(
        &mut self,
        realm: GcIdx,
        intrinsic: &str,
        fallback: Value,
    ) -> error::Result<Value> {
        if intrinsic == "Object" {
            return Ok(self
                .realm_object_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        let primitive_kind = match intrinsic {
            "String" => Some(PrimitivePrototypeKind::String),
            "Number" => Some(PrimitivePrototypeKind::Number),
            "Boolean" => Some(PrimitivePrototypeKind::Boolean),
            _ => None,
        };
        if let Some(kind) = primitive_kind {
            return Ok(self
                .realm_primitive_prototypes
                .get(&(realm.0, kind))
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "Date" {
            return Ok(self
                .realm_date_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "RegExp" {
            return Ok(self
                .realm_regexp_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "Promise" {
            return Ok(self
                .realm_promise_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "Function" {
            return Ok(self
                .realm_function_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "AsyncFunction" {
            return Ok(self
                .realm_async_function_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "GeneratorFunction" {
            return Ok(self
                .realm_generator_function_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        if intrinsic == "AsyncGeneratorFunction" {
            return Ok(self
                .realm_async_generator_function_prototypes
                .get(&realm.0)
                .cloned()
                .unwrap_or(fallback));
        }
        let Some(intrinsic_ctor) = env::get(&self.heap, realm, intrinsic) else {
            return Ok(fallback);
        };
        let proto = self.get_property_by_key(&intrinsic_ctor, &PropertyKey::from("prototype"))?;
        if matches!(proto, Value::Object(_)) {
            Ok(proto)
        } else {
            Ok(fallback)
        }
    }

    pub(crate) fn is_constructor_value(&self, value: &Value) -> bool {
        let Value::Object(index) = value else {
            return false;
        };
        self.heap.with_obj(index.0, |object| match object {
            HeapObj::Function(function) => match &function.kind {
                FunctionKind::Interpreted { func } => {
                    !func.is_arrow && !func.is_method && !func.is_async && !func.is_generator
                }
                FunctionKind::Native { construct_mode, .. } => construct_mode.is_some(),
                FunctionKind::Bound { constructable, .. } => *constructable,
            },
            HeapObj::Proxy(proxy) => proxy.constructable,
            _ => false,
        })
    }

    /// Define a global binding (visible to JS as a top-level variable).
    /// This is the embedding API for exposing host values to script code.
    pub fn define_global(&mut self, name: &str, value: Value) {
        crate::environment::declare(
            &self.heap,
            self.global,
            name,
            value,
            crate::value::BindingKind::Var,
        );
    }

    /// Get a global binding by name, or `undefined` if not present.
    pub fn get_global(&self, name: &str) -> Value {
        crate::environment::get(&self.heap, self.global, name).unwrap_or(Value::Undefined)
    }

    /// Minimal stub for `Object(value)` coercion.
    pub fn to_object(&mut self, value: &Value) -> error::Result<Value> {
        Ok(match value {
            Value::Object(idx) => Value::Object(*idx),
            _ => {
                let idx = self.new_object()?;
                let obj = Value::Object(idx);
                self.set_primitive(&obj, value.clone());
                let proto = self.current_realm_primitive_prototype(value);
                if !proto.is_undefined() {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Object(od) = o {
                            *od.proto.lock() = Some(proto);
                        }
                    });
                }
                Value::Object(idx)
            }
        })
    }
}

impl Vm {
    /// Call a function value with the given arguments and `this` binding.
    pub fn call_function(
        &mut self,
        func: &Value,
        args: &[Value],
        this: Option<Value>,
    ) -> error::Result<Value> {
        // Pin the callee, args, and receiver as GC roots for the duration of this call:
        // reading the function kind and building the call frame involve heap
        // allocations that can trigger a GC, which would otherwise collect
        // values held only in the caller's Rust locals / args slice.
        let mut pin_count = {
            let mut n = self.pin(func);
            for a in args {
                n += self.pin(a);
            }
            if let Some(this_value) = &this {
                n += self.pin(this_value);
            }
            n
        };
        let result = self.call_function_inner(func, args, this, &mut pin_count);
        self.unpin_many(pin_count);
        result
    }

    pub(crate) fn call_function_inner(
        &mut self,
        func: &Value,
        args: &[Value],
        this: Option<Value>,
        pin_count: &mut usize,
    ) -> error::Result<Value> {
        // Cap the call-stack depth before pushing another frame. Without this
        // an unbounded JS recursion would overflow the Rust stack (each JS
        // call recurses through `call_function` -> `execute_chunk_func` ->
        // `interpret_to_depth`), killing the process with a hard stack
        // overflow instead of a catchable RangeError. Keep the limit
        // conservative for debug builds on small CI stacks while still
        // allowing ordinary recursive code to run.
        const MAX_CALL_STACK_DEPTH: usize = 512;
        if self.frames.len() >= MAX_CALL_STACK_DEPTH {
            return Err(Error::range("Maximum call stack size exceeded"));
        }
        if !crate::builtins::is_callable(func, &self.heap) {
            return Err(Error::type_err(format!(
                "{} is not a function",
                func.type_of()
            )));
        }

        // Both transparent forwarding and calling a Proxy-valued apply trap
        // are tail operations, so one rooted state machine avoids native recursion.
        let mut active_func = func.clone();
        let mut active_args = Cow::Borrowed(args);
        let mut active_this = this;
        let idx = loop {
            let Value::Object(idx) = &active_func else {
                unreachable!("callable metadata must resolve to an object")
            };
            let idx = *idx;
            let proxy_call = self.heap.with_obj(idx.0, |object| {
                let HeapObj::Proxy(proxy) = object else {
                    return None;
                };
                if *proxy.revoked.lock() {
                    return Some(Err(Error::type_err(
                        "Cannot perform 'apply' on a proxy that has been revoked",
                    )));
                }
                Some(Ok((proxy.target.clone(), proxy.handler.clone())))
            });
            let Some(proxy_call) = proxy_call else {
                break idx;
            };

            let (target, handler) = proxy_call?;
            self.consume_fuel()?;
            let proxy_pins = self.pin_many(&[target.clone(), handler.clone()]);
            let trap = match self.get_proxy_method(&handler, "apply") {
                Ok(trap) => trap,
                Err(error) => {
                    self.unpin_many(proxy_pins);
                    return Err(error);
                }
            };
            if trap.is_nullish() {
                self.unpin_many(proxy_pins);
                active_func = target;
                continue;
            }
            if !crate::builtins::is_callable(&trap, &self.heap) {
                self.unpin_many(proxy_pins);
                return Err(Error::type_err("Proxy apply trap is not callable"));
            }

            let trap_pin = self.pin(&trap);
            let arg_array = match crate::builtins::make_value_array_in_current_realm(
                self,
                active_args.as_ref().to_vec(),
            ) {
                Ok(arg_array) => arg_array,
                Err(error) => {
                    self.unpin(trap_pin);
                    self.unpin_many(proxy_pins);
                    return Err(error);
                }
            };
            let arg_array_pin = self.pin(&arg_array);
            *pin_count += proxy_pins + trap_pin + arg_array_pin;
            let this_arg = active_this.take().unwrap_or(Value::Undefined);
            active_func = trap;
            active_args = Cow::Owned(vec![target, this_arg, arg_array]);
            active_this = Some(handler);
        };
        let args = active_args.as_ref();
        let this = active_this;

        // read function kind without holding borrow
        let kind_info = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                match &f.kind {
                    crate::value::FunctionKind::Native { func, .. } => Some(FuncCallInfo::Native {
                        func: *func,
                        closure: f.closure,
                    }),
                    crate::value::FunctionKind::Interpreted { func } => {
                        Some(FuncCallInfo::Interpreted {
                            func: func.clone(),
                            closure: f.closure,
                            lexical_new_target: f.lexical_new_target.clone(),
                            home_object: f.home_object.lock().clone(),
                            is_class_ctor: f
                                .is_class_ctor
                                .load(std::sync::atomic::Ordering::Relaxed),
                            is_arrow: func.is_arrow,
                            is_async: func.is_async,
                        })
                    }
                    crate::value::FunctionKind::Bound {
                        target,
                        this_val,
                        bound_args,
                        ..
                    } => Some(FuncCallInfo::Bound {
                        target: *target,
                        this_val: this_val.clone(),
                        bound_args: bound_args.clone(),
                    }),
                }
            } else {
                None
            }
        });
        match kind_info {
            Some(FuncCallInfo::Native { func: f, closure }) => {
                let context = ExecutionContext {
                    realm_env: closure,
                    kind: ExecutionContextKind::Native {
                        callee: Value::Object(idx),
                        new_target: self.pending_new_target.take(),
                        new_target_prototype: self.pending_new_target_prototype.take(),
                    },
                };
                self.with_execution_context(context, |vm| match f(vm, args, this) {
                    Err(err) if err.catchable() && err.thrown_value.is_none() => {
                        match vm.make_error_value(&err) {
                            Ok(thrown) => Err(Error::thrown(thrown, &vm.heap)),
                            Err(err) => Err(err),
                        }
                    }
                    result => result,
                })
            }
            Some(FuncCallInfo::Interpreted {
                func,
                closure,
                is_arrow,
                is_async,
                is_class_ctor,
                lexical_new_target,
                home_object,
            }) => {
                let context_depth = self.execution_contexts.len();
                self.execution_contexts.push(ExecutionContext {
                    realm_env: closure,
                    kind: ExecutionContextKind::Interpreted {
                        callee: Value::Object(idx),
                    },
                });
                // The interpreted context must exist before call setup because
                // sloppy this conversion and arguments/rest allocation use the
                // callee Realm before its bytecode frame is pushed.
                if is_class_ctor && self.pending_new_target.is_none() {
                    let error = self.materialize_current_interpreted_error(Error::type_err(
                        "Class constructor cannot be invoked without 'new'",
                    ));
                    self.execution_contexts.truncate(context_depth);
                    return Err(error);
                }
                let call_env = match env::new_env(&self.heap, Some(closure), true) {
                    Ok(call_env) => call_env,
                    Err(error) => {
                        let error = self.materialize_current_interpreted_error(error.into());
                        self.execution_contexts.truncate(context_depth);
                        return Err(error);
                    }
                };
                let call_env_pin_count = self.pin(&Value::Object(call_env));
                let call_result = (|| {
                    // Declare every parameter binding as *uninitialized* (TDZ). The raw
                    // argument still lives in `locals[i]`, which the compiled
                    // parameter prologue reads via `LoadLocal`; the binding is only
                    // lifted by `InitLet` once the prologue applies the raw value or
                    // the default, left-to-right. This makes
                    // `function f(a = b, b = 2)` a ReferenceError: when `a`'s
                    // default evaluates, `b` is still in the TDZ.
                    for param in func.params.iter() {
                        env::declare_uninit(
                            &self.heap,
                            call_env,
                            param,
                            crate::value::BindingKind::Param,
                        );
                    }
                    // rest parameter: collect remaining args into an array.
                    if let Some(rest_name) = &func.rest_param {
                        let rest: Vec<Value> = if func.params.len() <= args.len() {
                            args[func.params.len()..].to_vec()
                        } else {
                            Vec::new()
                        };
                        let arr = HeapObj::Array(crate::value::ArrayData::new(
                            rest,
                            Some(self.array_prototype_for_env(call_env)),
                        ));
                        env::declare(
                            &self.heap,
                            call_env,
                            rest_name,
                            Value::Object(GcIdx(self.heap.allocate(arr)?)),
                            crate::value::BindingKind::Param,
                        );
                    }
                    if !is_arrow {
                        let realm = env::global_env_root(&self.heap, call_env);
                        let arguments_iterator = self
                            .realm_array_values_functions
                            .get(&realm.0)
                            .cloned()
                            .ok_or_else(|| {
                                Error::internal("missing Array values intrinsic for arguments")
                            })?;
                        let arguments_prototype = self.object_prototype_for_env(call_env);
                        let mapped_arguments =
                            !func.chunk.is_strict && !func.has_parameter_expressions;
                        let restricted_callee = if mapped_arguments {
                            None
                        } else {
                            Some(crate::builtins::throw_type_error_intrinsic(self, closure)?)
                        };
                        let mut arguments_pin_count = self.pin_many(args);
                        arguments_pin_count += self.pin(&arguments_iterator);
                        arguments_pin_count += self.pin(&arguments_prototype);
                        if let Some(thrower) = &restricted_callee {
                            arguments_pin_count += self.pin(thrower);
                        }
                        let mut arg_array =
                            crate::value::ArrayData::new(args.to_vec(), Some(arguments_prototype));
                        arg_array
                            .is_arguments
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        if mapped_arguments {
                            let mut seen = std::collections::HashSet::new();
                            let mut names = vec![None; func.params.len()];
                            for (i, name) in func.params.iter().enumerate().rev() {
                                if i < args.len() && seen.insert(name.clone()) {
                                    names[i] = Some(name.clone());
                                }
                            }
                            arg_array.arguments_map =
                                Mutex::new(Some(crate::value::ArgumentsMap {
                                    env: call_env,
                                    names,
                                }));
                        }
                        let arr = HeapObj::Array(arg_array);
                        let arg_idx = match self.alloc(arr) {
                            Ok(index) => index,
                            Err(error) => {
                                self.unpin_many(arguments_pin_count);
                                return Err(error);
                            }
                        };
                        let arguments = Value::Object(arg_idx);
                        arguments_pin_count += self.pin(&arguments);
                        self.heap.with_obj(arg_idx.0, |obj| {
                            if let HeapObj::Array(a) = obj {
                                let mut props = a.props.lock();
                                let mut length_desc = crate::value::PropertyDescriptor::data(
                                    Value::Number(args.len() as f64),
                                );
                                length_desc.writable = true;
                                length_desc.enumerable = false;
                                length_desc.configurable = true;
                                props
                                    .insert(crate::value::PropertyKey::from("length"), length_desc);
                                let mut iterator_desc = crate::value::PropertyDescriptor::data(
                                    arguments_iterator.clone(),
                                );
                                iterator_desc.enumerable = false;
                                props.insert(
                                    crate::value::PropertyKey::Symbol(
                                        self.well_known_symbols.iterator,
                                    ),
                                    iterator_desc,
                                );
                            }
                        });
                        if !mapped_arguments {
                            let thrower = restricted_callee
                                .expect("unmapped arguments should retain a restricted callee");
                            self.heap.with_obj(arg_idx.0, |obj| {
                                if let HeapObj::Array(a) = obj {
                                    a.props.lock().insert(
                                        crate::value::PropertyKey::from("callee"),
                                        crate::builtins::restricted_throw_type_error_accessor(
                                            thrower,
                                        ),
                                    );
                                }
                            });
                        } else {
                            // In non-strict mode, arguments has a `callee` property
                            // pointing to the executing function.
                            self.heap.with_obj(arg_idx.0, |obj| {
                                if let HeapObj::Array(a) = obj {
                                    let mut props = a.props.lock();
                                    let mut callee_desc =
                                        crate::value::PropertyDescriptor::data(Value::Object(idx));
                                    callee_desc.writable = true;
                                    callee_desc.enumerable = false;
                                    callee_desc.configurable = true;
                                    props.insert(
                                        crate::value::PropertyKey::from("callee"),
                                        callee_desc,
                                    );
                                }
                            });
                        }
                        env::declare(
                            &self.heap,
                            call_env,
                            "arguments",
                            arguments,
                            crate::value::BindingKind::Var,
                        );
                        self.unpin_many(arguments_pin_count);
                    }
                    // In sloppy (non-strict) mode, an unbound `this` (plain
                    // function call with no receiver) defaults to the global
                    // object. In strict mode it stays `undefined`. Arrow functions
                    // ignore `this` entirely (lexical capture).
                    let this_val = if is_arrow {
                        this.unwrap_or(Value::Undefined)
                    } else {
                        let raw = this.unwrap_or(Value::Undefined);
                        if !func.chunk.is_strict {
                            if raw.is_nullish() {
                                self.realm_global_for_env(call_env)
                            } else {
                                self.to_object(&raw)?
                            }
                        } else {
                            raw
                        }
                    };
                    // Arrow functions capture `this` lexically from their
                    // enclosing scope, so they must NOT redeclare `this` in
                    // their own call environment (which would shadow the
                    // captured binding). Non-arrow functions bind `this` to the
                    // caller-supplied value (or `undefined`).
                    if !is_arrow {
                        // Derived class constructors leave `this` in the TDZ
                        // until `super()` initializes it.
                        if func.is_derived {
                            env::declare_uninit(
                                &self.heap,
                                call_env,
                                "this",
                                crate::value::BindingKind::Const,
                            );
                        } else {
                            env::declare(
                                &self.heap,
                                call_env,
                                "this",
                                this_val.clone(),
                                crate::value::BindingKind::Const,
                            );
                        }
                    }
                    // Object literal methods carry their [[HomeObject]] on the
                    // function. Class methods already have #super bound by the
                    // compiler, but a local binding must still shadow an outer
                    // method's #super when methods are nested.
                    if func.is_method && !is_arrow {
                        let has_super = crate::environment::has(&self.heap, call_env, "#super");
                        if home_object.is_some() || !has_super {
                            env::declare(
                                &self.heap,
                                call_env,
                                "#super",
                                home_object.clone().unwrap_or_else(|| this_val.clone()),
                                crate::value::BindingKind::Const,
                            );
                        }
                    }
                    let _ = &this_val;
                    let is_gen = func.is_generator;
                    if is_gen {
                        let prologue = self.execute_generator_prologue(
                            func.clone(),
                            call_env,
                            this_val.clone(),
                            args,
                        )?;
                        let generator_instance_proto = {
                            let callee = Value::Object(idx);
                            let proto = self
                                .get_property_by_key(
                                    &callee,
                                    &crate::value::PropertyKey::from("prototype"),
                                )
                                .unwrap_or(Value::Undefined);
                            if matches!(proto, Value::Object(_)) {
                                proto
                            } else if is_async {
                                self.async_generator_prototype_for_env(call_env)
                            } else {
                                self.generator_prototype_for_env(call_env)
                            }
                        };
                        // Lazy generator: don't run the body yet. Create a suspended
                        // generator object; the body runs incrementally via next().
                        let g_idx = self.heap.allocate(HeapObj::LazyGenerator(
                            crate::value::LazyGeneratorData {
                                fdef: func.clone(),
                                closure: call_env,
                                env: Mutex::new(prologue.env),
                                this_val: Mutex::new(this_val.clone()),
                                args: Mutex::new(args.to_vec()),
                                ip: AtomicUsize::new(prologue.ip),
                                stack: Mutex::new(prologue.stack),
                                locals: Mutex::new(prologue.locals),
                                catch_stack: Mutex::new(prologue.catch_stack),
                                finally_stack: Mutex::new(prologue.finally_stack),
                                guard_seq: AtomicU32::new(prologue.guard_seq),
                                finally_completion_tag: AtomicU8::new(
                                    prologue.finally_completion_tag,
                                ),
                                finally_completion_val: Mutex::new(prologue.finally_completion_val),
                                started: AtomicBool::new(false),
                                done: AtomicBool::new(false),
                                delegating: AtomicBool::new(false),
                                resume_value: Mutex::new(Value::Undefined),
                                is_async,
                                async_queue: Mutex::new(std::collections::VecDeque::new()),
                                async_processing: AtomicBool::new(false),
                                async_suspended_yield: AtomicBool::new(false),
                                async_delegate_await_kind: AtomicU8::new(0),
                                props: Mutex::new(IndexMap::new()),
                                proto: Mutex::new(Some(generator_instance_proto)),
                                extensible: AtomicBool::new(true),
                            },
                        ))?;
                        Ok(Value::Object(GcIdx(g_idx)))
                    } else {
                        // execute the compiled function chunk
                        let frame_new_target = if is_arrow {
                            lexical_new_target
                        } else {
                            self.pending_new_target_prototype.take();
                            self.pending_new_target.take().unwrap_or(Value::Undefined)
                        };
                        if is_async {
                            return self.execute_async_function(
                                Value::Object(idx),
                                func,
                                call_env,
                                this_val,
                                args,
                                frame_new_target,
                            );
                        }
                        let mut result = self.execute_chunk_func(
                            Value::Object(idx),
                            func.clone(),
                            call_env,
                            this_val,
                            args,
                            frame_new_target,
                        );
                        // For derived class constructors, check that `super()` was
                        // called (i.e. `this` is no longer in the TDZ). If the
                        // constructor returned without calling super, throw a
                        // ReferenceError per spec.
                        if func.is_derived {
                            if let Ok(ref rv) = result {
                                // Per spec [[Construct]] step 13: if a derived
                                // constructor returns a value, it must be an
                                // object (or undefined). Returning a primitive
                                // (number, string, boolean, null) is a TypeError.
                                let is_object = matches!(rv, Value::Object(_) | Value::Undefined);
                                if !is_object {
                                    return Err(Error::type_err(
                                    "Derived constructor may only return an object or undefined",
                                ));
                                }
                                if !matches!(rv, Value::Object(_)) {
                                    let bound_this = self.heap.with_obj(call_env.0, |obj| {
                                        if let HeapObj::Environment(e) = obj {
                                            let vars = e.vars.lock();
                                            vars.get("this").and_then(|b| {
                                                if b.initialized.load(Ordering::Relaxed) {
                                                    Some(b.value.lock().clone())
                                                } else {
                                                    None
                                                }
                                            })
                                        } else {
                                            None
                                        }
                                    });
                                    result = match bound_this {
                                    Some(this_val) => Ok(this_val),
                                    None => Err(Error::reference(
                                        "must call super constructor before accessing 'this' or returning"
                                    )),
                                };
                                }
                            }
                        }
                        result
                    }
                })();
                self.unpin_many(call_env_pin_count);
                let call_result =
                    call_result.map_err(|error| self.materialize_current_interpreted_error(error));
                debug_assert_eq!(self.execution_contexts.len(), context_depth + 1);
                self.execution_contexts.truncate(context_depth);
                call_result
            }
            Some(FuncCallInfo::Bound {
                target,
                this_val,
                bound_args,
            }) => {
                let mut all = bound_args;
                all.extend_from_slice(args);
                self.call_function(&Value::Object(target), &all, Some(this_val))
            }
            None => Err(Error::type_err("not a function".to_string())),
        }
    }

    pub fn construct(&mut self, constructor: &Value, args: &[Value]) -> error::Result<Value> {
        self.construct_with_new_target(constructor, args, constructor)
    }

    fn native_construct_mode(&self, idx: GcIdx) -> Option<NativeConstructMode> {
        self.heap.with_obj(idx.0, |obj| {
            let HeapObj::Function(f) = obj else {
                return None;
            };
            let FunctionKind::Native { construct_mode, .. } = &f.kind else {
                return None;
            };
            *construct_mode
        })
    }

    fn materialize_bound_constructor_arguments(
        &self,
        bound_functions: &[GcIdx],
        args: &[Value],
    ) -> error::Result<Vec<Value>> {
        let mut total_len = args.len();
        if total_len > crate::builtins::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS {
            return Err(Error::range("argument list too large"));
        }
        for function in bound_functions {
            let bound_len = self.heap.with_obj(function.0, |object| {
                let HeapObj::Function(data) = object else {
                    return None;
                };
                let FunctionKind::Bound { bound_args, .. } = &data.kind else {
                    return None;
                };
                Some(bound_args.len())
            });
            let bound_len =
                bound_len.ok_or_else(|| Error::internal("bound constructor metadata changed"))?;
            total_len = total_len
                .checked_add(bound_len)
                .ok_or_else(|| Error::range("argument list too large"))?;
            if total_len > crate::builtins::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS {
                return Err(Error::range("argument list too large"));
            }
        }

        let mut materialized = Vec::with_capacity(total_len);
        // The outermost bound function is visited first, but its arguments
        // follow every inner bound layer in the eventual [[Construct]] call.
        for function in bound_functions.iter().rev() {
            let appended = self.heap.with_obj(function.0, |object| {
                let HeapObj::Function(data) = object else {
                    return false;
                };
                let FunctionKind::Bound { bound_args, .. } = &data.kind else {
                    return false;
                };
                materialized.extend(bound_args.iter().cloned());
                true
            });
            if !appended {
                return Err(Error::internal("bound constructor metadata changed"));
            }
        }
        materialized.extend_from_slice(args);
        Ok(materialized)
    }

    pub(crate) fn call_function_with_new_target(
        &mut self,
        constructor: &Value,
        args: &[Value],
        this_value: Value,
        new_target: &Value,
        prototype_state: Option<NewTargetPrototype>,
    ) -> error::Result<Value> {
        // Dispatch can fail before it consumes pending construction metadata,
        // so the caller owns a scoped save/restore on every result path.
        let previous_new_target = self.pending_new_target.replace(new_target.clone());
        let previous_prototype =
            std::mem::replace(&mut self.pending_new_target_prototype, prototype_state);
        let result = self.call_function(constructor, args, Some(this_value));
        self.pending_new_target = previous_new_target;
        self.pending_new_target_prototype = previous_prototype;
        result
    }

    pub fn construct_with_new_target(
        &mut self,
        constructor: &Value,
        args: &[Value],
        new_target: &Value,
    ) -> error::Result<Value> {
        if !self.is_constructor_value(constructor) {
            return Err(Error::type_err("not a constructor".to_string()));
        }
        if !self.is_constructor_value(new_target) {
            return Err(Error::type_err("newTarget is not a constructor"));
        }
        if args.len() > crate::builtins::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS {
            return Err(Error::range("argument list too large"));
        }
        // `new` removes its operands from the VM stack before an observable
        // prototype lookup, so keep every construction input rooted here.
        let mut pin_count = self.pin(constructor);
        pin_count += self.pin_many(args);
        pin_count += self.pin(new_target);
        let result =
            self.construct_with_new_target_inner(constructor, args, new_target, &mut pin_count);
        self.unpin_many(pin_count);
        result
    }

    fn construct_with_new_target_inner(
        &mut self,
        constructor: &Value,
        args: &[Value],
        new_target: &Value,
        pin_count: &mut usize,
    ) -> error::Result<Value> {
        let mut active_constructor = constructor.clone();
        let mut active_new_target = new_target.clone();
        let mut bound_functions = Vec::new();
        let mut materialized_argument_count = args.len();
        let idx = loop {
            match self.constructor_traversal_step(&active_constructor) {
                ConstructorTraversalStep::Bound {
                    function,
                    target,
                    bound_len,
                    constructable,
                } => {
                    if !constructable {
                        return Err(Error::type_err("not a constructor"));
                    }
                    self.consume_fuel()?;
                    materialized_argument_count = materialized_argument_count
                        .checked_add(bound_len)
                        .ok_or_else(|| Error::range("argument list too large"))?;
                    if materialized_argument_count
                        > crate::builtins::call_arguments::MAX_MATERIALIZED_CALL_ARGUMENTS
                    {
                        return Err(Error::range("argument list too large"));
                    }
                    bound_functions
                        .try_reserve(1)
                        .map_err(|_| Error::range("constructor wrapper chain is too large"))?;
                    bound_functions.push(function);
                    *pin_count += self.pin(&Value::Object(function));
                    if active_constructor == active_new_target {
                        active_new_target = Value::Object(target);
                    }
                    active_constructor = Value::Object(target);
                }
                ConstructorTraversalStep::Proxy {
                    target,
                    handler,
                    constructable,
                    revoked,
                } => {
                    if !constructable {
                        return Err(Error::type_err("not a constructor"));
                    }
                    if revoked {
                        return Err(Error::type_err(
                            "Cannot perform 'construct' on a proxy that has been revoked",
                        ));
                    }
                    self.consume_fuel()?;
                    *pin_count += self.pin_many(&[target.clone(), handler.clone()]);
                    let trap = self.get_proxy_method(&handler, "construct")?;
                    if trap.is_nullish() {
                        active_constructor = target;
                        continue;
                    }
                    if !crate::builtins::is_callable(&trap, &self.heap) {
                        return Err(Error::type_err("Proxy construct trap is not callable"));
                    }
                    *pin_count += self.pin(&trap);
                    let active_args =
                        self.materialize_bound_constructor_arguments(&bound_functions, args)?;
                    let arg_array =
                        crate::builtins::make_value_array_in_current_realm(self, active_args)?;
                    *pin_count += self.pin(&arg_array);
                    let new_obj = self.call_function(
                        &trap,
                        &[target, arg_array, active_new_target.clone()],
                        Some(handler),
                    );
                    let new_obj = new_obj?;
                    if matches!(new_obj, Value::Object(_)) {
                        return Ok(new_obj);
                    }
                    return Err(Error::type_err(
                        "Proxy construct trap must return an object".to_string(),
                    ));
                }
                ConstructorTraversalStep::Function {
                    function,
                    constructable,
                    ..
                } => {
                    if !constructable {
                        return Err(Error::type_err("not a constructor"));
                    }
                    break function;
                }
                ConstructorTraversalStep::Other => {
                    return Err(Error::type_err("not a constructor"));
                }
            }
        };
        let constructor = &active_constructor;
        let active_args = self.materialize_bound_constructor_arguments(&bound_functions, args)?;
        let args = active_args.as_slice();
        let new_target = &active_new_target;
        match self.native_construct_mode(idx) {
            Some(NativeConstructMode::InternalEagerPrototype) => {
                let observed_prototype = self.get_property(new_target, "prototype")?;
                let prototype_state = if matches!(observed_prototype, Value::Object(_)) {
                    NewTargetPrototype::Observed(observed_prototype)
                } else {
                    NewTargetPrototype::FallbackRealm(self.constructor_realm(new_target)?)
                };
                return self.call_function_with_new_target(
                    constructor,
                    args,
                    Value::Undefined,
                    new_target,
                    Some(prototype_state),
                );
            }
            Some(NativeConstructMode::InternalDeferredPrototype) => {
                return self.call_function_with_new_target(
                    constructor,
                    args,
                    Value::Undefined,
                    new_target,
                    None,
                );
            }
            None => {}
        }
        // GetPrototypeFromConstructor reads the observable `.prototype`;
        // non-object values, including explicit null, use %Object.prototype%.
        let observed_proto = self.get_property(new_target, "prototype")?;
        let proto = observed_proto.clone();
        let proto = if matches!(proto, Value::Object(_)) {
            proto
        } else {
            self.constructor_realm_default_prototype(
                new_target,
                "Object",
                self.object_proto.clone(),
            )?
        };
        let class_name = self.heap.with_obj(idx.0, |obj| {
            if let HeapObj::Function(f) = obj {
                match f.name.as_deref() {
                    Some("Error")
                    | Some("EvalError")
                    | Some("RangeError")
                    | Some("ReferenceError")
                    | Some("SyntaxError")
                    | Some("TypeError")
                    | Some("URIError")
                    | Some("AggregateError") => Some(Arc::from("Error")),
                    Some("Date") => Some(Arc::from("Date")),
                    _ => None,
                }
            } else {
                None
            }
        });
        let prototype_pin = self.pin(&proto);
        let new_obj = HeapObj::Object(crate::value::ObjectData {
            props: Mutex::new(IndexMap::new()),
            proto: Mutex::new(Some(proto)),
            extensible: std::sync::atomic::AtomicBool::new(true),
            class_name,
            private_fields: Mutex::new(std::collections::HashMap::new()),
            primitive: Mutex::new(None),
        });
        let allocation = self.alloc(new_obj);
        self.unpin(prototype_pin);
        let this_obj = Value::Object(allocation?);
        let result = self.call_function_with_new_target(
            constructor,
            args,
            this_obj.clone(),
            new_target,
            Some(NewTargetPrototype::Observed(observed_proto)),
        )?;
        if matches!(result, Value::Object(_)) {
            Ok(result)
        } else {
            Ok(this_obj)
        }
    }

    // ---- iteration ----

    /// Build a heap iterator object that yields the values of `iterable`.
    pub fn make_iterator(&mut self, iterable: &Value) -> error::Result<Value> {
        // Built-in String/Map/Set/Generator values use fast paths below.
        // Arrays and arguments objects must observe their `@@iterator` method
        // because destructuring and for-of are sensitive to deletion and
        // overrides.
        let (is_map, is_set, is_gen, is_arr) = match iterable {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                (
                    matches!(o, HeapObj::Map(_)),
                    matches!(o, HeapObj::Set(_)),
                    matches!(o, HeapObj::Generator(_) | HeapObj::LazyGenerator(_)),
                    matches!(o, HeapObj::Array(_)),
                )
            }),
            _ => (false, false, false, false),
        };
        let is_builtin_iterable = is_map || is_set || is_gen;
        if !matches!(iterable, Value::Object(_)) {
            let sym_key = crate::value::PropertyKey::Symbol(self.well_known_symbols.iterator);
            let iter_method = self.get_property_by_key(iterable, &sym_key)?;
            if iter_method.is_nullish() {
                return Err(Error::type_err("value is not iterable"));
            }
            let iter_obj = self.call_function(&iter_method, &[], Some(iterable.clone()))?;
            return self.new_lazy_iterator(iter_obj);
        }
        if is_arr {
            let sym_key = crate::value::PropertyKey::Symbol(self.well_known_symbols.iterator);
            let iter_method = self.get_property_by_key(iterable, &sym_key)?;
            if iter_method.is_undefined() || iter_method.is_null() {
                return Err(Error::type_err("value is not iterable"));
            }
            let iter_obj = self.call_function(&iter_method, &[], Some(iterable.clone()))?;
            return self.new_lazy_iterator(iter_obj);
        }
        if !is_builtin_iterable {
            if let Value::Object(_) = iterable {
                let sym_key = crate::value::PropertyKey::Symbol(self.well_known_symbols.iterator);
                if self.has_property_key(iterable, &sym_key)? {
                    let iter_method = self.get_property_by_key(iterable, &sym_key)?;
                    let iter_obj = self.call_function(&iter_method, &[], Some(iterable.clone()))?;
                    return self.new_lazy_iterator(iter_obj);
                }
            }
        }
        match iterable {
            Value::Object(idx) => {
                let (is_array, is_map, is_set, is_generator) = self.heap.with_obj(idx.0, |o| {
                    (
                        matches!(o, HeapObj::Array(_)),
                        matches!(o, HeapObj::Map(_)),
                        matches!(o, HeapObj::Set(_)),
                        matches!(o, HeapObj::Generator(_) | HeapObj::LazyGenerator(_)),
                    )
                });
                if is_generator {
                    // Wrap the generator in a lazy iterator that resumes it per
                    // pull. This preserves the generator's return value (needed
                    // by `yield*`) and avoids eagerly draining infinite
                    // generators before the loop even starts.
                    self.new_generator_iterator(iterable.clone())
                } else if is_array {
                    unreachable!("array iterators are handled lazily above")
                } else if is_map {
                    let iter = crate::builtins::new_collection_iterator(
                        self,
                        iterable.clone(),
                        crate::value::CollectionIteratorKind::MapEntries,
                    )?;
                    self.new_lazy_iterator(iter)
                } else if is_set {
                    let iter = crate::builtins::new_collection_iterator(
                        self,
                        iterable.clone(),
                        crate::value::CollectionIteratorKind::SetValues,
                    )?;
                    self.new_lazy_iterator(iter)
                } else {
                    Err(Error::type_err("value is not iterable".to_string()))
                }
            }
            _ => Err(Error::type_err(format!(
                "{} is not iterable",
                iterable.type_of()
            ))),
        }
    }

    /// Obtain an async iterator for `for await...of`. Prefers a user-defined
    /// `Symbol.asyncIterator` method; falls back to the sync iterator protocol
    /// (`Symbol.iterator`) as an async-from-sync iterator. Async generators are
    /// wrapped directly (their `next()` already returns a Promise).
    pub fn make_async_iterator(&mut self, iterable: &Value) -> error::Result<Value> {
        if let Value::Object(_) = iterable {
            let akey = crate::value::PropertyKey::Symbol(self.well_known_symbols.async_iterator);
            if self.has_property_key(iterable, &akey)? {
                let m = self.get_property_by_key(iterable, &akey)?;
                if !m.is_nullish() {
                    if !crate::builtins::is_callable(&m, &self.heap) {
                        return Err(Error::type_err("Symbol.asyncIterator is not callable"));
                    }
                    let iter_obj = self.call_function(&m, &[], Some(iterable.clone()))?;
                    if !matches!(iter_obj, Value::Object(_)) {
                        return Err(Error::type_err("Async iterator is not an object"));
                    }
                    return self.new_lazy_iterator(iter_obj);
                }
            }
            // No async iterator: fall back to the sync iterator protocol. Each
            // `next()` is awaited (a non-Promise value awaits to itself).
            let it = self.make_iterator(iterable)?;
            self.mark_async_from_sync(&it);
            return Ok(it);
        }
        // Primitives (strings etc.): use the sync iterator, awaited per step.
        let it = self.make_iterator(iterable)?;
        self.mark_async_from_sync(&it);
        Ok(it)
    }

    fn mark_async_from_sync(&self, it: &Value) {
        if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |obj| {
                if let HeapObj::Iterator(data) = obj {
                    data.async_from_sync.store(true, Ordering::Relaxed);
                }
            });
        }
    }

    fn is_async_from_sync(&self, it: &Value) -> bool {
        match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |obj| {
                matches!(obj, HeapObj::Iterator(data) if data.async_from_sync.load(Ordering::Relaxed))
            }),
            _ => false,
        }
    }

    /// Build the lazy iterator state used by `for...in`.
    pub fn make_for_in_keys(&mut self, obj: &Value) -> error::Result<Value> {
        let source = if obj.is_nullish() {
            None
        } else {
            Some(self.to_object(obj)?)
        };
        self.new_for_in_iterator(source)
    }

    pub(crate) fn new_iterator(&mut self, items: Vec<Value>) -> error::Result<Value> {
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(items),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            lazy_next: Mutex::new(None),
            generator: Mutex::new(None),
            for_in: Mutex::new(None),
            async_from_sync: AtomicBool::new(false),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    /// Build a *lazy* iterator wrapping a JS iterator object (one returned by a
    /// user-defined `Symbol.iterator` method). Each `next()` call invokes the
    /// JS object's `next()` method and reads its `value`/`done` properties.
    pub(crate) fn new_lazy_iterator(&mut self, iter_obj: Value) -> error::Result<Value> {
        if !matches!(iter_obj, Value::Object(_)) {
            return Err(Error::type_err("iterator method must return an object"));
        }
        let mut pin_count = self.pin(&iter_obj);
        let result = (|| -> error::Result<Value> {
            let next = self.get_property(&iter_obj, "next")?;
            pin_count += self.pin(&next);
            let it = HeapObj::Iterator(crate::value::IteratorData {
                items: Mutex::new(Vec::new()),
                index: std::sync::atomic::AtomicUsize::new(0),
                lazy_iter: Mutex::new(Some(iter_obj)),
                lazy_next: Mutex::new(Some(next)),
                generator: Mutex::new(None),
                for_in: Mutex::new(None),
                async_from_sync: AtomicBool::new(false),
                done: std::sync::atomic::AtomicBool::new(false),
            });
            self.alloc(it).map(Value::Object)
        })();
        self.unpin_many(pin_count);
        result
    }

    pub(crate) fn new_async_from_sync_iterator(
        &mut self,
        iter_obj: Value,
        next: Value,
    ) -> error::Result<Value> {
        if !matches!(iter_obj, Value::Object(_)) {
            return Err(Error::type_err("iterator method must return an object"));
        }
        let iterator = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(Some(iter_obj)),
            lazy_next: Mutex::new(Some(next)),
            generator: Mutex::new(None),
            for_in: Mutex::new(None),
            async_from_sync: AtomicBool::new(true),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        self.alloc(iterator).map(Value::Object)
    }

    /// Build a lazy iterator wrapping a generator object. Each `next()` resumes
    /// the generator via `resume_generator`, preserving its return value (so
    /// `yield* gen()` yields the generator's return value as the result).
    pub(crate) fn new_generator_iterator(&mut self, gen: Value) -> error::Result<Value> {
        let next = self.get_property(&gen, "next")?;
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            lazy_next: Mutex::new(Some(next)),
            generator: Mutex::new(Some(gen)),
            for_in: Mutex::new(None),
            async_from_sync: AtomicBool::new(false),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        Ok(Value::Object(GcIdx(self.heap.allocate(it)?)))
    }

    pub(crate) fn new_for_in_iterator(&mut self, source: Option<Value>) -> error::Result<Value> {
        let source_pin = source.as_ref().map(|source| self.pin(source)).unwrap_or(0);
        let it = HeapObj::Iterator(crate::value::IteratorData {
            items: Mutex::new(Vec::new()),
            index: std::sync::atomic::AtomicUsize::new(0),
            lazy_iter: Mutex::new(None),
            lazy_next: Mutex::new(None),
            generator: Mutex::new(None),
            for_in: Mutex::new(Some(crate::value::ForInIteratorState {
                object: source,
                object_was_visited: false,
                visited_keys: indexmap::IndexSet::new(),
                remaining_keys: Vec::new(),
                remaining_index: 0,
            })),
            async_from_sync: AtomicBool::new(false),
            done: std::sync::atomic::AtomicBool::new(false),
        });
        let result = self.alloc(it).map(Value::Object);
        self.unpin(source_pin);
        result
    }

    pub(crate) fn iterator_close(&mut self, it: &Value) -> error::Result<()> {
        let iter_obj = match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    it.lazy_iter
                        .lock()
                        .clone()
                        .or_else(|| it.generator.lock().clone())
                } else {
                    None
                }
            }),
            _ => None,
        };
        let Some(iter_obj) = iter_obj else {
            return Ok(());
        };

        let return_method = self.get_property(&iter_obj, "return")?;
        if return_method.is_undefined() || matches!(return_method, Value::Null) {
            self.mark_iterator_done(it);
            return Ok(());
        }
        if !crate::builtins::is_callable(&return_method, &self.heap) {
            return Err(Error::type_err("Iterator return is not callable"));
        }
        let result = self.call_function(&return_method, &[], Some(iter_obj))?;
        if !matches!(result, Value::Object(_)) {
            return Err(Error::type_err("Iterator return result is not an object"));
        }
        self.mark_iterator_done(it);
        Ok(())
    }

    pub(crate) fn mark_iterator_done(&self, it: &Value) {
        if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    it.done.store(true, Ordering::Relaxed);
                }
            });
        }
    }

    pub fn iterator_next(&mut self, it: &Value) -> error::Result<(Value, bool)> {
        self.iterator_next_resume(it, Value::Undefined)
    }

    fn delegate_target_and_next(&self, it: &Value) -> (Option<Value>, Option<Value>) {
        match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(data) = o {
                    (
                        data.lazy_iter
                            .lock()
                            .clone()
                            .or_else(|| data.generator.lock().clone()),
                        data.lazy_next.lock().clone(),
                    )
                } else {
                    (None, None)
                }
            }),
            _ => (None, None),
        }
    }

    fn finish_delegate_result(
        &mut self,
        it: &Value,
        mut result: Value,
        return_completion: bool,
        rewrap_yield: bool,
    ) -> error::Result<DelegateOutcome> {
        if !matches!(result, Value::Object(_)) {
            return Err(Error::type_err("Iterator result is not an object"));
        }
        let done = self.get_property(&result, "done")?.is_truthy();
        if !done {
            if rewrap_yield {
                let value = self.get_property(&result, "value")?;
                result = crate::builtins::regexp::gen_result(self, value, false, false)?;
            }
            return Ok(DelegateOutcome::Yield(result));
        }
        self.mark_iterator_done(it);
        let value = self.get_property(&result, "value")?;
        if return_completion {
            Ok(DelegateOutcome::Return(value))
        } else {
            Ok(DelegateOutcome::Complete(value))
        }
    }

    pub(crate) fn iterator_delegate_step(
        &mut self,
        it: &Value,
        completion: ResumeKind,
        await_result: bool,
    ) -> error::Result<DelegateOutcome> {
        if await_result {
            return self.iterator_delegate_async_step(it, completion);
        }
        let (target, cached_next) = self.delegate_target_and_next(it);
        match completion {
            ResumeKind::Next(value) => {
                let result = if let Some(target) = target {
                    let next = cached_next
                        .ok_or_else(|| Error::type_err("Iterator next is not callable"))?;
                    if !crate::builtins::is_callable(&next, &self.heap) {
                        return Err(Error::type_err("Iterator next is not callable"));
                    }
                    self.call_function(&next, &[value], Some(target))?
                } else {
                    let (value, done) = self.iterator_next_resume(it, value)?;
                    crate::builtins::regexp::gen_result(self, value, done, false)?
                };
                self.finish_delegate_result(it, result, false, false)
            }
            ResumeKind::Return(value) => {
                let Some(target) = target else {
                    return Ok(DelegateOutcome::Return(value));
                };
                let method = self.get_property(&target, "return")?;
                if method.is_nullish() {
                    return Ok(DelegateOutcome::Return(value));
                }
                if !crate::builtins::is_callable(&method, &self.heap) {
                    return Err(Error::type_err("Iterator return is not callable"));
                }
                let result = self.call_function(&method, &[value], Some(target))?;
                self.finish_delegate_result(it, result, true, false)
            }
            ResumeKind::Throw(value) => {
                let Some(target) = target else {
                    return Err(Error::type_err("Iterator does not provide a throw method"));
                };
                let method = self.get_property(&target, "throw")?;
                if !method.is_nullish() {
                    if !crate::builtins::is_callable(&method, &self.heap) {
                        return Err(Error::type_err("Iterator throw is not callable"));
                    }
                    let result = self.call_function(&method, &[value], Some(target))?;
                    return self.finish_delegate_result(it, result, false, false);
                }

                let return_method = self.get_property(&target, "return")?;
                if !return_method.is_nullish() {
                    if !crate::builtins::is_callable(&return_method, &self.heap) {
                        return Err(Error::type_err("Iterator return is not callable"));
                    }
                    let result = self.call_function(&return_method, &[], Some(target))?;
                    if !matches!(result, Value::Object(_)) {
                        return Err(Error::type_err("Iterator return result is not an object"));
                    }
                }
                Err(Error::type_err("Iterator does not provide a throw method"))
            }
            ResumeKind::DelegateResult { .. }
            | ResumeKind::DelegateThrow(_)
            | ResumeKind::DelegateMissingThrow => Err(Error::internal(
                "internal async delegate completion reached sync iterator step",
            )),
        }
    }

    fn iterator_delegate_async_step(
        &mut self,
        it: &Value,
        completion: ResumeKind,
    ) -> error::Result<DelegateOutcome> {
        let (target, cached_next) = self.delegate_target_and_next(it);
        match completion {
            ResumeKind::DelegateResult {
                value,
                return_completion,
            } => self.finish_delegate_result(it, value, return_completion, true),
            ResumeKind::DelegateThrow(reason) => Err(Error::thrown(reason, &self.heap)),
            ResumeKind::DelegateMissingThrow => {
                Err(Error::type_err("Iterator does not provide a throw method"))
            }
            ResumeKind::Next(value) => {
                let result = if self.is_async_from_sync(it) {
                    self.async_from_sync_iterator_method_in_env(
                        it,
                        AsyncFromSyncMethod::Next,
                        Some(value),
                        self.current_realm_global_env(),
                    )?
                } else {
                    let target = target.ok_or_else(|| Error::type_err("not an async iterator"))?;
                    let next = cached_next
                        .ok_or_else(|| Error::type_err("Iterator next is not callable"))?;
                    if !crate::builtins::is_callable(&next, &self.heap) {
                        return Err(Error::type_err("Iterator next is not callable"));
                    }
                    self.call_function(&next, &[value], Some(target))?
                };
                Ok(DelegateOutcome::Await(result, DelegateAwaitKind::Result))
            }
            ResumeKind::Return(value) => {
                if self.is_async_from_sync(it) {
                    let result = self.async_from_sync_iterator_method_in_env(
                        it,
                        AsyncFromSyncMethod::Return,
                        Some(value),
                        self.current_realm_global_env(),
                    )?;
                    return Ok(DelegateOutcome::Await(
                        result,
                        DelegateAwaitKind::ReturnResult,
                    ));
                }
                let Some(target) = target else {
                    return Ok(DelegateOutcome::Return(value));
                };
                let method = self.get_property(&target, "return")?;
                if method.is_nullish() {
                    return Ok(DelegateOutcome::Return(value));
                }
                if !crate::builtins::is_callable(&method, &self.heap) {
                    return Err(Error::type_err("Iterator return is not callable"));
                }
                let result = self.call_function(&method, &[value], Some(target))?;
                Ok(DelegateOutcome::Await(
                    result,
                    DelegateAwaitKind::ReturnResult,
                ))
            }
            ResumeKind::Throw(value) => {
                if self.is_async_from_sync(it) {
                    let result = self.async_from_sync_iterator_method_in_env(
                        it,
                        AsyncFromSyncMethod::Throw,
                        Some(value),
                        self.current_realm_global_env(),
                    )?;
                    return Ok(DelegateOutcome::Await(result, DelegateAwaitKind::Result));
                }
                let target = target
                    .ok_or_else(|| Error::type_err("Iterator does not provide a throw method"))?;
                let method = self.get_property(&target, "throw")?;
                if !method.is_nullish() {
                    if !crate::builtins::is_callable(&method, &self.heap) {
                        return Err(Error::type_err("Iterator throw is not callable"));
                    }
                    let result = self.call_function(&method, &[value], Some(target))?;
                    return Ok(DelegateOutcome::Await(result, DelegateAwaitKind::Result));
                }
                let return_method = self.get_property(&target, "return")?;
                if return_method.is_nullish() {
                    return Err(Error::type_err("Iterator does not provide a throw method"));
                }
                if !crate::builtins::is_callable(&return_method, &self.heap) {
                    return Err(Error::type_err("Iterator return is not callable"));
                }
                let result = self.call_function(&return_method, &[], Some(target))?;
                Ok(DelegateOutcome::Await(
                    result,
                    DelegateAwaitKind::MissingThrow,
                ))
            }
        }
    }

    /// Like [`iterator_next`] but passes `resume` to a lazy iterator's JS
    /// `next()` method (used by `yield*` to forward the outer resume value to
    /// the delegated iterator). Eager (Vec-backed) iterators ignore `resume`.
    pub fn iterator_next_resume(
        &mut self,
        it: &Value,
        resume: Value,
    ) -> error::Result<(Value, bool)> {
        let (lazy, is_gen, is_for_in, already_done) = match it {
            Value::Object(idx) => self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    (
                        it.lazy_iter.lock().is_some(),
                        it.generator.lock().is_some(),
                        it.for_in.lock().is_some(),
                        it.done.load(Ordering::Relaxed),
                    )
                } else {
                    (false, false, false, true)
                }
            }),
            _ => return Err(Error::type_err("not an iterator".to_string())),
        };
        if already_done {
            return Ok((Value::Undefined, true));
        }
        if is_gen {
            // Resume the wrapped generator with `resume`. The generator's
            // return value (when done) is preserved as the iterator value.
            let gen = self.heap.with_obj(
                match it {
                    Value::Object(idx) => idx.0,
                    _ => return Err(Error::type_err("not an iterator".to_string())),
                },
                |o| {
                    if let HeapObj::Iterator(it) = o {
                        it.generator.lock().clone()
                    } else {
                        None
                    }
                },
            );
            let gen = gen.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let g_idx = match &gen {
                Value::Object(idx) => *idx,
                _ => return Err(Error::type_err("not a generator".to_string())),
            };
            let (mut value, mut done, forwarded_result, _awaiting) =
                self.resume_generator(g_idx, ResumeKind::Next(resume))?;
            if forwarded_result {
                done = self.get_property(&value, "done")?.is_truthy();
                value = if done {
                    Value::Undefined
                } else {
                    self.get_property(&value, "value")?
                };
            }
            if done {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.store(true, Ordering::Relaxed);
                        }
                    });
                }
            }
            return Ok((value, done));
        }
        if is_for_in {
            let idx = match it {
                Value::Object(idx) => idx.0,
                _ => return Err(Error::type_err("not an iterator".to_string())),
            };
            enum ForInAction {
                Snapshot(Value),
                Candidate {
                    object: Value,
                    key: Arc<str>,
                    already_visited: bool,
                },
                Prototype(Value),
                Complete,
                Invalid,
            }

            let initial_object = self.heap.with_obj(idx, |o| {
                if let HeapObj::Iterator(iterator) = o {
                    iterator
                        .for_in
                        .lock()
                        .as_ref()
                        .and_then(|state| state.object.clone())
                } else {
                    None
                }
            });
            let iterator_pin = self.pin(it);
            let initial_object_pin = initial_object
                .as_ref()
                .map(|object| self.pin(object))
                .unwrap_or(0);
            let mut traversal = PropertyTraversal::new(std::slice::from_ref(it), 0);
            let result = (|| loop {
                let action = self.heap.with_obj(idx, |o| {
                    let HeapObj::Iterator(iterator) = o else {
                        return ForInAction::Invalid;
                    };
                    let mut state = iterator.for_in.lock();
                    let Some(state) = state.as_mut() else {
                        return ForInAction::Invalid;
                    };
                    let Some(object) = state.object.clone() else {
                        return ForInAction::Complete;
                    };
                    if !state.object_was_visited {
                        return ForInAction::Snapshot(object);
                    }
                    if let Some(key) = state.remaining_keys.get(state.remaining_index).cloned() {
                        state.remaining_index += 1;
                        let already_visited = state.visited_keys.contains(&key);
                        return ForInAction::Candidate {
                            object,
                            key,
                            already_visited,
                        };
                    }
                    ForInAction::Prototype(object)
                });

                match action {
                    ForInAction::Snapshot(object) => {
                        let keys = crate::builtins::own_property_keys_or_throw(
                            self, &object, false, true, true,
                        )?;
                        let strings = keys
                            .into_iter()
                            .filter_map(|key| match key {
                                PropertyKey::Str(name) => Some(name),
                                PropertyKey::Symbol(_) => None,
                            })
                            .collect();
                        self.heap.with_obj(idx, |o| {
                            if let HeapObj::Iterator(iterator) = o {
                                if let Some(state) = iterator.for_in.lock().as_mut() {
                                    state.remaining_keys = strings;
                                    state.remaining_index = 0;
                                    state.object_was_visited = true;
                                }
                            }
                        });
                    }
                    ForInAction::Candidate {
                        object,
                        key,
                        already_visited,
                    } => {
                        self.consume_fuel()?;
                        if already_visited {
                            continue;
                        }
                        let property_key = PropertyKey::from(key.clone());
                        let descriptor = crate::builtins::own_property_descriptor_for_key_or_throw(
                            self,
                            &object,
                            &property_key,
                        )?;
                        let Some(descriptor) = descriptor else {
                            continue;
                        };
                        self.heap.with_obj(idx, |o| {
                            if let HeapObj::Iterator(iterator) = o {
                                if let Some(state) = iterator.for_in.lock().as_mut() {
                                    state.visited_keys.insert(key.clone());
                                }
                            }
                        });
                        if descriptor.enumerable {
                            return Ok((Value::String(key), false));
                        }
                    }
                    ForInAction::Prototype(object) => {
                        let Value::Object(object_idx) = object else {
                            return Err(Error::internal(
                                "for-in iterator current value is not an object",
                            ));
                        };
                        let is_proxy = self
                            .heap
                            .with_obj(object_idx.0, |o| matches!(o, HeapObj::Proxy(_)));
                        if is_proxy {
                            traversal.note_proxy();
                        }
                        let prototype = self.get_prototype_of(&Value::Object(object_idx))?;
                        if let Some(prototype) = prototype {
                            self.advance_property_edge(
                                &mut traversal,
                                object_idx,
                                &prototype,
                                !is_proxy,
                            )?;
                            self.heap.with_obj(idx, |o| {
                                if let HeapObj::Iterator(iterator) = o {
                                    if let Some(state) = iterator.for_in.lock().as_mut() {
                                        state.object = Some(prototype);
                                        state.object_was_visited = false;
                                        state.remaining_keys.clear();
                                        state.remaining_index = 0;
                                    }
                                }
                            });
                        } else {
                            self.heap.with_obj(idx, |o| {
                                if let HeapObj::Iterator(iterator) = o {
                                    if let Some(state) = iterator.for_in.lock().as_mut() {
                                        state.object = None;
                                        state.remaining_keys.clear();
                                        state.remaining_index = 0;
                                    }
                                    iterator.done.store(true, Ordering::Relaxed);
                                }
                            });
                            return Ok((Value::Undefined, true));
                        }
                    }
                    ForInAction::Complete => {
                        self.heap.with_obj(idx, |o| {
                            if let HeapObj::Iterator(iterator) = o {
                                iterator.done.store(true, Ordering::Relaxed);
                            }
                        });
                        return Ok((Value::Undefined, true));
                    }
                    ForInAction::Invalid => {
                        return Err(Error::type_err("not an iterator".to_string()));
                    }
                }
            })();
            self.unpin_many(traversal.pin_count());
            self.unpin(initial_object_pin);
            self.unpin(iterator_pin);
            return result;
        }
        if lazy {
            // Call the JS iterator object's next() method and read {value, done}.
            let (iter_obj, next_fn) = self.heap.with_obj(
                match it {
                    Value::Object(idx) => idx.0,
                    _ => return Err(Error::type_err("not an iterator".to_string())),
                },
                |o| {
                    if let HeapObj::Iterator(it) = o {
                        (it.lazy_iter.lock().clone(), it.lazy_next.lock().clone())
                    } else {
                        (None, None)
                    }
                },
            );
            let iter_obj =
                iter_obj.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let next_fn =
                next_fn.ok_or_else(|| Error::type_err("Iterator next is not callable"))?;
            if !crate::builtins::is_callable(&next_fn, &self.heap) {
                return Err(Error::type_err("Iterator next is not callable"));
            }
            let result = self.call_function(&next_fn, &[resume], Some(iter_obj))?;
            if !matches!(result, Value::Object(_)) {
                return Err(Error::type_err("Iterator result is not an object"));
            }
            let result_pin = self.pin(&result);
            let fields = (|| -> error::Result<(Value, bool)> {
                let done = self.get_property(&result, "done")?.is_truthy();
                if done {
                    return Ok((Value::Undefined, true));
                }
                Ok((self.get_property(&result, "value")?, false))
            })();
            self.unpin_many(result_pin);
            let (value, done) = fields?;
            if done {
                if let Value::Object(idx) = it {
                    self.heap.with_obj(idx.0, |o| {
                        if let HeapObj::Iterator(it) = o {
                            it.done.store(true, Ordering::Relaxed);
                        }
                    });
                }
                return Ok((Value::Undefined, true));
            }
            Ok((value, done))
        } else {
            let idx = match it {
                Value::Object(idx) => idx.0,
                _ => return Err(Error::type_err("not an iterator".to_string())),
            };
            self.heap.with_obj(idx, |o| {
                if let HeapObj::Iterator(it) = o {
                    let items = it.items.lock();
                    let i = it.index.load(Ordering::Relaxed);
                    if i < items.len() {
                        let v = items[i].clone();
                        it.index.store(i + 1, Ordering::Relaxed);
                        Ok((v, false))
                    } else {
                        Ok((Value::Undefined, true))
                    }
                } else {
                    Err(Error::type_err("not an iterator".to_string()))
                }
            })
        }
    }

    fn async_from_sync_iterator_next_in_env(
        &mut self,
        it: &Value,
        realm: GcIdx,
    ) -> error::Result<Value> {
        self.async_from_sync_iterator_method_in_env(it, AsyncFromSyncMethod::Next, None, realm)
    }

    fn async_from_sync_iterator_method_in_env(
        &mut self,
        it: &Value,
        kind: AsyncFromSyncMethod,
        argument: Option<Value>,
        realm: GcIdx,
    ) -> error::Result<Value> {
        let capability = self.new_intrinsic_promise_capability_in_env(realm)?;
        let promise = capability.promise.clone();
        let mut pins = self.pin_many(&[
            promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
            it.clone(),
        ]);
        if let Some(argument) = &argument {
            pins += self.pin(argument);
        }
        let setup = (|| -> error::Result<()> {
            let (iterator, cached_next) = self.delegate_target_and_next(it);
            let Some(iterator) = iterator else {
                let error = Error::type_err("not an async-from-sync iterator");
                self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                return Ok(());
            };

            let method = match kind {
                AsyncFromSyncMethod::Next => cached_next.unwrap_or(Value::Undefined),
                AsyncFromSyncMethod::Return => match self.get_property(&iterator, "return") {
                    Ok(method) if !method.is_nullish() => method,
                    Ok(_) => {
                        let result = crate::builtins::regexp::gen_result_in_env(
                            self,
                            argument.clone().unwrap_or(Value::Undefined),
                            true,
                            false,
                            realm,
                        )?;
                        self.mark_iterator_done(it);
                        return self.resolve_promise_capability_value(&capability, result);
                    }
                    Err(error) => {
                        self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                        return Ok(());
                    }
                },
                AsyncFromSyncMethod::Throw => match self.get_property(&iterator, "throw") {
                    Ok(method) if !method.is_nullish() => method,
                    Ok(_) => {
                        if let Err(error) = self.iterator_close(it) {
                            self.reject_promise_capability_error_in_env(
                                &capability,
                                &error,
                                realm,
                            )?;
                            return Ok(());
                        }
                        let error = Error::type_err("Iterator does not provide a throw method");
                        self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                        return Ok(());
                    }
                    Err(error) => {
                        self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                        return Ok(());
                    }
                },
            };
            if !crate::builtins::is_callable(&method, &self.heap) {
                let name = match kind {
                    AsyncFromSyncMethod::Next => "next",
                    AsyncFromSyncMethod::Return => "return",
                    AsyncFromSyncMethod::Throw => "throw",
                };
                let error = Error::type_err(format!("Iterator {name} is not callable"));
                self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                return Ok(());
            }
            let args = argument.clone().into_iter().collect::<Vec<_>>();
            let result = match self.call_function(&method, &args, Some(iterator)) {
                Ok(result) if matches!(result, Value::Object(_)) => result,
                Ok(_) => {
                    let error = Error::type_err("Iterator result is not an object");
                    self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                    return Ok(());
                }
                Err(error) => {
                    self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                    return Ok(());
                }
            };
            let result_pin = self.pin(&result);
            let fields = (|| -> error::Result<(Value, bool)> {
                let done = self.get_property(&result, "done")?.is_truthy();
                let value = self.get_property(&result, "value")?;
                Ok((value, done))
            })();
            self.unpin_many(result_pin);
            let (value, done) = match fields {
                Ok(fields) => fields,
                Err(error) => {
                    self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                    return Ok(());
                }
            };
            if done {
                self.mark_iterator_done(it);
            }
            self.attach_async_from_sync_iterator_continuation(
                &capability,
                value,
                done,
                matches!(kind, AsyncFromSyncMethod::Next | AsyncFromSyncMethod::Throw)
                    .then(|| it.clone()),
                matches!(kind, AsyncFromSyncMethod::Next | AsyncFromSyncMethod::Throw),
                realm,
            )
        })();
        self.unpin_many(pins);
        setup?;
        Ok(promise)
    }

    fn attach_async_from_sync_iterator_continuation(
        &mut self,
        capability: &PromiseReactionCapability,
        value: Value,
        done: bool,
        iterator: Option<Value>,
        close_on_rejection: bool,
        realm: GcIdx,
    ) -> error::Result<()> {
        let value_wrapper = match self.promise_resolve_intrinsic_in_env(value, realm) {
            Ok(promise) => promise,
            Err(error) => {
                let reason = self.promise_rejection_reason_in_realm(&error, realm)?;
                let reason_pin = self.pin(&reason);
                if !done && close_on_rejection {
                    if let Some(iterator) = &iterator {
                        if let Err(close_error) = self.iterator_close(iterator) {
                            if !close_error.catchable() {
                                self.unpin(reason_pin);
                                return Err(close_error);
                            }
                        }
                    }
                }
                let result = self.settle_promise_capability(capability, reason, true);
                self.unpin(reason_pin);
                return result;
            }
        };
        let state = self.heap.with_obj(value_wrapper.0, |object| {
            if let HeapObj::Promise(data) = object {
                *data.state.lock()
            } else {
                PromiseStatus::Fulfilled
            }
        });
        let handler = crate::value::PromiseHandler {
            on_fulfilled: Value::Undefined,
            on_rejected: Value::Undefined,
            derived: None,
            continuation: Some(crate::value::PromiseContinuation::AsyncFromSyncIterator {
                capability: capability.clone(),
                done,
                iterator,
                close_on_rejection: close_on_rejection && !done,
                realm,
            }),
        };
        if state == PromiseStatus::Pending {
            self.heap.with_obj(value_wrapper.0, |object| {
                if let HeapObj::Promise(data) = object {
                    data.handlers.lock().push(handler);
                }
            });
        } else {
            self.microtask_queue.push_back(Microtask::Then {
                promise: value_wrapper,
                on_fulfilled: Value::Undefined,
                on_rejected: Value::Undefined,
                derived: None,
                continuation: handler.continuation,
                realm: None,
            });
        }
        Ok(())
    }

    pub(crate) fn async_from_sync_iterator_close_start_in_env(
        &mut self,
        it: &Value,
        realm: GcIdx,
    ) -> error::Result<Value> {
        let capability = self.new_intrinsic_promise_capability_in_env(realm)?;
        let promise = capability.promise.clone();
        let pins = self.pin_many(&[
            promise.clone(),
            capability.resolve.clone(),
            capability.reject.clone(),
            it.clone(),
        ]);
        let setup = (|| -> error::Result<()> {
            let iterator = self
                .delegate_target_and_next(it)
                .0
                .ok_or_else(|| Error::type_err("not an async-from-sync iterator"))?;
            let return_method = match self.get_property(&iterator, "return") {
                Ok(method) => method,
                Err(error) => {
                    self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                    return Ok(());
                }
            };
            if return_method.is_nullish() {
                let iterator_result = crate::builtins::regexp::gen_result_in_env(
                    self,
                    Value::Undefined,
                    true,
                    false,
                    realm,
                )?;
                self.mark_iterator_done(it);
                return self.resolve_promise_capability_value(&capability, iterator_result);
            }
            let (value, done) = {
                if !crate::builtins::is_callable(&return_method, &self.heap) {
                    let error = Error::type_err("Iterator return is not callable");
                    self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                    return Ok(());
                }
                let returned = match self.call_function(&return_method, &[], Some(iterator)) {
                    Ok(value) => value,
                    Err(error) => {
                        self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                        return Ok(());
                    }
                };
                if !matches!(returned, Value::Object(_)) {
                    let error = Error::type_err("Iterator return result is not an object");
                    self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                    return Ok(());
                }
                let returned_pin = self.pin(&returned);
                let fields = (|| -> error::Result<(Value, bool)> {
                    let done = self.get_property(&returned, "done")?.is_truthy();
                    let value = self.get_property(&returned, "value")?;
                    Ok((value, done))
                })();
                self.unpin_many(returned_pin);
                match fields {
                    Ok(fields) => fields,
                    Err(error) => {
                        self.reject_promise_capability_error_in_env(&capability, &error, realm)?;
                        return Ok(());
                    }
                }
            };
            self.mark_iterator_done(it);
            self.attach_async_from_sync_iterator_continuation(
                &capability,
                value,
                done,
                None,
                false,
                realm,
            )
        })();
        self.unpin_many(pins);
        setup?;
        Ok(promise)
    }

    /// Call the next method used by `for await`, leaving Await to the bytecode
    /// interpreter so an async frame can suspend instead of draining jobs.
    pub(crate) fn iterator_next_await_start(&mut self, it: &Value) -> error::Result<Value> {
        let realm = self.current_realm_global_env();
        self.iterator_next_await_start_in_env(it, realm)
    }

    pub(crate) fn iterator_next_await_start_in_env(
        &mut self,
        it: &Value,
        realm: GcIdx,
    ) -> error::Result<Value> {
        if self.is_async_from_sync(it) {
            return self.async_from_sync_iterator_next_in_env(it, realm);
        }
        let lazy_or_gen = if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(i) = o {
                    i.lazy_iter.lock().is_some() || i.generator.lock().is_some()
                } else {
                    false
                }
            })
        } else {
            false
        };
        if lazy_or_gen {
            // Resolve the iterator object whose `next()` we call: either the
            // wrapped JS async iterator or the generator itself.
            let (iter_obj, next_fn) = if let Value::Object(idx) = it {
                self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Iterator(i) = o {
                        (
                            i.lazy_iter
                                .lock()
                                .clone()
                                .or_else(|| i.generator.lock().clone()),
                            i.lazy_next.lock().clone(),
                        )
                    } else {
                        (None, None)
                    }
                })
            } else {
                (None, None)
            };
            let iter_obj =
                iter_obj.ok_or_else(|| Error::type_err("not an iterator".to_string()))?;
            let next_fn = match next_fn {
                Some(next_fn) => next_fn,
                None => self.get_property(&iter_obj, "next")?,
            };
            return self.call_function(&next_fn, &[], Some(iter_obj));
        }

        let (value, done) = self.iterator_next(it)?;
        crate::builtins::regexp::gen_result(self, value, done, false)
    }

    pub(crate) fn iterator_unpack_await_result(
        &mut self,
        it: &Value,
        result: Value,
    ) -> error::Result<(Value, bool)> {
        if !matches!(result, Value::Object(_)) {
            return Err(Error::type_err("Iterator result is not an object"));
        }
        let done = self.get_property(&result, "done")?.is_truthy();
        if done {
            self.mark_iterator_done(it);
            return Ok((Value::Undefined, true));
        }
        let value = self.get_property(&result, "value")?;
        Ok((value, done))
    }

    /// Synchronous embedding helper for a complete `for await` iterator step.
    pub fn iterator_next_await(&mut self, it: &Value) -> error::Result<(Value, bool)> {
        let result = self.iterator_next_await_start(it)?;
        let result = self.await_value(result)?;
        self.iterator_unpack_await_result(it, result)
    }

    /// Await a value by resolving objects through their observable `then`
    /// method, then draining Promise jobs until the resulting Promise settles.
    pub(crate) fn await_value(&mut self, v: Value) -> error::Result<Value> {
        if let Value::Object(idx) = &v {
            let is_promise = self
                .heap
                .with_obj(idx.0, |o| matches!(o, HeapObj::Promise(_)));
            if is_promise {
                self.run_microtasks()?;
                let (state, result) = self.heap.with_obj(idx.0, |o| {
                    if let HeapObj::Promise(p) = o {
                        (*p.state.lock(), p.result.lock().clone())
                    } else {
                        (PromiseStatus::Fulfilled, Value::Undefined)
                    }
                });
                if state == PromiseStatus::Rejected {
                    return Err(Error::thrown(result, &self.heap));
                }
                return Ok(result);
            }

            let then = self.get_property(&v, "then")?;
            if crate::builtins::is_callable(&then, &self.heap) {
                let current_realm = self.current_realm_global_env();
                let realm = self.constructor_realm_or_fallback(&then, current_realm)?;
                let ctor = self.current_realm_promise_constructor();
                let capability = crate::builtins::new_promise_capability(self, ctor)?;
                let pins = self.pin_many(&[
                    v.clone(),
                    then.clone(),
                    capability.promise.clone(),
                    capability.resolve.clone(),
                    capability.reject.clone(),
                ]);
                let call_result = self.call_function(
                    &then,
                    &[capability.resolve.clone(), capability.reject.clone()],
                    Some(v),
                );
                if let Err(error) = call_result {
                    let reason = match self.promise_rejection_reason_in_realm(&error, realm) {
                        Ok(reason) => reason,
                        Err(error) => {
                            self.unpin_many(pins);
                            return Err(error);
                        }
                    };
                    if let Err(reject_error) =
                        self.call_function(&capability.reject, &[reason], Some(Value::Undefined))
                    {
                        self.unpin_many(pins);
                        return Err(reject_error);
                    }
                }
                let awaited = self.await_value(capability.promise.clone());
                self.unpin_many(pins);
                return awaited;
            }
        }
        Ok(v)
    }

    pub fn iterator_done(&self, it: &Value) -> bool {
        if let Value::Object(idx) = it {
            self.heap.with_obj(idx.0, |o| {
                if let HeapObj::Iterator(it) = o {
                    return it.index.load(Ordering::Relaxed) >= it.items.lock().len();
                }
                false
            })
        } else {
            false
        }
    }
}
