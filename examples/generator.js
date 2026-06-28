// Infinite lazy generators with manual take()
function* naturals() {
    let n = 1;
    while (true) yield n++;
}

function take(g, n) {
    let out = [];
    for (let i = 0; i < n; i++) out.push(g.next().value);
    return out;
}

console.log(take(naturals(), 5).join(", ")); // 1, 2, 3, 4, 5
