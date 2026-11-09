type A = "a";
type B = "b";

type X = `${A}_${B}`;
//          ^ defined: 1
//               ^ defined: 2

export {};
