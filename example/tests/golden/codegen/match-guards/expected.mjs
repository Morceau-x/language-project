// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function describe(s) {
  switch (s.$) {
    case "Circle": {
      const r = s._0;
      if (r > 10.0) {
        return "big circle";
      } else {
        return "circle";
      }
    }
    case "Rect": {
      const w = s._0;
      const h = s._1;
      if ($rt.$eq(w, h)) {
        return "square";
      } else {
        return "rect";
      }
    }
  }
}

export function main() {
  return $rt.$log(describe({ $: "Rect", _0: 2.0, _1: 2.0 }));
}

