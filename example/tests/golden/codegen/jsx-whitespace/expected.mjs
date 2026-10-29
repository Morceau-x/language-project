// ==== Main.mjs
import * as Html from "./Html.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function view(name) {
  return Html.node(
    "p",
    { $: "Nil" },
    {
      $: "Cons",
      _0: Html.text("hello   " + name + ", welcome "),
      _1: {
        $: "Cons",
        _0: Html.node("b", { $: "Nil" }, { $: "Cons", _0: Html.text("back"), _1: { $: "Nil" } }),
        _1: { $: "Cons", _0: Html.text(" today"), _1: { $: "Nil" } }
      }
    }
  );
}

export function main() {
  return $rt.$log(Html.toString(view("max")));
}

