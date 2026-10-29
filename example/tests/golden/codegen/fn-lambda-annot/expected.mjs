// ==== Main.mjs
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function findEven(xs) {
  return List.find(xs, (x) => $rt.$eq(x % 2, 0));
}

export function describe(xs) {
  const fmt = (extra) => {
    const $0 = findEven(xs);
    if ($0.$ === "None") {
      return $0;
    }
    const n = $0._0;
    return { $: "Some", _0: String(n) + " (+" + String(extra) + ")" };
  };
  return fmt(9);
}

export function main() {
  $rt.$log(
    describe(
      { $: "Cons", _0: 1, _1: { $: "Cons", _0: 4, _1: { $: "Cons", _0: 9, _1: { $: "Nil" } } } }
    )
  );
  return $rt.$log(
    describe(
      { $: "Cons", _0: 1, _1: { $: "Cons", _0: 3, _1: { $: "Cons", _0: 9, _1: { $: "Nil" } } } }
    )
  );
}

