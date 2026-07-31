//! `for await...of` async iteration.

mod common;
use common::run;
use ruja::{Value, Vm};
use std::sync::Arc;

#[test]
fn for_await_over_async_generator() {
    let src = r#"
        async function* gen() { yield 1; yield 2; yield 3; }
        async function main() {
            let sum = 0;
            for await (let x of gen()) { sum += x; }
            return sum;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(6.0));
}

#[test]
fn for_await_over_custom_async_iterator() {
    let src = r#"
        let obj = {
            [Symbol.asyncIterator]: async function*() { yield 10; yield 20; }
        };
        async function main() {
            let total = 0;
            for await (let v of obj) { total += v; }
            return total;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(30.0));
}

#[test]
fn for_await_over_sync_iterable_fallback() {
    let src = r#"
        async function main() {
            let s = "";
            for await (let c of ["a","b","c"]) { s += c; }
            return s;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("abc")));
}

#[test]
fn for_await_only_unwraps_async_from_sync_values() {
    let src = r#"
        async function check() {
            let nativePromise = Promise.resolve("native");
            let nativeAsyncIterable = {
                [Symbol.asyncIterator]() {
                    let done = false;
                    return {
                        next() {
                            if (done) return { value: undefined, done: true };
                            done = true;
                            return { value: nativePromise, done: false };
                        }
                    };
                }
            };
            let nativeValue;
            for await (let value of nativeAsyncIterable) nativeValue = value;

            let syncValue;
            for await (let value of [Promise.resolve("sync")]) syncValue = value;

            return [nativeValue === nativePromise, syncValue].join("|");
        }
        await check();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("true|sync")));
}

#[test]
fn for_await_break_exits_early() {
    let src = r#"
        async function* gen() { yield 1; yield 2; yield 3; yield 4; }
        async function main() {
            let collected = [];
            for await (let x of gen()) {
                collected.push(x);
                if (x === 2) break;
            }
            return collected.join(",");
        }
        await main();
    "#;
    assert_eq!(run(src), Value::String(Arc::from("1,2")));
}

#[test]
fn for_await_body_await() {
    let src = r#"
        async function* gen() { yield 5; }
        async function double(x) { return x * 2; }
        async function main() {
            let total = 0;
            for await (let x of gen()) { total += await double(x); }
            return total;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(10.0));
}

#[test]
fn for_await_interleaves_with_promise_jobs() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        const actual = [];
        async function* naturalNumbers(start) {
          while (start > 0) yield Promise.resolve(start--);
        }
        async function trigger() {
          for await (const value of naturalNumbers(3)) {
            actual.push("Await: " + value);
          }
        }
        function countdown(counter) {
          actual.push("Promise: " + counter);
          if (counter > 0) {
            return Promise.resolve(counter - 1).then(countdown);
          }
        }
        trigger();
        countdown(6);
        "#,
    )
    .expect("for-await and Promise jobs should complete");
    let result = vm
        .run("actual.join('|');")
        .expect("failed to read interleaved job order");

    assert_eq!(
        result,
        Value::String(Arc::from(
            "Promise: 6|Promise: 5|Await: 3|Promise: 4|Promise: 3|Await: 2|Promise: 2|Promise: 1|Await: 1|Promise: 0"
        ))
    );
}

#[test]
fn for_await_allows_async_identifier_lhs() {
    let source = r#"
        let async;
        async function assign() {
            for await (async of [7]);
        }
        await assign();
        async;
    "#;
    assert_eq!(run(source), Value::Number(7.0));
}

#[test]
fn async_from_sync_observes_constructor_lookup_and_job_ticks() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        async function iterate() {
            var promise = Promise.resolve(0);
            actual.push("pre");
            for await (var value of [promise]) actual.push("loop");
            actual.push("post");
        }
        Promise.resolve(0)
            .then(() => actual.push("tick 1"))
            .then(() => actual.push("tick 2"))
            .then(() => actual.push("tick 3"))
            .then(() => actual.push("tick 4"));
        Object.defineProperty(Promise.prototype, "constructor", {
            get() {
                actual.push("constructor");
                return Promise;
            },
            configurable: true
        });
        iterate();
        "#,
    )
    .expect("failed to run async-from-sync iterator");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read async-from-sync job order"),
        Value::String(Arc::from(
            "pre|constructor|constructor|tick 1|tick 2|loop|constructor|tick 3|tick 4|post"
        ))
    );
}

#[test]
fn async_from_sync_rejects_abrupt_promise_constructor_lookup() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.run(
        r#"
        var actual = [];
        var marker = {};
        async function iterate() {
            var promise = Promise.resolve(0);
            Object.defineProperty(promise, "constructor", {
                get() { throw marker; }
            });
            actual.push("start");
            for await (var value of [promise]);
            actual.push("unreachable");
        }
        Promise.resolve(0)
            .then(() => actual.push("tick 1"))
            .then(() => actual.push("tick 2"));
        iterate().catch(error => actual.push("catch:" + (error === marker)));
        "#,
    )
    .expect("failed to run abrupt async-from-sync iterator");

    assert_eq!(
        vm.run("actual.join('|');")
            .expect("failed to read async-from-sync rejection order"),
        Value::String(Arc::from("start|tick 1|tick 2|catch:true"))
    );
}

