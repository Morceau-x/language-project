// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function add3(a, b, c) {
  return a + b + c;
}

export function expensive(n) {
  return $rt.$log(n);
}

export function main() {
  const add12 = (c) => add3(1, 2, c);
  $rt.$log(add12(3));
  const addB = (a, c) => add3(a, 20, c);
  $rt.$log(addB(1, 300));
  const f = (b, c) => add3(1, b, c);
  const g = (c) => f(2, c);
  $rt.$log(g(3));
  const $t6 = expensive(5);
  const h = (c) => add3($t6, 6, c);
  return $rt.$log(h(7));
}

