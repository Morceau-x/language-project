// ==== Main.mjs
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function evaluate(e) {
  switch (e.$) {
    case "Num": {
      const n = e._0;
      return n;
    }
    case "Add": {
      const a = e._0;
      const b = e._1;
      return evaluate(a) + evaluate(b);
    }
    case "Sub": {
      const a = e._0;
      const b = e._1;
      return evaluate(a) - evaluate(b);
    }
    case "Mul": {
      const a = e._0;
      const b = e._1;
      return evaluate(a) * evaluate(b);
    }
    case "Div": {
      const a = e._0;
      const b = e._1;
      return evaluate(a) / evaluate(b);
    }
  }
}

export const exprs = {
  $: "Cons",
  _0: {
    $: "Mul",
    _0: { $: "Add", _0: { $: "Num", _0: 1.0 }, _1: { $: "Num", _0: 2.0 } },
    _1: { $: "Num", _0: 3.0 }
  },
  _1: {
    $: "Cons",
    _0: {
      $: "Sub",
      _0: { $: "Num", _0: 10.0 },
      _1: { $: "Div", _0: { $: "Num", _0: 9.0 }, _1: { $: "Num", _0: 2.0 } }
    },
    _1: {
      $: "Cons",
      _0: { $: "Div", _0: { $: "Num", _0: 10.0 }, _1: { $: "Num", _0: 4.0 } },
      _1: { $: "Nil" }
    }
  }
};

export function main() {
  return $rt.$log(List.sum(List.map(exprs, evaluate)));
}

