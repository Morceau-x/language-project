// ==== Counter.mjs
import * as Html from "./Html.mjs";
import * as Option from "./Option.mjs";

export const Inc = { $: "Inc" };

export function init(props) {
  return { $: "Model", count: Option.withDefault(props.start, 0) };
}

export function update(model, msg) {
  switch (msg.$) {
    case "Inc": {
      return {
        $: "Step",
        _0: { $: "Model", count: model.count + 1 },
        _1: { $: "Cons", _0: { $: "Changed", _0: model.count + 1 }, _1: { $: "Nil" } }
      };
    }
  }
}

export function view(props, model) {
  return Html.node(
    "button",
    { $: "Cons", _0: Html.on("click", Inc), _1: { $: "Nil" } },
    { $: "Cons", _0: Html.text(String(model.count)), _1: { $: "Nil" } }
  );
}

export const Counter = { init: init, update: update, view: view, step: true };


// ==== Main.mjs
import * as Counter from "./Counter.mjs";
import * as Html from "./Html.mjs";
import * as Option from "./Option.mjs";
import * as $rt from "./maxim_runtime.mjs";

export function page() {
  return Html.node(
    "div",
    { $: "Nil" },
    {
      $: "Cons",
      _0: $rt.$instance(
        Counter.Counter,
        { $: "Props", start: { $: "Some", _0: 7 } },
        { changed: ($t0) => ({ $: "Grew", _0: $t0 }) }
      ),
      _1: {
        $: "Cons",
        _0: $rt.$instance(Counter.Counter, { $: "Props", start: Option.None }),
        _1: { $: "Nil" }
      }
    }
  );
}

export function main() {
  return $rt.$log(Html.toString(page()));
}

