// ==== Main.mjs
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function total(xs) {
  return List.sum(
    List.map(
      xs,
      (x) => {
        const doubled = x * 2;
        return doubled + 1;
      }
    )
  );
}

export function main() {
  return $rt.$log(
    total(
      { $: "Cons", _0: 1, _1: { $: "Cons", _0: 2, _1: { $: "Cons", _0: 3, _1: { $: "Nil" } } } }
    )
  );
}

