import fs from "node:fs";
import path from "node:path";
import { gzipSync, brotliCompressSync } from "node:zlib";
const root = path.resolve(import.meta.dirname, "../../web");
const read = (file) =>
  JSON.parse(fs.readFileSync(path.join(root, file), "utf8"));
const fail = (message) => {
  throw new Error(message);
};
switch (process.argv[2]) {
  case "locales": {
    const en = read("src/locales/en.json");
    const pt = read("src/locales/pt-BR.json");
    if (
      JSON.stringify(Object.keys(en).sort()) !==
      JSON.stringify(Object.keys(pt).sort())
    )
      fail("Locale keys differ");
    for (const key of Object.keys(en)) {
      if (!en[key].trim() || !pt[key].trim()) fail(`Empty translation: ${key}`);
      const parameters = (v) => JSON.stringify(v.match(/{{[^}]+}}/g) ?? []);
      if (parameters(en[key]) !== parameters(pt[key]))
        fail(`Interpolation differs: ${key}`);
    }
    console.log(`Locale parity: ${Object.keys(en).length} translated keys.`);
    break;
  }
  case "coverage": {
    const report = read("coverage/coverage-summary.json");
    const sources = fs
      .readdirSync(path.join(root, "src"), { recursive: true })
      .filter((f) => /\.(ts|tsx)$/.test(f));
    for (const file of sources)
      if (!report[path.join(root, "src", file)])
        fail(`Missing coverage: ${file}`);
    for (const metric of ["lines", "branches"]) {
      const v = report.total[metric];
      if (!v.total || v.covered / v.total <= 0.9)
        fail(`Web ${metric} must exceed 90%`);
    }
    console.log(
      JSON.stringify(
        {
          component: "web",
          files: sources.length,
          lines: report.total.lines,
          branches: report.total.branches,
        },
        null,
        2,
      ),
    );
    break;
  }
  case "bundle": {
    const totals = {
      js: { raw: 0, gzip: 0, brotli: 0 },
      css: { raw: 0, gzip: 0, brotli: 0 },
    };
    for (const file of fs.readdirSync(path.join(root, "dist/assets"))) {
      const kind = path.extname(file).slice(1);
      if (!(kind in totals)) continue;
      const data = fs.readFileSync(path.join(root, "dist/assets", file));
      totals[kind].raw += data.length;
      totals[kind].gzip += gzipSync(data).length;
      totals[kind].brotli += brotliCompressSync(data).length;
    }
    if (totals.js.gzip > 150 * 1024 || totals.css.gzip > 10 * 1024)
      fail("Compressed bundle budget exceeded");
    console.log(
      JSON.stringify({ baseline: 0, bytesAndDelta: totals }, null, 2),
    );
    break;
  }
  default:
    fail("Expected locales, coverage or bundle");
}
