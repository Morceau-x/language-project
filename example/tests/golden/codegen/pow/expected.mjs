// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function main() {
  const x = 2.0;
  const y = 3.0;
  const z = 2.0;
  $rt.$log(x ** y);
  $rt.$log(-(x ** y));
  $rt.$log(x ** -y);
  $rt.$log(x ** y ** z);
  $rt.$log((x ** y) ** z);
  $rt.$log((x + y) ** z);
  $rt.$log(x * y ** z);
  $rt.$log(x ** y * z);
  $rt.$log(2 ** 10);
  return $rt.$log(2 ** -1);
}