#[test]
fn for_await_closes_sync_generator_after_yielded_promise_rejection() {
    let source = r#"
        var reason = {};
        var log = [];
        function* values() {
            try {
                yield Promise.reject(reason);
            } finally {
                log.push("close");
            }
        }
        async function iterate() {
            try {
                for await (var value of values()) log.push("unreachable");
            } catch (error) {
                log.push(error === reason ? "reason" : "wrong");
            }
            return log.join("|");
        }
        await iterate();
    "#;
    assert_eq!(run(source), Value::String(Arc::from("close|reason")));
}

#[test]
fn for_await_empty_async_generator() {
    let src = r#"
        async function* gen() {}
        async function main() {
            let count = 0;
            for await (let x of gen()) { count++; }
            return count;
        }
        await main();
    "#;
    assert_eq!(run(src), Value::Number(0.0));
}

#[test]
fn for_await_awaits_async_iterator_close_before_completion() {
    let source = r#"
        var log = [];
        var iterable = {
          [Symbol.asyncIterator]() {
            var done = false;
            return {
              next() {
                if (done) return Promise.resolve({ done: true });
                done = true;
                return Promise.resolve({ value: 1, done: false });
              },
              return() {
                log.push("return");
                return Promise.resolve().then(function () {
                  log.push("closed");
                  return {};
                });
              }
            };
          }
        };
        async function checkBreak() {
          for await (var value of iterable) {
            log.push("body");
            break;
          }
          log.push("after-break");
        }
        async function checkReturn() {
          for await (var value of iterable) return "returned";
        }
        await checkBreak();
        log.push(await checkReturn());
        log.join("|");
    "#;
    assert_eq!(
        run(source),
        Value::String(Arc::from(
            "body|return|closed|after-break|return|closed|returned"
        ))
    );
}

#[test]
fn for_await_close_error_obeys_original_completion_precedence() {
    let source = r#"
        var original = {};
        var closeError = {};
        function iterableWith(returnMethod) {
          return {
            [Symbol.asyncIterator]() {
              return {
                next() { return Promise.resolve({ value: 1, done: false }); },
                return: returnMethod
              };
            }
          };
        }
        async function throwing(returnMethod) {
          try {
            for await (var value of iterableWith(returnMethod)) throw original;
          } catch (error) { return error === original; }
        }
        async function breaking(returnMethod) {
          try {
            for await (var value of iterableWith(returnMethod)) break;
            return "normal";
          } catch (error) {
            return error === closeError ? "close" : error.name;
          }
        }
        var rejected = function () { return Promise.reject(closeError); };
        var primitive = function () { return Promise.resolve(1); };
        var nonCallable = 1;
        [
          await throwing(rejected),
          await breaking(rejected),
          await throwing(primitive),
          await breaking(primitive),
          await throwing(nonCallable),
          await breaking(nonCallable)
        ].join("|");
    "#;
    assert_eq!(
        run(source),
        Value::String(Arc::from("true|close|true|TypeError|true|TypeError"))
    );
}

#[test]
fn for_await_async_from_sync_close_awaits_value_and_skips_inner_continue() {
    let source = r#"
        var log = [];
        var returns = 0;
        var syncIterable = {
          [Symbol.iterator]() {
            var value = 0;
            return {
              next() { return { value: ++value, done: false }; },
              return() {
                returns++;
                return {
                  done: true,
                  value: Promise.resolve().then(function () {
                    log.push("close-value");
                    return undefined;
                  })
                };
              }
            };
          }
        };
        async function check() {
          for await (var value of syncIterable) {
            log.push("body:" + value);
            if (value === 1) continue;
            break;
          }
          log.push("after");
        }
        await check();
        log.join("|") + ":" + returns;
    "#;
    assert_eq!(
        run(source),
        Value::String(Arc::from("body:1|body:2|close-value|after:1"))
    );
}

#[test]
fn for_await_restores_stack_and_environment_before_async_close() {
    let source = r#"
        var log = [];
        var original = {};
        var target = {};
        var iterable = {
          [Symbol.asyncIterator]() {
            return {
              next() { return Promise.resolve({ value: 1, done: false }); },
              return() {
                log.push("return");
                return Promise.resolve().then(function () {
                  log.push("closed");
                  return {};
                });
              }
            };
          }
        };
        function key() {
          log.push("key");
          throw original;
        }
        async function check() {
          var local = "alive";
          try {
            for await (target[key()] of iterable) {}
          } catch (error) {
            log.push(error === original ? local : "wrong");
          }
        }
        await check();
        log.join("|");
    "#;
    assert_eq!(
        run(source),
        Value::String(Arc::from("key|return|closed|alive"))
    );
}

