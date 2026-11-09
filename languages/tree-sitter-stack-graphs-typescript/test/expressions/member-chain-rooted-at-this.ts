class Inner {
    get(k: string): number { return 1; }
}

class C {
    _map: Inner = new Inner();

    m(k: string) {
        return this._map.get(k);
        //          ^ defined: 6
        //                ^ defined: 2
    }
}

export {};
