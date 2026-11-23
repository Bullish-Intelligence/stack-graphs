# A union annotation looks a member up in every member that is not None. This holds for `|`,
# `Optional`, `Union`, a quoted form, a union alias, and `x or default`.

from typing import Optional, Union

class A:
    def only_a(self):
        pass
    def shared(self):
        pass

class B:
    def only_b(self):
        pass
    def shared(self):
        pass

class C:
    def only_c(self):
        pass

# the written forms
def bar(x: A | None, y: Optional[A], z: Union[A, B], w: "A | None", v: "Optional[A]", u: Optional["A"]):
    x.only_a
    #  ^ defined: 7
    y.only_a
    #  ^ defined: 7
    z.only_b
    #  ^ defined: 13
    w.only_a
    #  ^ defined: 7
    v.only_a
    #  ^ defined: 7
    u.only_a
    #  ^ defined: 7

def many(x: A | B | None, y: "A | B", z: Union[A, B, None], w: Optional[Union[A, B]], v: "Union[A, B]", u: "A | B | None"):
    x.shared
    #  ^ defined: 9, 15
    y.shared
    #  ^ defined: 9, 15
    z.shared
    #  ^ defined: 9, 15
    w.shared
    #  ^ defined: 9, 15
    v.shared
    #  ^ defined: 9, 15
    u.only_b
    #  ^ defined: 13

# a member that only one class has
def only(x: A | B):
    x.only_a
    #  ^ defined: 7
    x.only_b
    #  ^ defined: 13
    x.only_c
    #  ^ defined:

# None adds nothing
def none_only(x: None, y: "None", z: A | None):
    x.only_a
    #  ^ defined:
    y.only_a
    #  ^ defined:
    z.only_b
    #  ^ defined:

# an attribute
class Holder:
    plain: "A | None"
    pair: Union[A, B]

    def __init__(self, a: "A | None" = None):
        self.attr: "B | None" = None
        self.param = a

    def use(self):
        self.plain.only_a
        #          ^ defined: 7
        self.pair.only_b
        #         ^ defined: 13
        self.attr.only_b
        #         ^ defined: 13
        self.param.only_a
        #          ^ defined: 7

# `x or default`
def either(x: A | None, y: "B | None"):
    z = x or B()
    z.only_a
    #  ^ defined: 7
    z.only_b
    #  ^ defined: 13
    w = y or C()
    w.only_b
    #  ^ defined: 13
    w.only_c
    #  ^ defined: 19
    v = x or y or C()
    v.only_a
    #  ^ defined: 7
    v.only_b
    #  ^ defined: 13
    v.only_c
    #  ^ defined: 19

# a union alias
Alias = A | B
AliasU = Union[A, B]
AliasO = Optional[A]
AliasN = A | None
AliasS = Union[A, "B"]
Nested = Alias | C

def alias(a: Alias, b: AliasU, c: AliasO, d: AliasN, e: "Alias", f: Nested, g: Alias | None, h: AliasS):
    a.only_b
    #  ^ defined: 13
    a.only_a
    #  ^ defined: 7
    b.only_b
    #  ^ defined: 13
    c.only_a
    #  ^ defined: 7
    d.only_a
    #  ^ defined: 7
    e.only_b
    #  ^ defined: 13
    f.only_c
    #  ^ defined: 19
    f.only_a
    #  ^ defined: 7
    g.only_b
    #  ^ defined: 13
    h.only_b
    #  ^ defined: 13
    h.only_a
    #  ^ defined: 7

# a class that has no such member does not match
def negative(a: Alias):
    a.only_c
    #  ^ defined:

# a plain reference to the alias resolves to the alias only
Alias
# <- defined: 109
AliasU
# <- defined: 110
type Pair = A | B
Pair
# <- defined: 150

def typed(x: Pair):
    x.only_a
    #  ^ defined: 7
    x.only_b
    #  ^ defined: 13
    x.only_c
    #  ^ defined:

# a return annotation that names a union
def ret_alias() -> Alias:
    return A()

def ret_string() -> "A | None":
    pass

ret_alias().only_a
#           ^ defined: 7
ret_alias().only_b
#           ^ defined: 13
ret_string().only_a
#            ^ defined: 7
ret_string().only_b
#            ^ defined:

# shapes from real code: an attribute set from `x or default`, and a closure that reads a local
class Cache:
    def isfile(self):
        pass

class Finder:
    def __init__(self, fscache: "Cache | None"):
        self.fscache = fscache or Cache()

    def use(self):
        self.fscache.isfile
        #            ^ defined: 180

def build(existing: Cache | None):
    info = existing or Cache()

    def add_field():
        info.isfile
        #    ^ defined: 180
    return add_field
