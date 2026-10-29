// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function scale(v, by, plus) {
  return v * by + plus;
}

export function double(n) {
  return n * 2;
}

export function main() {
  $rt.$log(double(3));
  $rt.$log(scale(3, 2, 1));
  $rt.$log(((plus) => scale(2, 1, plus))(3));
  $rt.$log(scale(3, 2, 1));
  const f = (plus) => scale(3, 2, plus);
  return $rt.$log(f(4));
}

