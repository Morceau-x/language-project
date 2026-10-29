// ==== Main.mjs
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function main() {
  return $rt.$log(
    List.sum(
      List.map(
        { $: "Cons", _0: 1, _1: { $: "Cons", _0: 2, _1: { $: "Cons", _0: 3, _1: { $: "Nil" } } } },
        (x) => x * x
      )
    )
  );
}

