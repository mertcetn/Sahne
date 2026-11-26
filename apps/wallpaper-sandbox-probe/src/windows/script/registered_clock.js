(register, clearInterval, clock) => {
    let callback;
    function setInterval(fn, delay) {
        const handle = register(delay);
        if (handle === 0) throw new Error("Clock interval registration failed");
        callback = fn;
        return handle;
    }
    let calls = 0;
    const handle = setInterval((document, time) => {
        clock(document, time);
        if (++calls === 3) clearInterval(handle);
    }, 32);
    return callback;
}
