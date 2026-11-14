// 把引擎編譯成 WebAssembly，並把資料檔放到網頁可以下載的位置。
// 由 `npm run dev` 與 `npm run build` 自動呼叫。

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const web = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(web, "..");

execFileSync(
  "cargo",
  ["build", "--release", "--target", "wasm32-unknown-unknown", "-p", "mofoim-wasm"],
  { cwd: root, stdio: "inherit" },
);
mkdirSync(join(web, "src", "wasm"), { recursive: true });
copyFileSync(
  join(root, "target", "wasm32-unknown-unknown", "release", "mofoim_wasm.wasm"),
  join(web, "src", "wasm", "mofoim.wasm"),
);

const data = join(root, "data", "mofoim.dat");
if (!existsSync(data)) {
  console.error("找不到 data/mofoim.dat，請先依 README 建置資料檔。");
  process.exit(1);
}
mkdirSync(join(web, "public"), { recursive: true });
copyFileSync(data, join(web, "public", "mofoim.dat"));
