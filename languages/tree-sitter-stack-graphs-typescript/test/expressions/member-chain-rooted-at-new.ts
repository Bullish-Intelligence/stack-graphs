class Leaf {
    b: number = 1;
}

class X {
    a: Leaf = new Leaf();
}

  new X().a.b;
//    ^ defined: 5
//        ^ defined: 6
//          ^ defined: 2

export {};
