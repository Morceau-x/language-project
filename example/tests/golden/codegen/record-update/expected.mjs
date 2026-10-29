// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function birthday(u) {
  return { ...u, age: u.age + 1 };
}

export function main() {
  return $rt.$log(birthday({ $: "User", name: "Ada", age: 36 }).age);
}

