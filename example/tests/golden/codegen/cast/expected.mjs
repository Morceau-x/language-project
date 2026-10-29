// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function floatToCelsius(d) {
  return { $: "Celsius", degrees: d };
}

export function celsiusToString(c) {
  return String(c.degrees) + " degrees";
}

export function main() {
  $rt.$log(1);
  $rt.$log($rt.$trunc(1.9));
  $rt.$log(String(42));
  $rt.$log(floatToCelsius(21.5));
  $rt.$log(celsiusToString(floatToCelsius(21.5)));
  return $rt.$log(7);
}

