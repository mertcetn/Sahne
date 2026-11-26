(register, clearInterval, clock) => {
    let callback;
    function setInterval(fn, delay) {
        const handle = register(delay);
        if (handle === 0) throw new Error("Clock interval registration failed");
        callback = fn;
        return handle;
    }
    let calls = 0;
    let phase = "running";
    const handle = setInterval((document, time) => {
        if (phase !== "resumed") throw new Error("Tick before resume callback");
        clock(document, time);
        if (++calls === 3) clearInterval(handle);
    }, 32);
    return {
        tick: callback,
        pause() {
            if (phase !== "running" || calls !== 0) throw new Error("Unexpected pause callback");
            phase = "paused";
        },
        resume() {
            if (phase !== "paused" || calls !== 0) throw new Error("Unexpected resume callback");
            phase = "resumed";
        },
    };
}
