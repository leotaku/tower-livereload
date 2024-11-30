(() => {
  const inputs = document.currentScript.dataset;

  const controller = new AbortController();
  var unloaded = false;
  addEventListener("beforeunload", () => {
    unloaded = true;
    controller.abort();
  });

  const retry = (url) => {
    const controller = new AbortController();
    setTimeout(() => controller.abort(), 500);
    fetch(url, { cache: "no-store", signal: controller.signal })
      .then((resp) => {
        if (!resp.ok) return Promise.reject();
        console.log("[tower-livereload] reload...");
        window.location.reload();
      })
      .catch(() => setTimeout(() => retry(url), inputs.reloadInterval));
  };

  const main = () =>
    fetch(inputs.longPoll, { cache: "no-store", signal: controller.signal })
      .then((rsp) => rsp.text())
      .catch(() => null)
      .then(() => {
        console.log("[tower-livereload] disconnected...");
        if (!unloaded) retry(inputs.backUp);
      });

  console.log("[tower-livereload] connected...");
  main();
})();
