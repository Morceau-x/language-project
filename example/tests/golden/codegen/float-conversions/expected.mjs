// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export const toInt = ($t0) => $rt.$trunc($t0);

export function main() {
  $rt.$log($rt.$trunc(7.9));
  $rt.$log($rt.$trunc(-7.9));
  $rt.$log($rt.$floor(-7.1));
  $rt.$log($rt.$ceil(7.1));
  $rt.$log($rt.$round(2.5));
  $rt.$log($rt.$round(-2.5));
  $rt.$log(toInt(3.7));
  $rt.$log($rt.$trunc(1.9));
  $rt.$log(3);
  $rt.$log(String(42));
  return $rt.$log(String(2.5));
}

