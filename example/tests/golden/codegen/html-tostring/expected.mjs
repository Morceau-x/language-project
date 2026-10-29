// ==== Main.mjs
import * as Html from "./Html.mjs";
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export const Clicked = { $: "Clicked" };

export function page(items) {
  return Html.node(
    "div",
    { $: "Cons", _0: Html.attr("class", "app"), _1: { $: "Nil" } },
    {
      $: "Cons",
      _0: Html.node(
        "button",
        { $: "Cons", _0: Html.on("click", Clicked), _1: { $: "Nil" } },
        { $: "Cons", _0: Html.text("go"), _1: { $: "Nil" } }
      ),
      _1: {
        $: "Cons",
        _0: Html.node("ul", { $: "Nil" }, List.map(items, item)),
        _1: { $: "Cons", _0: Html.node("br", { $: "Nil" }, { $: "Nil" }), _1: { $: "Nil" } }
      }
    }
  );
}

export function item(label) {
  return Html.node("li", { $: "Nil" }, { $: "Cons", _0: Html.text(label), _1: { $: "Nil" } });
}

export function main() {
  return $rt.$log(
    Html.toString(
      page({ $: "Cons", _0: "a & b", _1: { $: "Cons", _0: "<script>", _1: { $: "Nil" } } })
    )
  );
}

