type U = string;

function f<T>(x: T): T { return x; }

const g = f<U>;
//        ^ defined: 3
//          ^ defined: 1

export {};
