// Promise chaining with then/catch
Promise.resolve(1)
    .then(function (v) { return v * 2; })
    .then(function (v) { return v + 10; })
    .then(function (v) { console.log("result:", v); }); // result: 12
