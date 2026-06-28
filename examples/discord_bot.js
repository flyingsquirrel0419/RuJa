// A Discord bot skeleton with a command dispatcher.
//
// RuJa has no network stack yet, so this is an offline simulation: messages
// are queued and processed through a command registry, exactly the way a real
// gateway handler would route them once a transport layer exists. The command
// system, argument parsing, and plugin-style registration are all real.
//
// Run:  cargo run --release -- examples/discord_bot.js

// --- Command framework ------------------------------------------------------

// A Command is: name, description, and a handler(ctx) -> reply string.
// ctx carries everything a handler needs: author, channel, args.
function Command(name, description, handler) {
    return { name: name, description: description, handler: handler };
}

// Split on one or more spaces, dropping empties (handles padding).
function splitArgs(s) {
    let out = [];
    let cur = "";
    for (let i = 0; i < s.length; i++) {
        if (s[i] === " ") {
            if (cur.length > 0) { out.push(cur); cur = ""; }
        } else {
            cur = cur + s[i];
        }
    }
    if (cur.length > 0) out.push(cur);
    return out;
}

// A Bot owns a command table and counts processed events.
class Bot {
    constructor(name) {
        this.name = name;
        this.commands = {};
        this.events = 0;
    }

    // Register a command. Multiple commands build up a plugin table.
    register(cmd) {
        this.commands[cmd.name] = cmd;
        return this;
    }

    // Parse a raw message string into { author, channel, args }.
    // Message format:  @author #channel /command arg1 arg2 ...
    // Both @author and #channel are optional; args may be empty.
    parse(raw) {
        let author = "unknown";
        let channel = "general";
        let tokens = splitArgs(raw);

        // consume a leading @author token
        let i = 0;
        if (tokens.length > 0 && tokens[0][0] === "@") {
            author = tokens[0].slice(1);
            i = 1;
        }
        // consume a leading #channel token
        if (tokens.length > i && tokens[i][0] === "#") {
            channel = tokens[i].slice(1);
            i = i + 1;
        }
        let args = [];
        while (i < tokens.length) { args.push(tokens[i]); i++; }
        return { author: author, channel: channel, args: args };
    }

    // Dispatch one parsed message. Returns a reply string or null.
    handle(ctx) {
        this.events++;
        if (ctx.args.length === 0) return null;
        let first = ctx.args[0];
        if (first[0] !== "/") return null;
        let name = first.slice(1);
        let cmd = this.commands[name];
        if (!cmd) return "Unknown command: /" + name + ". Try /help.";
        let callCtx = {
            author: ctx.author,
            channel: ctx.channel,
            args: ctx.args.slice(1),
            bot: this,
        };
        return cmd.handler(callCtx);
    }
}

// --- Built-in commands ------------------------------------------------------

// /ping — latency check (simulated; no clock API yet, so a tick counter)
const ping = Command("ping", "Check if the bot is alive", function (ctx) {
    let latency = ctx.bot.events % 50; // simulated ms
    return "Pong! " + latency + "ms";
});

// /help — list every registered command
const help = Command("help", "List available commands", function (ctx) {
    let cmds = ctx.bot.commands;
    let names = [];
    let i = 0;
    for (let k in cmds) {
        names[i] = "/" + k;
        i++;
    }
    return "Commands: " + names.join(", ");
});

// /echo <text> — echo back what you say
const echo = Command("echo", "Repeat what you say", function (ctx) {
    return ctx.author + " said: " + ctx.args.join(" ");
});

// /roll [n] — roll a d6 or an n-sided die
const roll = Command("roll", "Roll a die (default d6)", function (ctx) {
    let sides = ctx.args.length > 0 ? parseInt(ctx.args[0]) : 6;
    if (isNaN(sides) || sides < 1) sides = 6;
    // Pseudo-random from event count (no Math.random yet)
    let seed = ctx.bot.events * 7 + 3;
    let result = (seed % sides) + 1;
    return "rolled a d" + sides + " -> " + result;
});

// --- Wire up the bot --------------------------------------------------------

const bot = new Bot("RuJa-Bot")
    .register(ping)
    .register(help)
    .register(echo)
    .register(roll);

// --- Simulated gateway: a queue of incoming messages -----------------------

const messages = [
    "@alice #general /ping",
    "@bob   #general /help",
    "@carol #general /echo hello world",
    "@alice #gaming    /roll",
    "@bob   #general /roll 20",
    "@dave  #general /ping",
    "@eve   #general /unknowncmd",
    "@alice #general /ping",
];

console.log("=== " + bot.name + " starting ===");
console.log("");

for (let i = 0; i < messages.length; i++) {
    let ctx = bot.parse(messages[i]);
    let reply = bot.handle(ctx);
    if (reply !== null) {
        console.log("[" + ctx.channel + "] " + ctx.author + " -> " + reply);
    }
}

console.log("");
console.log("=== processed " + bot.events + " events ===");
