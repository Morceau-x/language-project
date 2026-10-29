// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function base() {
  return 10;
}

export let table;
const base$1 = base() * 2;
table = base$1 + 1;

export function apply(e) {
  switch (e.$) {
    case "Add": {
      const a = e._0;
      const b = e._1;
      return combine(a, b, (x, y) => x + y);
    }
    case "Sub": {
      const a = e._0;
      const b = e._1;
      return combine(a, b, (x, y) => x - y);
    }
  }
}

export function combine(a, b, f) {
  return f(a, b);
}

export function pick(x) {
  switch (x.$) {
    case "Some": {
      const x$1 = x._0;
      return x$1;
    }
    case "None": {
      return 0;
    }
  }
}

export function grade(x) {
  switch (x.$) {
    case "Some": {
      const n = x._0;
      if (n > 100) {
        return n;
      } else {
        const n = x._0;
        return n + 1;
      }
    }
    case "None": {
      return 0;
    }
  }
}

export function stepped(n) {
  const f = (m) => {
    const y = m + n;
    return y * 2;
  };
  return f(1);
}

export function main() {
  $rt.$log(apply({ $: "Add", _0: 1, _1: 2 }));
  $rt.$log(apply({ $: "Sub", _0: 50, _1: 8 }));
  $rt.$log(table);
  $rt.$log(pick({ $: "Some", _0: 7 }));
  $rt.$log(grade({ $: "Some", _0: 7 }));
  return $rt.$log(stepped(3));
}

