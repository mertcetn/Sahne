(schedule, cancel) => {
    const callbacks = Object.create(null);
    let total = 0;
    function register(callback, delay, repeating) {
        if (typeof callback !== "function" || !Number.isInteger(delay) ||
            delay < 16 || delay > 86400000) throw new RangeError("Invalid timer");
        const id = schedule(delay, repeating);
        if (id === 0) throw new RangeError("Scheduling failed");
        callbacks[id] = { callback, repeating };
        return id;
    }
    const setTimeout = (callback, delay) => register(callback, delay, false);
    const setInterval = (callback, delay) => register(callback, delay, true);
    const clearTimeout = (id) => {
        if (cancel(id)) delete callbacks[id];
    };
    const clearInterval = clearTimeout;
    let rejected = false;
    try { setTimeout(() => {}, 15); } catch (error) { rejected = error instanceof RangeError; }
    if (!rejected) throw new Error("Out-of-range delay accepted");
    if (setTimeout(() => { total += 1; }, 16) !== 1) throw new Error("Unexpected timeout handle");
    clearTimeout(setTimeout(() => { throw new Error("Cancelled callback"); }, 16));
    const interval = setInterval(() => { total += 10; clearInterval(interval); }, 16);
    if (interval !== 3) throw new Error("Unexpected interval handle");
    return (id) => {
        const entry = callbacks[id];
        if (!entry) throw new Error("Unknown timer");
        if (!entry.repeating) delete callbacks[id];
        entry.callback();
        return total;
    };
}
