(document, displayTime) => {
    const clock = document.getElementById("clock");
    const accent = document.getElementById("accent");
    if (clock === null || accent === null ||
        document.getElementById("missing") !== null ||
        document.getElementById("__proto__") !== null ||
        clock !== document.getElementById("clock") ||
        typeof clock.textContent !== "string") {
        throw new Error("Unexpected element binding");
    }
    clock.textContent = "staged";
    clock.textContent = displayTime;
    const color = [36, 179, 144, 255];
    accent.style.color = color;
    color[0] = 0;
    const observed = accent.style.color;
    if (clock.textContent !== displayTime || observed[0] !== 36) {
        throw new Error("Staged update did not retain its value");
    }
}
