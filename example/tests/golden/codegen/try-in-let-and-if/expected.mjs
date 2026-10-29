// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function double(r, flag) {
  if (flag) {
    if (r.$ === "Err") {
      return r;
    }
    const n = r._0;
    return { $: "Ok", _0: n * 2 };
  } else {
    return r;
  }
}

export function main() {
  $rt.$log(double({ $: "Ok", _0: 21 }, true));
  $rt.$log(double({ $: "Err", _0: "no" }, true));
  return $rt.$log(double({ $: "Ok", _0: 21 }, false));
}

