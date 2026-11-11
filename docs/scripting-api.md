# Prototype Scripting Contract

Status: M0 draft; API implementation is pending M2/M4.

Expose document.getElementById, textContent, allowlisted style writes, bounded setTimeout/setInterval and cancellation, pause/resume events, and wallpaper.time.format. Missing-element lookup returns null. Scripts may read their own writes within a callback; only acknowledged transactions are host-authoritative. Synchronous layout/computed-style APIs are excluded.

Timers use monotonic deadlines; formatting uses host-provided calendar time and explicit local timezone/locale settings. Pausing freezes animation progress and suspends callbacks. On resume, repeating overdue timers run at most once, then schedule from the current time; do not replay a backlog. A clock recomputes calendar text after resume. Time formatting tokens and invalid-format errors must be specified before the clock API is implemented.

Host budgets cover callbacks, microtasks, execution time, messages, and memory. A rejected mutation batch closes the session and preserves the last accepted scene. No filesystem, network, process, native modules, browser globals, or externally supplied bytecode. Resource limits cannot be raised by script.

Animation uses the host monotonic clock. A script write to a property under active native animation is rejected in the prototype; stopping/changing animations requires a later explicit API contract. All errors and logs are bounded. The initial API does not promise browser event-loop compatibility.