#[test]
fn for_await_closes_only_for_transfers_leaving_the_loop() {
    let source = r#"
        var returns = 0;
        function iterable() {
          return {
            [Symbol.asyncIterator]() {
              var value = 0;
              return {
                next() { return Promise.resolve({ value: ++value, done: false }); },
                return() { returns++; return Promise.resolve({}); }
              };
            }
          };
        }
        async function check() {
          outer: for (var round = 0; round < 1; round++) {
            for await (var value of iterable()) {
              if (value === 1) continue;
              continue outer;
            }
          }
          return returns;
        }
        await check();
    "#;
    assert_eq!(run(source), Value::Number(1.0));
}

#[test]
fn for_await_does_not_close_after_next_rejection() {
    let source = r#"
        var reason = {};
        var returns = 0;
        var iterable = {
          [Symbol.asyncIterator]() {
            return {
              next() { return Promise.reject(reason); },
              return() { returns++; return Promise.resolve({}); }
            };
          }
        };
        async function check() {
          try { for await (var value of iterable) {} }
          catch (error) { return error === reason && returns === 0; }
        }
        await check();
    "#;
    assert_eq!(run(source), Value::Bool(true));
}

#[test]
fn async_generator_return_awaits_for_await_close() {
    let source = r#"
        var log = [];
        var iterable = {
          [Symbol.asyncIterator]() {
            return {
              next() { return Promise.resolve({ value: 1, done: false }); },
              return() {
                log.push("return");
                return Promise.resolve().then(function () {
                  log.push("closed");
                  return {};
                });
              }
            };
          }
        };
        async function* values() {
          for await (var value of iterable) yield value;
        }
        var iterator = values();
        await iterator.next();
        var result = await iterator.return("done");
        log.push(result.value);
        log.join("|");
    "#;
    assert_eq!(run(source), Value::String(Arc::from("return|closed|done")));
}

#[test]
fn for_await_close_observes_getter_call_and_thenable_errors() {
    let source = r#"
        var original = {};
        var closeError = {};
        function iterableWith(descriptor) {
          return {
            [Symbol.asyncIterator]() {
              var iterator = {
                next() { return Promise.resolve({ value: 1, done: false }); }
              };
              Object.defineProperty(iterator, "return", descriptor);
              return iterator;
            }
          };
        }
        async function throwing(descriptor) {
          try {
            for await (var value of iterableWith(descriptor)) throw original;
          } catch (error) { return error === original ? "original" : "wrong"; }
        }
        async function breaking(descriptor) {
          try {
            for await (var value of iterableWith(descriptor)) break;
            return "normal";
          } catch (error) { return error === closeError ? "close" : "wrong"; }
        }
        var getterThrow = { get() { throw closeError; } };
        var callThrow = { value() { throw closeError; } };
        var nullMethod = { value: null };
        var fulfilledThenable = {
          value() { return { then(resolve) { resolve({}); } }; }
        };
        var rejectedThenable = {
          value() { return { then(resolve, reject) { reject(closeError); } }; }
        };
        var throwingThenGetter = {
          value() {
            return Object.defineProperty({}, "then", {
              get() { throw closeError; }
            });
          }
        };
        [
          await throwing(getterThrow), await breaking(getterThrow),
          await throwing(callThrow), await breaking(callThrow),
          await breaking(nullMethod), await breaking(fulfilledThenable),
          await breaking(rejectedThenable), await breaking(throwingThenGetter)
        ].join("|");
    "#;
    assert_eq!(
        run(source),
        Value::String(Arc::from(
            "original|close|original|close|normal|normal|close|close"
        ))
    );
}

#[test]
fn async_generator_throw_preserves_original_over_close_rejection() {
    let source = r#"
        var original = {};
        var closeError = {};
        var iterable = {
          [Symbol.asyncIterator]() {
            return {
              next() { return Promise.resolve({ value: 1, done: false }); },
              return() { return Promise.reject(closeError); }
            };
          }
        };
        async function* values() {
          for await (var value of iterable) yield value;
        }
        var iterator = values();
        await iterator.next();
        try {
          await iterator.throw(original);
          false;
        } catch (error) {
          error === original;
        }
    "#;
    assert_eq!(run(source), Value::Bool(true));
}

#[test]
fn for_await_close_suspension_preserves_environment_across_gc() {
    let mut vm = Vm::new().expect("failed to initialize VM");
    vm.register_fn(
        "forceGc",
        |vm, _, _| {
            vm.gc();
            Ok(Value::Undefined)
        },
        0,
    )
    .expect("failed to register GC hook");

    assert_eq!(
        vm.run(
            r#"
            var log = [];
            async function check() {
              let held = { value: "alive" };
              let iterable = {
                [Symbol.asyncIterator]() {
                  return {
                    next() { return Promise.resolve({ value: 1, done: false }); },
                    return() {
                      forceGc();
                      return {
                        then(resolve) {
                          forceGc();
                          log.push(held.value);
                          resolve({});
                        }
                      };
                    }
                  };
                }
              };
              try {
                for await (let value of iterable) break;
                log.push("after-loop:" + held.value);
              } finally {
                log.push("finally:" + held.value);
              }
            }
            await check();
            log.join("|");
            "#,
        )
        .expect("for-await close state should survive GC"),
        Value::String(Arc::from("alive|after-loop:alive|finally:alive"))
    );
}
