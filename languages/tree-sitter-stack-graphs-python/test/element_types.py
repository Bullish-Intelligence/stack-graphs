# A container annotation gives the type of its elements to a loop, a comprehension, and a subscript.
from typing import Dict, List, Optional, Sequence
import typing

class Foo:
    def bar(self):
        pass

class Other:
    def baz(self):
        pass

def loops(a: list[Foo], b: List[Foo], c: Optional[list[Foo]], d: list[Foo] | None, e: typing.Sequence[Foo], f: tuple[Foo, ...], g: Optional[typing.List[Foo]], h: frozenset["Foo"]):
    for x in a:
        x.bar
        # ^ defined: 6
    for x in b:
        x.bar
        # ^ defined: 6
    for x in c:
        x.bar
        # ^ defined: 6
    for x in d:
        x.bar
        # ^ defined: 6
    for x in e:
        x.bar
        # ^ defined: 6
    for x in f:
        x.bar
        # ^ defined: 6
    for x in g:
        x.bar
        # ^ defined: 6
    for x in h:
        x.bar
        # ^ defined: 6

def comprehensions(a: list[Foo], m: dict[Other, Foo]):
    [x.bar for x in a]
    #  ^ defined: 6
    {x.bar for x in a}
    #  ^ defined: 6
    (x.bar for x in a)
    #  ^ defined: 6
    {x.baz: x for x in m}
    #  ^ defined: 10

# A loop variable without a marker is the loop variable only.
def plain(a: list[Foo]):
    for x in a:
        x
      # ^ defined: 51

# A subscript gives an element of a sequence and a value of a mapping.
def subscripts(a: list[Foo], m: dict[str, Foo], n: Dict[str, List[Foo]], o: typing.Mapping[str, Foo], p: Optional[Dict[str, Foo]]):
    a[0].bar
    #    ^ defined: 6
    m["k"].bar
    #      ^ defined: 6
    n["k"][0].bar
    #         ^ defined: 6
    o["k"].bar
    #      ^ defined: 6
    p["k"].bar
    #      ^ defined: 6
    a[1:].bar
    #     ^ defined:
    m["k"].baz
    #      ^ defined:

# A variable, an attribute, and a return annotation.
items: list[Foo] = []
for item in items:
    item.bar
    #    ^ defined: 6

def make() -> list[Foo]:
    pass

for made in make():
    made.bar
    #    ^ defined: 6

class Holder:
    members: List[Foo]

    def __init__(self):
        self.table: dict[str, Other] = {}

    def use(self):
        for member in self.members:
            member.bar
            #      ^ defined: 6
        self.table["k"].baz
        #               ^ defined: 10

# A class without an element type gives no element.
def other(a: Foo, b: list):
    for x in a:
        x.bar
        # ^ defined:
    for y in b:
        y.bar
        # ^ defined:

# Tuple patterns read the positions of a tuple type, of enumerate, and of zip.
def unpacking(pairs: list[tuple[Other, Foo]], a: list[Foo], b: list[Other]):
    for o, f in pairs:
        o.baz
        # ^ defined: 10
        f.bar
        # ^ defined: 6
    for i, x in enumerate(a):
        x.bar
        # ^ defined: 6
    for x, y in zip(a, b):
        x.bar
        # ^ defined: 6
        y.baz
        # ^ defined: 10
        y.bar
        # ^ defined:
    for x in sorted(a):
        x.bar
        # ^ defined: 6
    reversed(b)[0].baz
    #              ^ defined: 10

# The methods of a mapping give its keys and values.
def methods(m: dict[Other, Foo], n: Optional[Dict[Other, Foo]]):
    for k in m.keys():
        k.baz
        # ^ defined: 10
    for v in m.values():
        v.bar
        # ^ defined: 6
    for k, v in m.items():
        k.baz
        # ^ defined: 10
        v.bar
        # ^ defined: 6
    for k, v in n.items():
        v.bar
        # ^ defined: 6
    m.get(k).bar
    #        ^ defined: 6
    n.pop(k).bar
    #        ^ defined: 6

# The elements of `*args` and the values of `**kwargs`.
def splats(*args: Foo, **kwargs: Other):
    for a in args:
        a.bar
        # ^ defined: 6
    args[0].bar
    #       ^ defined: 6
    kwargs["k"].baz
    #           ^ defined: 10
