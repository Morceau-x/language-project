// ==== Card.mjs
import * as Html from "./Html.mjs";

export function view(title, urgent, children) {
  return Html.node(
    "section",
    { $: "Cons", _0: Html.attr("class", urgent ? "card urgent" : "card"), _1: { $: "Nil" } },
    {
      $: "Cons",
      _0: Html.node("h2", { $: "Nil" }, { $: "Cons", _0: Html.text(title), _1: { $: "Nil" } }),
      _1: children
    }
  );
}

export const Card = { view: view };


// ==== Main.mjs
import * as Card from "./Card.mjs";
import * as Html from "./Html.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function main() {
  const $t0 = true;
  const $t1 = "Today";
  const $t2 = {
    $: "Cons",
    _0: Html.node(
      "p",
      { $: "Nil" },
      { $: "Cons", _0: Html.text("write the spec"), _1: { $: "Nil" } }
    ),
    _1: { $: "Nil" }
  };
  return $rt.$log(
    Html.toString(
      Html.node(
        "div",
        { $: "Nil" },
        {
          $: "Cons",
          _0: Card.view($t1, $t0, $t2),
          _1: { $: "Cons", _0: Card.view("Later", false, { $: "Nil" }), _1: { $: "Nil" } }
        }
      )
    )
  );
}

