// Pure evidence helpers; exercised without a browser or network.
const privateField = /credential|token|reclaim|password|secret|authorization|cookie/i;
export function redact(value, secrets = new Set()) {
  const collectLeaves = v => {
    if (typeof v === 'string' && v && v !== '[REDACTED]') secrets.add(v);
    else if (v && typeof v === 'object') Object.values(v).forEach(collectLeaves);
  };
  const discover = v => {
    if (!v || typeof v !== 'object') return;
    for (const [key, child] of Object.entries(v)) {
      if (privateField.test(key)) collectLeaves(child);
      else discover(child);
    }
  };
  discover(value); // Discover first: free-text logs may precede the private field.
  const scrub = v => {
    if (typeof v === 'string') {
      for (const secret of [...secrets].sort((a,b) => b.length-a.length)) v = v.split(secret).join('[REDACTED]');
      return v;
    }
    if (Array.isArray(v)) return v.map(scrub);
    if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([key,child]) => [key,privateField.test(key) ? '[REDACTED]' : scrub(child)]));
    return v;
  };
  return scrub(value);
}
export function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  // Preserve integer ticks/IDs/ack sequences; frame geometry is Rust f32.
  return typeof value === 'number' && !Number.isInteger(value) ? Math.fround(value) : value;
}
