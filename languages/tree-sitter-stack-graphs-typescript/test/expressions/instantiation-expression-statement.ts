type U = string;
type T<V> = V;

function f<A>(x: A): A { return x; }
const x: U = "a";

  f(x) satisfies T<U>
//^ defined: 4
//  ^ defined: 5
//               ^ defined: 2
//                 ^ defined: 1

  f<U>
//^ defined: 4
//  ^ defined: 1

export {};
