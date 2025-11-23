(function () {
    const frame = document.getElementById("editor-frame");
    const fallback = document.getElementById("fallback");
    const status = document.getElementById("editor-status");

    function showFallback() {
        fallback.hidden = false;
        status.textContent = "エディタが見つかりません";
        status.classList.remove("status-success");
        status.classList.add("status-warning");
    }

    function markReady() {
        status.textContent = "エディタを読み込みました";
        status.classList.remove("status-warning");
        status.classList.add("status-success");
    }

    const timeout = window.setTimeout(showFallback, 1500);

    frame.addEventListener("load", () => {
        window.clearTimeout(timeout);
        try {
            const doc = frame.contentDocument;
            if (!doc || doc.body.children.length === 0) {
                showFallback();
                return;
            }
        } catch (error) {
            showFallback();
            return;
        }
        fallback.hidden = true;
        markReady();
    });
})();
