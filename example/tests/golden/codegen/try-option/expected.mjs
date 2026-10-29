// ==== Main.mjs
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function firstTen(xs) {
  const $0 = List.head(xs);
  if ($0.$ === "None") {
    return $0;
  }
  const head = $0._0;
  return { $: "Some", _0: head * 10 };
}

export function main() {
  $rt.$log(firstTen({ $: "Cons", _0: 7, _1: { $: "Cons", _0: 8, _1: { $: "Nil" } } }));
  return $rt.$log(firstTen({ $: "Nil" }));
}

