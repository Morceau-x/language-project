// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function escape(s) {
  return s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

export function main() {
  $rt.$log("a-b-c".replaceAll("-", "+"));
  return $rt.$log(escape("<a & b>"));
}

