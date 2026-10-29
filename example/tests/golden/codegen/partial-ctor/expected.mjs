// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function describe(u) {
  return u.name + " (" + String(u.age) + ")";
}

export function main() {
  const named = (age) => ({ $: "User", name: "John", age: age });
  const complete = named(30);
  $rt.$log(describe(complete));
  const copy = { ...complete };
  $rt.$log(describe(copy));
  const make = (name, age) => ({ $: "User", name: name, age: age });
  return $rt.$log(describe(make("Ada", 36)));
}

