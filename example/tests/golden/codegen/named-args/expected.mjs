// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function scale(dx, dy) {
  return dx * 10 + dy;
}

export function main() {
  $rt.$log(scale(1, 2));
  const $t0 = $rt.$log(2);
  const $t1 = $rt.$log(1);
  $rt.$log(scale($t1, $t0));
  const bump = (amount) => amount + 1;
  return $rt.$log(bump(41));
}

