// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function parse(s) {
  if (s.length > 0) {
    return { $: "Ok", _0: s.length };
  } else {
    return { $: "Err", _0: "empty input" };
  }
}

export function total(a, b) {
  const $0 = parse(a);
  if ($0.$ === "Err") {
    return $0;
  }
  const x = $0._0;
  const $1 = parse(b);
  if ($1.$ === "Err") {
    return $1;
  }
  const y = $1._0;
  return { $: "Ok", _0: x + y };
}

export function main() {
  $rt.$log(total("hi", "maxim"));
  return $rt.$log(total("", "maxim"));
}

