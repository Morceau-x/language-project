// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function wrap(n, m) {
  return $rt.$mod(n, m);
}

export function main() {
  $rt.$log(7 % 2);
  $rt.$log(-7 % 2);
  $rt.$log($rt.$mod(5, 0));
  $rt.$log(wrap(10, 3));
  $rt.$log(7.5 % 2.0);
  return $rt.$log(-7.5 % 2.0);
}

