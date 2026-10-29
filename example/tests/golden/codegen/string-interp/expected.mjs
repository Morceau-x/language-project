// ==== Main.mjs
import * as $rt from "./maxim_runtime.mjs";

export function greet(name, count) {
  return "hello " + name + ", you have " + String(count) + " messages";
}

export function solo(s) {
  return s;
}

export function tail(s) {
  return s + "!";
}

export function label(flag) {
  return "answer: " + (flag ? "yes" : "no");
}

export function main() {
  $rt.$log(greet("max", 3));
  $rt.$log(solo("alone"));
  $rt.$log(tail("wow"));
  $rt.$log(label(true));
  return $rt.$log("literal ${ and lone $ stay put");
}

