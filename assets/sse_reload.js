(() => {
  const inputs = document.currentScript.dataset;
  let source;

  const cleanup = () => {
    if (source) {
      source.close();
      source = null;
    }
  };

  addEventListener("pageshow", () => {
    if (source) return;

    source = new EventSource(inputs.eventStream);

    source.addEventListener("reload", () => {
      cleanup();
      window.location.reload();
    });

    const onerror = () => {
      source.removeEventListener("error", onerror);
      source.addEventListener("init", () => {
        cleanup();
        window.location.reload();
      });
    };

    source.addEventListener("error", onerror);
  });

  addEventListener("pagehide", () => {
    cleanup();
  });
})();
