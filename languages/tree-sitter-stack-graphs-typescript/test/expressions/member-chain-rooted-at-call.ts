class Leaf {
    baz: number = 1;
}

class Mid {
    bar: Leaf = new Leaf();
}

function foo(): Mid { return new Mid(); }

  foo().bar.baz;
//^ defined: 9
//      ^ defined: 6
//          ^ defined: 2

export {};
