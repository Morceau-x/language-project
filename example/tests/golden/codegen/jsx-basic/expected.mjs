// ==== Main.mjs
import * as Html from "./Html.mjs";
import * as $rt from "./maxim_runtime.mjs";

export const Inc = { $: "Inc" };

export const Dec = { $: "Dec" };

export function view(count) {
  return Html.node(
    "div",
    { $: "Cons", _0: Html.attr("class", "counter"), _1: { $: "Nil" } },
    {
      $: "Cons",
      _0: Html.node(
        "button",
        { $: "Cons", _0: Html.on("click", Dec), _1: { $: "Nil" } },
        { $: "Cons", _0: Html.text("−"), _1: { $: "Nil" } }
      ),
      _1: {
        $: "Cons",
        _0: Html.node(
          "span",
          { $: "Nil" },
          { $: "Cons", _0: Html.text("count: " + String(count)), _1: { $: "Nil" } }
        ),
        _1: {
          $: "Cons",
          _0: Html.node(
            "button",
            { $: "Cons", _0: Html.on("click", Inc), _1: { $: "Nil" } },
            { $: "Cons", _0: Html.text("+"), _1: { $: "Nil" } }
          ),
          _1: { $: "Nil" }
        }
      }
    }
  );
}

export function main() {
  return $rt.$log(Html.toString(view(3)));
}

