// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function area(s) {
  switch (s.$) {
    case "Circle": {
      const r = s._0;
      return 3.14 * r * r;
    }
    case "Rect": {
      const w = s._0;
      const h = s._1;
      return w * h;
    }
  }
}

export function main() {
  return $rt.$log(area({ $: "Circle", _0: 1.0 }));
}

