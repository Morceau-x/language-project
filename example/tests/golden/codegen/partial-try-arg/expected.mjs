// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function add3(a, b, c) {
  return a + b + c;
}

export function parse(s) {
  if (s.length > 0) {
    return { $: "Ok", _0: s.length };
  } else {
    return { $: "Err", _0: "empty" };
  }
}

export function build(s) {
  const $0 = parse(s);
  if ($0.$ === "Err") {
    return $0;
  }
  const $t0 = $0._0;
  return { $: "Ok", _0: (b, c) => add3($t0, b, c) };
}

export function main() {
  const $1 = build("hi");
  switch ($1.$) {
    case "Ok": {
      const f = $1._0;
      return $rt.$log(f(1, 2));
    }
    case "Err": {
      return $rt.$log(0 - 1);
    }
  }
}

