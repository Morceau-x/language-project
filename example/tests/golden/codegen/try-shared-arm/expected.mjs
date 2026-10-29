// ==== Main.mjs
import * as Option from "./Option.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function scale(o, r) {
  switch (o.$) {
    case "Some": {
      const k = o._0;
      if (k > 10) {
        return { $: "Ok", _0: k };
      } else {
        if (r.$ === "Err") {
          return r;
        }
        return { $: "Ok", _0: r._0 + 1 };
      }
    }
    default: {
      if (r.$ === "Err") {
        return r;
      }
      return { $: "Ok", _0: r._0 + 1 };
    }
  }
}

export function describe(o, r) {
  let $0;
  switch (o.$) {
    case "Some": {
      const k = o._0;
      if (k > 100) {
        $0 = { $: "Ok", _0: k };
      } else {
        if (r.$ === "Err") {
          return r;
        }
        $0 = { $: "Ok", _0: r._0 * 2 };
      }
      break;
    }
    default: {
      if (r.$ === "Err") {
        return r;
      }
      $0 = { $: "Ok", _0: r._0 * 2 };
      break;
    }
  }
  const base = $0;
  return base;
}

export function main() {
  $rt.$log(scale({ $: "Some", _0: 20 }, { $: "Ok", _0: 1 }));
  $rt.$log(scale({ $: "Some", _0: 2 }, { $: "Ok", _0: 5 }));
  $rt.$log(scale(Option.None, { $: "Err", _0: "nope" }));
  return $rt.$log(describe(Option.None, { $: "Ok", _0: 4 }));
}

