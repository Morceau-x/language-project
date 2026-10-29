// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function greet(u) {
  return "hi " + u.name;
}

export function main() {
  return $rt.$log(greet({ $: "User", name: "Ada", age: 36 }));
}

