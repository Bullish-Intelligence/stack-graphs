class Leaf {
    v: number = 1;
}

class C {
    leaf: Leaf = new Leaf();

    m() {
        let w: typeof this.leaf.v;
        //                 ^ defined: 6
        //                      ^ defined: 2
    }
}

export {};
