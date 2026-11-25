(() => {
    let current = "initial";
    let dirty = false;
    const clock = Object.freeze({
        get textContent() { return current; },
        set textContent(value) {
            if (typeof value !== "string" || value.length > 16384) {
                throw new TypeError("Invalid fixture text");
            }
            current = value;
            dirty = true;
        },
    });
    const document = Object.freeze({
        getElementById(id) { return id === "clock" ? clock : null; },
    });

    // A fixed callback and sample time keep this probe deterministic.
    function updateClock(displayTime) {
        const element = document.getElementById("clock");
        if (element === null || element.textContent !== "initial") {
            throw new Error("Clock binding missing");
        }
        element.textContent = displayTime;
        if (element.textContent !== displayTime) {
            throw new Error("Staged text was not visible");
        }
    }

    if (document.getElementById("missing") !== null) {
        throw new Error("Unknown element resolved");
    }
    updateClock("12:34");
    if (!dirty) {
        throw new Error("No staged update");
    }
    return current;
})()
