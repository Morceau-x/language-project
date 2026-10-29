// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function wholeCells(width, cell) {
  return $rt.$idiv(width, cell);
}

export function halve(n) {
  return $rt.$idiv(n, 2);
}

export function main() {
  $rt.$log($rt.$idiv(7, 2));
  $rt.$log($rt.$idiv(-7, 2));
  $rt.$log($rt.$idiv(1, 0));
  $rt.$log(wholeCells(7.5, 2.0));
  $rt.$log(halve(9));
  return $rt.$log(halve(9.0));
}

