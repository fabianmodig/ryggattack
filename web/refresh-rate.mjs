// Measure the empty page, before Bevy starts rendering. Game FPS is not a
// display-refresh signal: a slow GPU must still trigger quality reductions.
export function calibrateRefreshRate(env = globalThis) {
  return new Promise((resolve) => {
    const doc = env.document;
    if (!doc || doc.hidden || typeof env.requestAnimationFrame !== "function") {
      resolve(null);
      return;
    }
    let previous;
    let raf;
    let timer;
    let finished = false;
    const intervals = [];
    function finish(hz = null) {
      if (finished) return;
      finished = true;
      env.cancelAnimationFrame?.(raf);
      env.clearTimeout(timer);
      doc.removeEventListener("visibilitychange", visibility);
      resolve(hz);
    }
    function visibility() {
      if (doc.hidden) finish();
    }
    function schedule() {
      try { raf = env.requestAnimationFrame(frame); }
      catch { finish(); }
    }
    function frame(timestamp) {
      if (finished) return;
      if (doc.hidden || !Number.isFinite(timestamp)) { finish(); return; }
      if (previous !== undefined) {
        const dt = timestamp - previous;
        if (dt <= 0) { finish(); return; }
        intervals.push(dt);
      }
      previous = timestamp;
      if (intervals.length < 24) { schedule(); return; }
      // Fastest quartile avoids mistaking a few missed callbacks for a slower
      // display. Require most samples to agree; uncertain data falls back to
      // Rust's 60 Hz default rather than granting a slow GPU an easier target.
      intervals.sort((a, b) => a - b);
      const cadence = intervals.slice(0, 6).reduce((sum, dt) => sum + dt, 0) / 6;
      const consistent = intervals.filter((dt) => Math.abs(dt - cadence) <= cadence * 0.1).length;
      const hz = 1000 / cadence;
      finish(consistent >= 18 && hz >= 20 && hz <= 1000 ? Math.min(60, hz) : null);
    }
    doc.addEventListener("visibilitychange", visibility);
    // rAF can stop entirely in a hidden/throttled tab. Startup must not wait
    // forever; incomplete measurements are not calibration data.
    timer = env.setTimeout(() => finish(), 1500);
    schedule();
  });
}
