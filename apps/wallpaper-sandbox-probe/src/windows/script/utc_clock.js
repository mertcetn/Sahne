(clock) => {
    let paused = false;
    return {
        pause() {
            if (paused) throw new Error("Clock already paused");
            paused = true;
        },
        resume() {
            if (!paused) throw new Error("Clock already running");
            paused = false;
        },
        tick(document, time) {
            if (paused) throw new Error("Clock tick during pause");
            clock(document, time);
        },
    };
}
