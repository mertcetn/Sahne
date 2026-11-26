(() => {
    const { create, freeze, setPrototypeOf } = Object;
    const isArray = Array.isArray;
    const isInteger = Number.isInteger;
    const SceneError = Error;
    const SceneRangeError = RangeError;
    const SceneTypeError = TypeError;
    return (bindings, callback, displayTime) => {
        const ids = create(null);
        const states = setPrototypeOf([], null);
        let editCount = 0;
        let operations = 0;
        let budgetError;
        let active = true;
        function countOperation() {
            if (!active) throw new SceneError("Scene callback has ended");
            if (++operations > 256) budgetError ??= "Too many DOM operations";
            if (budgetError) throw new SceneRangeError(budgetError);
        }
        function reserve(dirty) {
            if (!dirty && ++editCount > 64) {
                budgetError = "Too many scene edits";
                throw new SceneRangeError(budgetError);
            }
        }
        for (let index = 0; index < bindings.length; index++) {
            const entry = bindings[index];
            const name = entry[0], initialText = entry[1], initialColor = entry[2];
            if (ids[name] !== undefined) throw new SceneError("Duplicate element binding");
            const state = { text: initialText, color: initialColor, textDirty: false, colorDirty: false };
            states[index] = state;
            const style = freeze({
                get color() {
                    countOperation();
                    return [state.color[0], state.color[1], state.color[2], state.color[3]];
                },
                set color(value) {
                    countOperation();
                    if (!isArray(value) || value.length !== 4) throw new SceneTypeError("Expected RGBA");
                    const copy = [value[0], value[1], value[2], value[3]];
                    for (let i = 0; i < 4; i++) {
                        if (!isInteger(copy[i]) || copy[i] < 0 || copy[i] > 255) {
                            throw new SceneTypeError("Expected RGBA bytes");
                        }
                    }
                    reserve(state.colorDirty);
                    state.color = copy;
                    state.colorDirty = true;
                },
            });
            const element = freeze({
                get textContent() {
                    countOperation();
                    return typeof initialText === "string" ? state.text : null;
                },
                set textContent(value) {
                    countOperation();
                    if (typeof initialText !== "string" || typeof value !== "string" || value.length > 16384) {
                        throw new SceneTypeError("Invalid text write");
                    }
                    reserve(state.textDirty);
                    state.text = value;
                    state.textDirty = true;
                },
                get style() {
                    countOperation();
                    return style;
                },
            });
            ids[name] = element;
        }
        const document = freeze({
            getElementById(id) {
                countOperation();
                return typeof id === "string" ? (ids[id] ?? null) : null;
            },
        });
        const result = callback(document, displayTime);
        return [result, () => {
            active = false;
            if (budgetError) throw new SceneRangeError(budgetError);
            const edits = setPrototypeOf([], null);
            for (let index = 0; index < states.length; index++) {
                const state = states[index];
                if (state.textDirty) edits[edits.length] = [index, 1, state.text];
                if (state.colorDirty) edits[edits.length] = [index, 2, state.color];
            }
            return edits;
        }];
    };
})()
