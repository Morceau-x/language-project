// ==== Main.mjs
import * as Html from "./Html.mjs";
import * as List from "./List.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function item(label) {
  return Html.node("li", { $: "Nil" }, { $: "Cons", _0: Html.text(label), _1: { $: "Nil" } });
}

export function view(header, items, extra) {
  return Html.node(
    "ul",
    { $: "Cons", _0: Html.attr("id", "all"), _1: { $: "Nil" } },
    {
      $: "Cons",
      _0: header,
      _1: List.append(
        List.map(items, item),
        {
          $: "Cons",
          _0: Html.node(
            "li",
            { $: "Nil" },
            { $: "Cons", _0: Html.text("interleaved"), _1: { $: "Nil" } }
          ),
          _1: extra
        }
      )
    }
  );
}

export function main() {
  return $rt.$log(
    Html.toString(
      view(
        Html.node("li", { $: "Nil" }, { $: "Cons", _0: Html.text("first"), _1: { $: "Nil" } }),
        { $: "Cons", _0: "a", _1: { $: "Cons", _0: "b", _1: { $: "Nil" } } },
        {
          $: "Cons",
          _0: Html.node("li", { $: "Nil" }, { $: "Cons", _0: Html.text("z"), _1: { $: "Nil" } }),
          _1: { $: "Nil" }
        }
      )
    )
  );
}

