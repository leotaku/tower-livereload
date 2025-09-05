(() => {
  const inputs = document.currentScript.dataset;

  addEventListener("pageshow", () => {
    const source = new EventSource(inputs.eventStream);
    source.addEventListener("reload", () => {
      source.close();
      window.location.reload();
    });

    const onerror = () => {
      source.removeEventListener("error", onerror);
      source.addEventListener("init", () => {
        source.close();
        window.location.reload();
      });
    };

    source.addEventListener("error", onerror);
  });
})();
