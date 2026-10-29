// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function arguments$() {
  return 41;
}

export function eval$(x) {
  return x + 1;
}

export function main() {
  return $rt.$log(eval$(arguments$()));
}

