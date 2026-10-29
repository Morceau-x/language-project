// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function classify(n) {
  const label = n < 0 ? "neg" : "pos";
  return label + "!";
}

export function main() {
  return $rt.$log(classify(3));
}

