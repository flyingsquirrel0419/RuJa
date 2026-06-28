// async/await with Promise resolution
async function fetchUser(id) {
    return { id, name: "user-" + id };
}

async function main() {
    let user = await fetchUser(42);
    console.log(`loaded ${user.name} (${user.id})`);
}

main(); // loaded user-42 (42)
